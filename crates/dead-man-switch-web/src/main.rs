//! Web implementation for the Dead Man's Switch.

use std::{collections::HashMap, ops::Deref, sync::Arc, time::Duration};

use anyhow::Context;
use askama::Template;
use axum::{
    error_handling::HandleErrorLayer,
    extract::{Form, FromRef, State},
    http::{Method, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    serve, BoxError, Json, Router,
};
use axum_extra::extract::cookie::{Cookie, Key, PrivateCookieJar};
use bcrypt::{hash, verify, DEFAULT_COST};
use dead_man_switch::{
    config::{load_or_initialize_config, Config, Email},
    timer::{Timer, TimerType},
};
use serde::Serialize;
use tokio::{net::TcpListener, time::sleep};
use tokio::{
    runtime::Handle,
    sync::{Mutex, RwLock},
};
use tower::{buffer::BufferLayer, limit::RateLimitLayer, ServiceBuilder};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info, subscriber, warn, Level};
use tracing_subscriber::FmtSubscriber;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// App state.
struct AppState {
    /// Dead Man's Switch [`Config`].
    config: RwLock<Config>,
    timer: Mutex<Timer>,
}

/// Secret data to be zeroized.
#[derive(Zeroize, ZeroizeOnDrop)]
struct SecretData {
    /// Password from the config.
    password: String,
    /// Hashed password from the config.
    hashed_password: String,
}

/// Wrapper for [`Key`] that provides secure zeroization.
#[derive(Clone)]
struct SecureKey {
    /// The wrapped [`Key`].
    key: Key,
    /// The pointer to the key's memory.
    ///
    /// Using an  `Arc<Mutex<Vec<u8>>>` to make the pointer thread-safe.
    bytes: Arc<Mutex<Vec<u8>>>,
}

impl SecureKey {
    /// Create a new [`SecureKey`] from a [`Key`].
    fn new(key: Key) -> Self {
        let bytes = key.master().to_vec();
        Self {
            key,
            bytes: Arc::new(Mutex::new(bytes)),
        }
    }
}

impl Zeroize for SecureKey {
    fn zeroize(&mut self) {
        match Handle::try_current() {
            Ok(rt) => {
                // block_on returns the MutexGuard directly
                let mut guard = rt.block_on(async { self.bytes.lock().await });
                guard.zeroize();
            }
            Err(_) => {
                // No runtime available, try to zeroize synchronously
                if let Ok(mut guard) = self.bytes.try_lock() {
                    guard.zeroize();
                }
            }
        }
    }
}

impl Drop for SecureKey {
    fn drop(&mut self) {
        // Use try_lock() instead of depending on the runtime
        if let Ok(guard) = self.bytes.try_lock() {
            let mut bytes = guard.to_vec();
            bytes.zeroize();
        }
    }
}

impl Deref for SecureKey {
    type Target = Key;

    fn deref(&self) -> &Self::Target {
        &self.key
    }
}

/// Combined state containing both AppState and SecretState.
#[derive(Clone)]
struct SharedState {
    /// Dead Man's Switch [`AppState`].
    app_state: Arc<AppState>,
    /// [`SecretData`] from the config.
    secret_data: Arc<SecretData>,
    /// Secret key for cookie encryption.
    key: SecureKey,
}

/// Tells [`PrivateCookieJar`] how to access the key from a [`SharedState`].
impl FromRef<SharedState> for Key {
    fn from_ref(state: &SharedState) -> Self {
        state.key.key.clone()
    }
}

/// Timer data to be sent as a JSON response.
#[derive(Serialize)]
struct TimerData {
    timer_type: String,
    time_left_percentage: u16,
    label: String,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    error: bool,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    timer_type: String,
    time_left_percentage: u16,
    label: String,
}

/// Timer loop to check for expired timers and send emails
async fn main_timer_loop(app_state: Arc<AppState>) {
    loop {
        let mut timer = app_state.timer.lock().await;
        let config = app_state.config.read().await;
        // Check timer expiration
        if timer.expired() {
            match timer.get_type() {
                TimerType::Warning => {
                    config
                        .send_email(Email::Warning)
                        .expect("Failed to send warning email");
                }
                TimerType::DeadMan => {
                    config
                        .send_email(Email::DeadMan)
                        .expect("Failed to send dead man email");
                    break;
                }
            }
        }
        let elapsed = timer.elapsed();
        timer.update(elapsed, config.timer_dead_man);
        sleep(Duration::from_secs(1)).await;
    }
}

/// Shows the login page.
async fn show_login(jar: PrivateCookieJar, State(state): State<SharedState>) -> impl IntoResponse {
    if let Some(cookie) = jar.get("auth") {
        if cookie.value() == "true" {
            let timer = state.app_state.timer.lock().await;
            let timer_type = match timer.get_type() {
                TimerType::Warning => "Warning".to_string(),
                TimerType::DeadMan => "Dead Man".to_string(),
            };
            let time_left_percentage = timer.remaining_percent();
            let label = timer.label();
            let dashboard_template = DashboardTemplate {
                timer_type,
                time_left_percentage,
                label,
            };
            return Html(
                dashboard_template
                    .render()
                    .expect("Failed to render dashboard"),
            );
        }
    }
    let login_template = LoginTemplate { error: false };
    Html(
        login_template
            .render()
            .expect("Failed to render login template"),
    )
}

