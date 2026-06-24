//! Web implementation for the Dead Man's Switch.

use anyhow::Context;
use askama::Template;
use axum::{
    BoxError, Extension, Json, Router,
    error_handling::HandleErrorLayer,
    extract::{Form, FromRef, Request, State},
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    serve,
};
use axum_extra::extract::cookie::{Cookie, Key, PrivateCookieJar, SameSite};
use bcrypt::{DEFAULT_COST, hash, verify};
use dead_man_switch::{
    config::{self, Config, Email},
    timer::{Timer, TimerType},
    tor::{self, DmsTorClient, OnionEndpoint},
};
use futures::StreamExt;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as HyperBuilder;
use hyper_util::service::TowerToHyperService;
use jsonwebtoken::{
    DecodingKey, EncodingKey, Header, Validation, decode, encode, errors::Error as JsonTokenError,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Deref, path::PathBuf, sync::Arc, time::Duration};
use tokio::{net::TcpListener, time::sleep};
use tokio::{
    runtime::Handle,
    sync::{Mutex, RwLock},
};
use tower::{ServiceBuilder, buffer::BufferLayer, limit::RateLimitLayer};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{Level, error, info, subscriber, warn};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// App state.
struct AppState {
    /// Dead Man's Switch [`Config`].
    config: RwLock<Config>,
    timer: Mutex<Timer>,
    /// Bootstrapped Tor client, shared by the onion service and outbound email.
    ///
    /// `None` until/unless Tor is enabled and has finished bootstrapping.
    tor_client: RwLock<Option<Arc<DmsTorClient>>>,
}

/// Secret data to be zeroized.
#[derive(Zeroize, ZeroizeOnDrop)]
struct SecretData {
    /// Password from the config.
    password: String,
    /// Hashed password from the config.
    hashed_password: String,
    /// JWT signing key
    jwt_secret: String,
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    /// Subject (user identifier)
    sub: String,

    /// Expiration time (as UTC timestamp)
    exp: usize,

    /// Issued at (as UTC timestamp)
    iat: usize,

    /// JWT ID (unique identifier)
    jti: String,
}

impl Claims {
    fn new(user_id: String, exp_hours: i64) -> Self {
        let now = chrono::Utc::now();
        let exp = (now + chrono::Duration::hours(exp_hours)).timestamp() as usize;

        Self {
            sub: user_id,
            exp,
            iat: now.timestamp() as usize,
            jti: Uuid::new_v4().to_string(),
        }
    }
}

/// User context for authenticated requests
#[derive(Debug, Clone, Default)]
struct UserContext {
    user_id: String,
    authenticated: bool,
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
    /// Current `.onion` address, once the onion service publishes it.
    tor_address: Arc<RwLock<Option<String>>>,
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
    time_left_seconds: i64,
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
    time_left_seconds: i64,
}

/// Create a secure cookie with proper security flags
fn create_secure_cookie<'a>(name: &str, value: String, max_age_hours: i64) -> Cookie<'a> {
    Cookie::build((name.to_string(), value))
        .path("/")
        .http_only(true)
        .secure(!cfg!(debug_assertions)) // Only secure in release mode for localhost development
        .same_site(SameSite::Strict)
        .max_age(
            Duration::from_secs((max_age_hours * 3_600) as u64)
                .try_into()
                .unwrap(),
        )
        .build()
}

/// Generate JWT token
fn generate_jwt(secret: &str, claims: Claims) -> Result<String, JsonTokenError> {
    let key = EncodingKey::from_secret(secret.as_bytes());
    encode(&Header::default(), &claims, &key)
}

/// Validate JWT token
fn validate_jwt(secret: &str, token: &str) -> Result<Claims, JsonTokenError> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::default();
    decode::<Claims>(token, &key, &validation).map(|data| data.claims)
}

/// Authentication middleware - provides UserContext to all routes
async fn auth_middleware(
    State(state): State<SharedState>,
    jar: PrivateCookieJar,
    mut request: Request,
    next: Next,
) -> impl IntoResponse {
    let mut context = UserContext::default();
    let mut updated_jar = jar.clone();

    // Check for JWT token in cookies
    if let Some(jwt_cookie) = jar.get("jwt") {
        match validate_jwt(&state.secret_data.jwt_secret, jwt_cookie.value()) {
            Ok(claims) => {
                context.user_id = claims.sub;
                context.authenticated = true;
            }
            Err(_) => {
                // Invalid JWT, remove it
                updated_jar = jar.remove(Cookie::from("jwt"));
                warn!("Invalid JWT token removed");
            }
        }
    }

    // Inject the resolved context into request extensions
    request.extensions_mut().insert(context);

    let response = next.run(request).await;

    // Return response with potentially updated cookies
    (updated_jar, response).into_response()
}

/// Middleware to require authentication
async fn require_auth(
    Extension(context): Extension<UserContext>,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    if !context.authenticated {
        warn!("Unauthorized access attempt to protected route");
        return Redirect::to("/").into_response();
    }

    next.run(request).await
}

/// Timer loop to check for expired timers and send emails
async fn main_timer_loop(app_state: Arc<AppState>) -> anyhow::Result<()> {
    loop {
        let mut timer = app_state.timer.lock().await;
        // Clone the config and Tor client handle so we never hold a lock guard
        // across the `.await` on the (potentially slow) email send.
        let config = app_state.config.read().await.clone();
        let tor_client = app_state.tor_client.read().await.clone();
        // Check timer expiration
        if timer.expired() {
            match timer.get_type() {
                TimerType::Warning => {
                    if let Err(e) = config
                        .send_email_maybe_tor(tor_client.as_deref(), Email::Warning)
                        .await
                    {
                        error!(?e, "failed to send warning email");
                    }
                }
                TimerType::DeadMan => {
                    if let Err(e) = config
                        .send_email_maybe_tor(tor_client.as_deref(), Email::DeadMan)
                        .await
                    {
                        error!(?e, "failed to send dead man email");
                    }
                    return Ok(());
                }
            }
        }
        let elapsed = timer.elapsed();
        timer
            .update(elapsed, config.timer_dead_man)
            .context("Failed to update timer")?;

        // Drop the timer lock before sleeping
        drop(timer);

        sleep(Duration::from_secs(1)).await;
    }
}

// Handle favicon request
async fn handle_favicon() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

// Handle health endpoint
async fn handle_health() -> impl IntoResponse {
    "OK"
}

/// Shows the login page or redirects to dashboard if already authenticated.
async fn show_login(
    Extension(context): Extension<UserContext>,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    if context.authenticated {
        let timer = state.app_state.timer.lock().await;
        let timer_type = match timer.get_type() {
            TimerType::Warning => "Warning".to_string(),
            TimerType::DeadMan => "Dead Man".to_string(),
        };
        let time_left_percentage = timer.remaining_percent();
        let time_left_seconds = timer.remaining_chrono().num_seconds();
        let dashboard_template = DashboardTemplate {
            timer_type,
            time_left_percentage,
            time_left_seconds,
        };
        return match dashboard_template.render() {
            Ok(html) => Html(html),
            Err(_) => Html("<h1>Error rendering dashboard</h1>".to_string()),
        };
    }

    let login_template = LoginTemplate { error: false };
    match login_template.render() {
        Ok(html) => Html(html),
        Err(_) => Html("<h1>Error rendering login page</h1>".to_string()),
    }
}

/// Handles the login.
async fn handle_login(
    State(state): State<SharedState>,
    jar: PrivateCookieJar,
    Form(params): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    // Check if password field exists and is not empty
    let user_password = match params.get("password") {
        Some(password) if !password.is_empty() => password.clone(),
        _ => {
            warn!("login attempt with missing or empty password");
            return (jar, Redirect::to("/")).into_response();
        }
    };

    let is_valid = match verify(&user_password, &state.secret_data.hashed_password) {
        Ok(valid) => valid,
        Err(e) => {
            error!(?e, "failed to verify password");
            false
        }
    };

    if is_valid {
        let config = state.app_state.config.read().await;
        let cookie_exp_hours = i64::try_from(config.cookie_exp_days)
            .unwrap_or(i64::MAX)
            .saturating_mul(24);

        // Create JWT claims
        let claims = Claims::new("user".to_string(), cookie_exp_hours);

        match generate_jwt(&state.secret_data.jwt_secret, claims) {
            Ok(token) => {
                let secure_cookie = create_secure_cookie("jwt", token, cookie_exp_hours);
                let updated_jar = jar.add(secure_cookie);
                info!("User successfully authenticated");
                (updated_jar, Redirect::to("/dashboard")).into_response()
            }
            Err(e) => {
                error!(?e, "Failed to generate JWT token");
                (jar, Redirect::to("/")).into_response()
            }
        }
    } else {
        warn!("Invalid login attempt");
        (jar, Redirect::to("/")).into_response()
    }
}

/// Handles the logout.
async fn handle_logout(jar: PrivateCookieJar) -> impl IntoResponse {
    let updated_jar = jar.remove(Cookie::from("jwt"));
    info!("User logged out from web interface");
    (updated_jar, Redirect::to("/"))
}