/// Handles the login.
async fn handle_login(
    State(state): State<SharedState>,
    Form(params): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let jar = PrivateCookieJar::new(state.key.key.clone());

    let mut user_password = params.get("password").expect("Password not found").clone();

    let is_valid = verify(&user_password, &state.secret_data.hashed_password)
        .expect("Failed to verify password");

    // Zeroize the user-provided password after use
    user_password.zeroize();

    if is_valid {
        let updated_jar = jar.add(Cookie::new("auth", "true"));
        (updated_jar, Redirect::to("/dashboard"))
    } else {
        warn!("Unauthorized access to check-in");
        (jar, Redirect::to("/"))
    }
}

/// Handles the logout.
async fn handle_logout(jar: PrivateCookieJar) -> impl IntoResponse {
    // Remove the "auth" cookie by setting it with an empty value and "max-age" set to 0
    let updated_jar = jar.remove(Cookie::from("auth"));
    warn!("User logged out");
    (updated_jar, Redirect::to("/"))
}

/// Shows the dashboard (protected page)
async fn show_dashboard(
    jar: PrivateCookieJar,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, Redirect> {
    if let Some(cookie) = jar.get("auth") {
        if cookie.value() == "true" {
            let timer = state.app_state.timer.lock().await;
            let timer_type = match timer.get_type() {
                TimerType::Warning => "Warning".to_string(),
                TimerType::DeadMan => "Dead Man".to_string(),
            };
            let time_left_percentage = timer.remaining_percent();
            let label = timer.label();
            let dashboard_template = DashboardTemplate {
                timer_type,
                time_left_percentage,
                label,
            };
            return Ok(Html(
                dashboard_template
                    .render()
                    .expect("Failed to render dashboard"),
            ));
        }
    }
    warn!("Unauthorized access to check-in");
    Err(Redirect::to("/"))
}

/// Handle the check-in button
async fn handle_check_in(
    jar: PrivateCookieJar,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    if let Some(cookie) = jar.get("auth") {
        if cookie.value() == "true" {
            let config = state.app_state.config.read().await;
            let mut timer = state.app_state.timer.lock().await;
            timer.reset(&config);
            return Ok(Redirect::to("/dashboard"));
        }
    }
    warn!("Unauthorized access to check-in");
    Err(StatusCode::UNAUTHORIZED)
}

/// Reset without auth
async fn handle_reset_without_auth(
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let config = state.app_state.config.read().await;
    let mut timer = state.app_state.timer.lock().await;
    timer.reset(&config);
    (StatusCode::OK, "Reset ok")
}

/// Endpoint to serve the current timer data in JSON
async fn timer_data(
    jar: PrivateCookieJar,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    if let Some(cookie) = jar.get("auth") {
        if cookie.value() == "true" {
            let timer = state.app_state.timer.lock().await;
            let timer_type = format!("{:?}", timer.get_type());
            let time_left_percentage = timer.remaining_percent();
            let label = timer.label();

            let data = TimerData {
                timer_type,
                label,
                time_left_percentage,
            };

            return Ok(Json(data));
        }
    }
    warn!("Unauthorized access to timer data");
    Err(StatusCode::UNAUTHORIZED)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up logging
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::WARN)
        .finish();
    subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Instantiate the Config
    let config = load_or_initialize_config().context("Failed to load or initialize config")?;
    // Hash the password
    let password = config.web_password.clone();
    let hashed_password = hash(&password, DEFAULT_COST).expect("Failed to hash password");

    // Create a new SecretData
    let secret_data = Arc::new(SecretData {
        password,
        hashed_password,
    });

    // Save path for reset without auth
    let path = &config.route_to_reset.clone();

    // Create a new Timer
    let timer = Timer::new(
        TimerType::Warning,
        Duration::from_secs(config.timer_warning),
    );
    let app_state = Arc::new(AppState {
        config: RwLock::new(config),
        timer: Mutex::new(timer),
    });

    // Create combined shared state
    let shared_state = SharedState {
        app_state: app_state.clone(),
        key: SecureKey::new(Key::generate()),
        secret_data,
    };

    // CORS Layer
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any);

    // Routes
    let app = Router::new()
        .route("/", get(show_login).post(handle_login))
        .route("/dashboard", get(show_dashboard).post(handle_check_in))
        .route("/logout", post(handle_logout))
        .route("/timer", get(timer_data))
        .route(path, get (handle_reset_without_auth))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|err: BoxError| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled error: {}", err),
                    )
                }))
                .layer(BufferLayer::new(1024))
                .layer(RateLimitLayer::new(5, Duration::from_secs(1))),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state);

    // Main loop for the timer
    tokio::spawn(main_timer_loop(app_state));

    // Run app with axum, listening globally on port 3000
    let port = 3000_u16;
    let addr = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind to port");
    info!("router initialized, listening on port {:?}", port);
    serve(addr, app)
        .await
        .context("error while starting server")?;

    Ok(())
}