/// Shows the dashboard (protected page)
async fn show_dashboard(
    Extension(_context): Extension<UserContext>, // Authentication guaranteed by middleware
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let timer = state.app_state.timer.lock().await;
    let timer_type = match timer.get_type() {
        TimerType::Warning => "Warning".to_string(),
        TimerType::DeadMan => "Dead Man".to_string(),
    };
    let time_left_percentage = timer.remaining_percent();
    let time_left_seconds = timer.remaining_chrono().num_seconds();
    let dashboard_template = DashboardTemplate {
        timer_type,
        time_left_percentage,
        time_left_seconds,
    };

    match dashboard_template.render() {
        Ok(html) => Html(html),
        Err(_) => Html("<h1>Error rendering dashboard</h1>".to_string()),
    }
}

/// Handle the check-in button
async fn handle_check_in(
    Extension(_context): Extension<UserContext>, // Authentication guaranteed by middleware
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let config = state.app_state.config.read().await;
    let mut timer = state.app_state.timer.lock().await;
    if let Err(e) = timer.reset(&config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error resetting timer: {}", e),
        )
            .into_response();
    }
    info!("User checked-in from web interface");
    Redirect::to("/dashboard").into_response()
}

/// Endpoint to serve the current timer data in JSON
async fn timer_data(
    Extension(_context): Extension<UserContext>, // Authentication guaranteed by middleware
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let timer = state.app_state.timer.lock().await;
    let timer_type = format!("{:?}", timer.get_type());
    let time_left_percentage = timer.remaining_percent();
    let time_left_seconds = timer.remaining_chrono().num_seconds();

    let data = TimerData {
        timer_type,
        time_left_percentage,
        time_left_seconds,
    };

    Json(data)
}

/// Tor status, returned as JSON by `/api/tor`.
#[derive(Serialize)]
struct TorInfo {
    /// Whether an onion address is currently available.
    enabled: bool,
    /// The `.onion` address, if the onion service has published one.
    onion_address: Option<String>,
}

/// Endpoint returning the current onion service address as JSON.
async fn tor_info(
    Extension(_context): Extension<UserContext>, // Authentication guaranteed by middleware
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let onion_address = state.tor_address.read().await.clone();
    Json(TorInfo {
        enabled: onion_address.is_some(),
        onion_address,
    })
}

/// Serve the web UI `Router` over the onion service's incoming streams.
///
/// Holds the [`OnionEndpoint`] (and thus the `RunningOnionService`) alive for
/// as long as it runs; returning would stop the onion service.
async fn serve_onion(mut endpoint: OnionEndpoint, app: Router) {
    while let Some(stream_request) = endpoint.streams.next().await {
        let app = app.clone();
        tokio::spawn(async move {
            let data_stream = match tor::accept_stream(stream_request).await {
                Ok(stream) => stream,
                Err(e) => {
                    warn!(?e, "failed to accept onion stream");
                    return;
                }
            };
            // axum's `Router` implements `tower::Service<Request<Incoming>>`;
            // adapt it to a hyper service and serve a single connection.
            let service = TowerToHyperService::new(app);
            if let Err(e) = HyperBuilder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(TokioIo::new(data_stream), service)
                .await
            {
                warn!(error = %e, "onion connection error");
            }
        });
    }
}

/// Bootstrap Tor, launch the onion service, and serve the web UI over it.
///
/// Runs as a long-lived background task so the clearnet listener stays
/// available immediately while Tor bootstraps. Populates `tor_address` once the
/// onion address is known and stores the shared [`DmsTorClient`] in `AppState`
/// for outbound email.
async fn run_onion_service(
    app_state: Arc<AppState>,
    app: Router,
    tor_address: Arc<RwLock<Option<String>>>,
    nickname: String,
    state_dir: Option<PathBuf>,
) {
    let client = match tor::bootstrap_tor_client(state_dir).await {
        Ok(client) => client,
        Err(e) => {
            error!(?e, "failed to bootstrap Tor client");
            return;
        }
    };
    // Share the client so outbound email can reuse it.
    *app_state.tor_client.write().await = Some(client.clone());

    let endpoint = match tor::launch_onion_service(&client, &nickname) {
        Ok(endpoint) => endpoint,
        Err(e) => {
            error!(?e, "failed to launch onion service");
            return;
        }
    };

    // The onion address may not be available immediately after launch; poll
    // briefly in the background and publish it once known.
    {
        let service = endpoint.service.clone();
        let slot = tor_address.clone();
        tokio::spawn(async move {
            for _ in 0..60 {
                if let Some(address) = tor::onion_address_of(&service) {
                    info!(%address, "onion service address available");
                    *slot.write().await = Some(address);
                    return;
                }
                sleep(Duration::from_secs(1)).await;
            }
            warn!("onion service address still unavailable after polling");
        });
    }

    serve_onion(endpoint, app).await;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get the Config data.
    let config = config::load_or_initialize().map_err(|e| anyhow::anyhow!(e))?;

    // Set up logging
    let log_level = config
        .log_level
        .as_deref()
        .unwrap_or("info")
        .parse::<Level>()
        .unwrap_or(Level::INFO);
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    subscriber::set_global_default(subscriber)
        .map_err(|e| anyhow::anyhow!("Setting default subscriber failed: {}", e))?;

    // Create a new Timer
    // Will be initialised from any persisted state, or be set to defaults
    let timer = Timer::new(&config).map_err(|e| anyhow::anyhow!(e))?;

    if config.smtp_check_timeout.is_some_and(|t| t > 0) {
        let smtp_ok = config.check_smtp_connection().is_ok();
        if smtp_ok {
            info!("SMTP server verified");
        } else {
            error!("SMTP server timeout - Check config");
        }
    } else {
        warn!("SMTP server verification disabled");
    }

    // Hash the password
    let password = config.web_password.clone();
    let hashed_password = hash(&password, DEFAULT_COST)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

    // Generate a secure JWT secret
    let jwt_secret = Uuid::new_v4().to_string();

    // Create a new SecretData
    let secret_data = Arc::new(SecretData {
        password,
        hashed_password,
        jwt_secret,
    });

    // Capture Tor settings before `config` is moved into the shared state.
    let tor_enabled = config.tor_enabled;
    let tor_nickname = config.tor_nickname.clone();
    let tor_state_dir = config.tor_state_dir.clone().map(PathBuf::from);

    let app_state = Arc::new(AppState {
        config: RwLock::new(config),
        timer: Mutex::new(timer),
        tor_client: RwLock::new(None),
    });

    // Shared slot for the onion address, populated once the service is up.
    let tor_address = Arc::new(RwLock::new(None));

    // Create combined shared state
    let shared_state = SharedState {
        app_state: app_state.clone(),
        key: SecureKey::new(Key::generate()),
        secret_data,
        tor_address: tor_address.clone(),
    };

    // More restrictive CORS - only allow same origin in production
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any); // For simplicity, keeping permissive for now

    // Protected routes that require authentication
    let protected_routes = Router::new()
        .route("/dashboard", get(show_dashboard).post(handle_check_in))
        .route("/timer", get(timer_data))
        .route("/api/tor", get(tor_info))
        .route("/check-in", get(handle_check_in))
        .layer(middleware::from_fn(require_auth));

    // Public routes
    let public_routes = Router::new()
        .route("/", get(show_login).post(handle_login))
        .route("/favicon.ico", get(handle_favicon))
        .route("/health", get(handle_health))
        .route("/logout", post(handle_logout));

    // Combine routes and apply auth middleware to all
    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(middleware::from_fn_with_state(
            shared_state.clone(),
            auth_middleware,
        ))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|err: BoxError| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled error: {err}"),
                    )
                }))
                .layer(BufferLayer::new(1_024))
                .layer(RateLimitLayer::new(5, Duration::from_secs(1))),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state);

    // Launch the Tor onion service (and share the Tor client for outbound
    // email) when enabled. Runs in the background so the clearnet listener is
    // available immediately while Tor bootstraps.
    if tor_enabled {
        tokio::spawn(run_onion_service(
            app_state.clone(),
            app.clone(),
            tor_address.clone(),
            tor_nickname,
            tor_state_dir,
        ));
    } else {
        info!("Tor disabled in config");
    }

    // Main loop for the timer
    tokio::spawn(async move {
        if let Err(e) = main_timer_loop(app_state).await {
            error!(?e, "main timer loop failed");
        }
    });

    // Run app with axum, listening globally on port 3000
    let port = 3000_u16;
    let addr = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .context("Failed to bind to port")?;
    info!(port, "router initialized, listening on port");
    serve(addr, app)
        .await
        .context("error while starting server")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cookie_duration_conversion() {
        let mut c = Cookie::new("name", "value");
        let config = Config::default();
        let duration = Duration::from_secs(config.cookie_exp_days * 60 * 60 * 24)
            .try_into()
            .unwrap();

        c.set_max_age(duration);
        assert_eq!(c.max_age(), Some(duration));
    }

    #[test]
    fn test_jwt_generation_and_validation() {
        let secret = "test_secret";
        let claims = Claims::new("test_user".to_string(), 1);

        let token = generate_jwt(secret, claims).unwrap();
        let decoded = validate_jwt(secret, &token).unwrap();

        assert_eq!(decoded.sub, "test_user");
    }
}
