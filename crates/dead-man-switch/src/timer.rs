//! Timer implementations
//!
//! Timers are created using the [`Timer`] struct.
//!
//! There are two types of timers:
//!
//! 1. The [`TimerType::Warning`] timer that emits a warning to the user's
//!    configured `From` email address upon expiration.
//! 1. The [`TimerType::DeadMan`] timer that will trigger the message and optional
//!    attachment to the user's configured `To` email address upon expiration.

use crate::app;
use crate::config::Config;
use crate::error::TimerError;

use chrono::Duration as ChronoDuration;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{error, trace, warn};

static PERSIST_SENDER: OnceLock<mpsc::Sender<State>> = OnceLock::new();

const DEFAULT_DEBOUNCE_MS: u64 = 300;

/// The timer enum.
///
/// See [`timer`](crate::timer) module for more information.
#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum TimerType {
    /// The warning timer.
    #[default]
    Warning,
    /// Dead Man's Switch timer.
    DeadMan,
}

/// The state struct.
///
/// Holds the [`TimerType`] and last modified time.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct State {
    /// The timer type.
    timer_type: TimerType,
    /// The last modified time.
    last_modified: u64,
}

/// The timer struct.
///
/// Holds the [`TimerType`], current start and expiration times.
/// See [`timer`](crate::timer) module for more information.
#[derive(Debug, Clone, PartialEq)]
pub struct Timer {
    /// The timer type.
    timer_type: TimerType,
    /// The start time.
    start: Instant,
    /// The duration.
    duration: Duration,
}

impl Timer {
    /// Create a new timer.
    pub fn new(config: &Config) -> Result<Self, TimerError> {
        let timer = load_or_initialize(config)?;

        Ok(timer)
    }

    /// Get the type of the timer.
    /// Returns [`TimerType`].
    pub fn get_type(&self) -> TimerType {
        self.timer_type
    }

    /// Get the elapsed time.
    pub fn elapsed(&self) -> Duration {
        Instant::now().duration_since(self.start)
    }

    /// Calculate the remaining time
    pub fn remaining_chrono(&self) -> ChronoDuration {
        let elapsed = self.elapsed();
        if elapsed < self.duration {
            let remaining = self.duration.saturating_sub(elapsed);
            return ChronoDuration::try_seconds(remaining.as_secs() as i64)
                .unwrap_or(ChronoDuration::zero());
        }

        ChronoDuration::zero()
    }

    /// Calculate the remaining time as a percentage
    pub fn remaining_percent(&self) -> u16 {
        let remaining_chrono = self.remaining_chrono();

        if remaining_chrono > ChronoDuration::zero() {
            return (remaining_chrono.num_seconds() as f64 / self.duration.as_secs() as f64 * 100.0)
                as u16;
        }

        0
    }

    /// Update label based on the remaining time
    pub fn label(&self) -> String {
        let remaining_chrono = self.remaining_chrono();
        if remaining_chrono > ChronoDuration::zero() {
            return format_duration(remaining_chrono);
        }

        "0 second(s)".to_string()
    }

    /// Update the timer logic for switching from [`TimerType::Warning`] to
    /// [`TimerType::DeadMan`].
    pub fn update(&mut self, elapsed: Duration, dead_man_duration: u64) -> Result<(), TimerError> {
        if self.timer_type == TimerType::Warning && elapsed >= self.duration {
            self.timer_type = TimerType::DeadMan;
            // Reset the start time for the DeadMan timer
            self.start = Instant::now();
            self.duration = Duration::from_secs(dead_man_duration);

            let state = state_from_timer(self)?;
            persist_state_non_blocking(state)?;
        }

        Ok(())
    }

    /// Check if the timer has expired.
    pub fn expired(&self) -> bool {
        self.elapsed() >= self.duration
    }

    /// Reset the timer and promote the timer type from [`TimerType::DeadMan`]
    /// to [`TimerType::Warning`], if applicable.
    ///
    /// This is called when the user checks in.
    pub fn reset(&mut self, config: &crate::config::Config) -> Result<(), TimerError> {
        let (timer, state) = default_timer_and_state(config)?;
        self.timer_type = timer.timer_type;
        self.start = timer.start;
        self.duration = timer.duration;

        persist_state_non_blocking(state)?;

        Ok(())
    }
}

/// Formats a duration into a human-readable string adjusting the resolution based on the duration.
fn format_duration(duration: ChronoDuration) -> String {
    let days = duration.num_days();
    let hours = duration.num_hours() % 24;
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;

    let mut parts = vec![];

    if days > 0 {
        parts.push(format!("{days} day(s)"));
    }
    if hours > 0 {
        parts.push(format!("{hours} hour(s)"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes} minute(s)"));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{} second(s)", seconds));
    }

    parts.join(", ")
}

/// Load or initialize the persisted state file.
///
/// # Errors
///
/// - Fails if the home directory cannot be found
/// - Fails if the state directory cannot be created
/// - Fails if the state file is not writeable
///
pub fn load_or_initialize(config: &Config) -> Result<Timer, TimerError> {
    let file_path = file_path()?;

    // try to read persisted state; if it fails, fall back to default timer
    let (timer, state) = match fs::read_to_string(&file_path) {
        Ok(state_str) => {
            match toml::from_str::<State>(state_str.as_str()) {
                Ok(state) => {
                    // persistence file OK - setup timer based on persisted state
                    let wall_elapsed = wall_elapsed(state.last_modified);
                    // Build a monotonic start Instant such that
                    // Instant::now() - start == wall_elapsed,
                    // but guard against underflow using checked_sub.
                    let now_mono = Instant::now();
                    let start = match now_mono.checked_sub(wall_elapsed) {
                        Some(s) => s,
                        None => now_mono, // clamp to zero elapsed
                    };
                    let timer = Timer {
                        timer_type: state.timer_type,
                        start,
                        duration: match state.timer_type {
                            TimerType::Warning => Duration::from_secs(config.timer_warning),
                            TimerType::DeadMan => Duration::from_secs(config.timer_dead_man),
                        },
                    };
                    (timer, state)
                }
                Err(e) => {
                    // fall back to default (same behaviour as "no persistence file")
                    warn!(
                        error = ?e,
                        path = %file_path.display(),
                        "persisted file parse error: using defaults"
                    );
                    default_timer_and_state(config)?
                }
            }
        }
        Err(_) => {
            // fall back to default (same behaviour as "no persistence file")
            trace!(
                path = %file_path.display(),
                "no persisted file found: using defaults"
            );
            default_timer_and_state(config)?
        }
    };

    // set the initial state for persistence worker
    let _ = PERSIST_SENDER.get_or_init(|| spawn_persistence_worker(Some(state.clone())));

    // ensure we can write state to file (even if it's just been read)
    // this is a chance to verify write-ability (future writes will be handled by background task)
    persist_state_blocking(state)?;

    Ok(timer)
}

fn default_timer_and_state(config: &Config) -> Result<(Timer, State), TimerError> {
    let timer = Timer {
        timer_type: TimerType::Warning,
        start: Instant::now(),
        duration: Duration::from_secs(config.timer_warning),
    };
    let state = state_from_timer(&timer)?;

    Ok((timer, state))
}

/// Returns the name of the persisted state file.
fn file_name() -> &'static str {
    "state.toml"
}

/// Get the persisted state file path.
///
/// # Errors
///
/// - Fails if the home directory cannot be found
///
pub fn file_path() -> Result<PathBuf, TimerError> {
    let path = app::file_path(file_name())?;

    Ok(path)
}

// wall clock
pub fn system_time_epoch() -> Result<Duration, TimerError> {
    let now_wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(TimerError::SystemTime)?;

    Ok(now_wall)
}

/// Compute wall elapsed since last modified
///
/// If last_modified is in the future (relative to current wall clock)
/// clamp to zero so we don't artificially extend the timer
fn wall_elapsed(last_modified: u64) -> Duration {
    let persisted_last_modified = Duration::from_secs(last_modified);
    let now_wall = system_time_epoch().unwrap_or(Duration::from_secs(0));

    if now_wall > persisted_last_modified {
        now_wall - persisted_last_modified
    } else {
        Duration::from_secs(0)
    }
}

fn state_from_timer(timer: &Timer) -> Result<State, TimerError> {
    let state = State {
        timer_type: timer.timer_type,
        last_modified: system_time_epoch()?.as_secs(),
    };

    Ok(state)
}

/// Blocking persistence of state.
///
/// - Only used at app initialisation (allows foreground verification of persistence)
///
pub fn persist_state_blocking(state: State) -> Result<(), TimerError> {
    let path = file_path()?;
    if let Err(e) = persist_state_to_path(&path, &state) {
        error!(error = ?e, path = %path.display(), "persist new state failed");
        return Err(e);
    }

    Ok(())
}

/// Non-blocking persistence of state.
///
/// - Only persists if the state has changed (debounced)
/// - Uses a worker to offload tasks from the main thread
///
fn persist_state_non_blocking(state: State) -> Result<(), TimerError> {
    let sender = PERSIST_SENDER
        .get_or_init(|| spawn_persistence_worker(Some(state.clone())))
        .clone();
    if let Err(e) = sender.send(state) {
        error!(error = ?e, "failed to enqueue persistence of state; background worker may have stopped");
    }
    Ok(())
}

fn spawn_persistence_worker(state: Option<State>) -> mpsc::Sender<State> {
    let (tx, rx) = mpsc::channel::<State>();

    thread::spawn(move || {
        let mut last_written: Option<State> = state;
        let debounce = Duration::from_millis(DEFAULT_DEBOUNCE_MS);

        while let Ok(mut snapshot) = rx.recv() {
            // Drain immediately to get the latest snapshot available now.
            while let Ok(next) = rx.try_recv() {
                snapshot = next;
            }

            // Debounce window: drain any additional updates arriving shortly.
            let start = Instant::now();
            while start.elapsed() < debounce {
                if let Ok(next) = rx.try_recv() {
                    snapshot = next;
                } else {
                    // small sleep to avoid busy-wait
                    thread::sleep(Duration::from_millis(10));
                }
            }

            // Skip write if identical to last written
            if last_written.as_ref() == Some(&snapshot) {
                trace!("skipping persist state: identical to last written");
                continue;
            }

            if let Ok(path) = file_path() {
                if let Err(e) = persist_state_to_path(&path, &snapshot) {
                    error!(error = ?e, path = %path.display(), "persist new state failed");
                } else {
                    trace!(path = %path.display(), "persisted new state");
                    last_written = Some(snapshot);
                }
            }
        }
    });

    tx
}

fn persist_state_to_path(path: &PathBuf, state: &State) -> Result<(), TimerError> {
    let state_str = toml::to_string(state)?;
    write_atomic(path, state_str.as_bytes())
}

fn write_atomic(path: &PathBuf, data: &[u8]) -> Result<(), TimerError> {
    let tmp_path = path.with_extension("tmp");
    {
        let mut tmp = File::create(&tmp_path)?;
        tmp.write_all(data)?;
        tmp.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{self, Config};
    use std::fs::{self, File};
    use std::io::Write;
    use std::ops::Sub;
    use std::path::Path;
    use std::thread::sleep;

    fn get_test_config() -> Config {
        Config {
            username: "user@example.com".to_string(),
            password: "password".to_string(),
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_check_timeout: Some(5),
            message: "This is a test message".to_string(),
            message_warning: "This is a test warning message".to_string(),
            subject: "Test Subject".to_string(),
            subject_warning: "Test Warning Subject".to_string(),
            to: "recipient@example.com".to_string(),
            from: "sender@example.com".to_string(),
            attachment: None,
            timer_warning: 60,
            timer_dead_man: 120,
            web_password: "password".to_string(),
            cookie_exp_days: 7,
            log_level: None,
        }
    }

    struct TestGuard;
    impl TestGuard {
        fn new(s: &State) -> Self {
            // setup before test

            let file_path = file_path().expect("setup: failed file_path()");

            // Ensure parent directory exists
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).expect("setup: failed to create dir");
            }
            let mut file = File::create(file_path).expect("setup: failed to create file");
            let s_str = toml::to_string(s).expect("setup: failed to convert data");
            file.write_all(s_str.as_bytes())
                .expect("setup: failed to write data");
            file.sync_all()
                .expect("setup: failed to ensure file written to disk");

            TestGuard
        }
    }
    impl Drop for TestGuard {
        fn drop(&mut self) {
            // clean-up after a test
            let file_path = file_path().expect("teardown: failed file_path()");
            cleanup_test_dir_parent(file_path.as_path());
        }
    }

    // helper
    fn cleanup_test_dir(dir: &Path) {
        if let Some(parent) = dir.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    // helper
    fn cleanup_test_dir_parent(dir: &Path) {
        if let Some(parent) = dir.parent() {
            cleanup_test_dir(parent)
        }
    }

    // helper
    fn load_state_from_path(path: &PathBuf) -> State {
        let state_str = fs::read_to_string(path).expect("helper: error reading state data");
        let state: State = toml::from_str(&state_str).expect("helper: error parsing state data");
        state
    }

    #[test]
    fn file_path_in_test_mode() {
        // This test verifies that file_path() uses temp directory in test mode
        let result = file_path();
        assert!(result.is_ok());

        let expected = format!("{}_test", app::name());
        let result = result.unwrap();
        assert!(result.to_string_lossy().contains(expected.as_str()));

        // It should also of course contain the actual file name
        let expected = Path::new(app::name()).join(file_name());
        assert!(result
            .to_string_lossy()
            .contains(expected.to_string_lossy().as_ref()));

        // Cleanup any created directories
        cleanup_test_dir_parent(&result);
    }

    #[test]
    fn generates_system_time_epoch() {
        let result = system_time_epoch();
        assert!(result.is_ok());

        let expected_range_low = 1767225600; // 2026-01-01 00:00:00
        let expected_range_high = 4102444800; // 2100-01-01 00:00:00
        let result = result.unwrap().as_secs();
        assert!(
            result >= expected_range_low && result <= expected_range_high,
            "expected: between {:?} (2026-01-01 00:00:00) and {:?} (2100-01-01 00:00:00) (got: {:?})",
            expected_range_low,
            expected_range_high,
            result
        );
    }

    #[test]
    fn update_persisted_state_ok() {
        // Set state for this test
        let state = State {
            timer_type: TimerType::Warning,
            last_modified: 1,
        };

        let result = persist_state_blocking(state.clone());
        assert!(result.is_ok());

        let test_path = file_path().unwrap();
        let loaded_state = load_state_from_path(&test_path);
        // Compare against the original state instance
        assert_eq!(loaded_state, state);

        // Cleanup any created directories
        cleanup_test_dir_parent(&test_path);
    }

    #[test]
    fn state_guard_ok() {
        // This test verifies that the guard is working as expected
        // by saving a state and reading it back

        // Set state for this test
        let state = State {
            timer_type: TimerType::DeadMan,
            last_modified: 2,
        };
        let _guard = TestGuard::new(&state);

        // Compare against the same state instance saved by guard
        let test_path = file_path().unwrap();
        let loaded_state = load_state_from_path(&test_path);
        assert_eq!(loaded_state, state);
    }

    #[test]
    fn load_or_initialize_with_existing_file() {
        let config = Config::default();

        // Set state for this test
        let existing_state_1 = State {
            timer_type: TimerType::Warning,
            last_modified: 3,
        };
        let existing_state_2 = State {
            timer_type: TimerType::DeadMan,
            last_modified: 4,
        };

        let _guard_1 = TestGuard::new(&existing_state_1);

        // With state data persisted, we should see a timer with those values
        let timer = load_or_initialize(&config).unwrap();

        // Compare loaded timer against the existing state data that was saved
        assert_eq!(timer.timer_type, existing_state_1.timer_type);

        let _guard_2 = TestGuard::new(&existing_state_2);

        // With state data persisted, we should see a timer with those values
        let timer = load_or_initialize(&config).unwrap();

        // Compare loaded timer against the existing state data that was saved
        assert_eq!(timer.timer_type, existing_state_2.timer_type);
    }

    #[test]
    fn load_or_initialize_with_no_existing_file() {
        // get default config so we can use its timer_warning later
        let config = Config::default();

        // With no previous data persisted, we should see a timer with defaults
        let timer = load_or_initialize(&config).unwrap();

        let expected = TimerType::Warning;
        let result = timer.timer_type;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        let expected = Duration::from_secs(config.timer_warning);
        let result = timer.duration;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        // Cleanup any created directories
        let test_path = file_path().unwrap();
        cleanup_test_dir_parent(&test_path);
    }

    #[test]
    fn timer_remaining_chrono_with_state_in_past() {
        let default_config = config::load_or_initialize().expect("failed to load default config");

        let now_wall = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();

        // Set state for this test
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::Warning,
            last_modified: now_wall - 10000,
        });

        let timer = Timer::new(&default_config).expect("failed to create new timer");

        let tolerance_secs = 5;
        let expected_range_high =
            chrono::TimeDelta::new((default_config.timer_warning - 10000) as i64, 0)
                .expect("failed creating time delta");
        let expected_range_low = expected_range_high
            .sub(chrono::TimeDelta::new(tolerance_secs, 0).expect("failed creating time delta"));
        let result = timer.remaining_chrono();
        // result should be in the range (tolerance for slow tests)
        assert!(
            result >= expected_range_low && result <= expected_range_high,
            "expected: timer.remaining_chrono() between {:?} and {:?} (got: {:?})",
            expected_range_low,
            expected_range_high,
            result
        );
    }

    #[test]
    fn timer_remaining_chrono_with_state_in_future() {
        let default_config = config::load_or_initialize().expect("failed to load default config");

        let now_wall = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();

        // Set state for this test
        // a system time in the future should not increase remaining chrono time
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::Warning,
            last_modified: now_wall + 1000,
        });

        let timer = Timer::new(&default_config).expect("failed to create new timer");

        let tolerance_secs = 5;
        let expected_range_high = chrono::TimeDelta::new((default_config.timer_warning) as i64, 0)
            .expect("failed creating time delta");
        let expected_range_low = expected_range_high
            .sub(chrono::TimeDelta::new(tolerance_secs, 0).expect("failed creating time delta"));
        // result should be in the range (tolerance for slow tests)
        let result = timer.remaining_chrono();
        assert!(
            result >= expected_range_low && result <= expected_range_high,
            "expected: timer.remaining_chrono() between {:?} and {:?} (got: {:?})",
            expected_range_low,
            expected_range_high,
            result
        );
    }

    #[test]
    fn timer_remaining_chrono_with_state() {
        let mut config = get_test_config();

        let now_wall = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();

        // Set state for this test
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::Warning,
            last_modified: now_wall,
        });

        let mut timer = Timer::new(&config).expect("failed to create new timer");

        config.timer_warning = 2;
        // reset allows injection of (test) config
        timer.reset(&config).expect("failed to reset timer");

        let expected = chrono::TimeDelta::new(2, 0).expect("failed creating time delta");
        let result = timer.remaining_chrono();
        assert!(
            result <= expected,
            "expected: timer.remaining_chrono() < {:?} (got: {:?})",
            expected,
            result
        );

        sleep(Duration::from_secs(2));

        let expected = chrono::TimeDelta::new(1, 0).expect("failed creating time delta");
        let result = timer.remaining_chrono();
        assert!(
            result <= expected,
            "expected: timer.remaining_chrono() < {:?} (got: {:?})",
            expected,
            result
        );
    }

    #[test]
    fn timer_elapsed_less_than_duration() {
        let mut config = get_test_config();

        let now_wall = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();

        // Set state for this test
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::Warning,
            last_modified: now_wall,
        });

        let mut timer = Timer::new(&config).expect("failed to create new timer");

        config.timer_warning = 60;
        // reset allows injection of (test) config
        timer.reset(&config).expect("failed to reset timer");

        let expected = Duration::from_secs(2);
        let result = timer.elapsed();
        assert!(
            result < expected,
            "expected: timer.elapsed() < {:?} (got: {:?})",
            expected,
            result
        );

        sleep(Duration::from_secs_f64(1.5));

        let expected = Duration::from_secs(1);
        let result = timer.elapsed();
        assert!(
            result > expected,
            "expected: timer.elapsed() > {:?} (got: {:?})",
            expected,
            result
        );
    }

    #[test]
    fn timer_update_to_dead_man() {
        let mut config = get_test_config();

        let now_wall = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();

        // Set state for this test
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::Warning,
            last_modified: now_wall,
        });

        let mut timer = Timer::new(&config).expect("failed to create new timer");

        config.timer_warning = 1;
        // reset allows injection of (test) config
        timer.reset(&config).expect("failed to reset timer");

        // verify timer still in TimerType::Warning
        let expected = TimerType::Warning;
        let result = timer.timer_type;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        // verify timer still has expected timer_warning
        let expected = Duration::from_secs(config.timer_warning);
        let result = timer.duration;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        // Simulate elapsed time by directly manipulating the timer's state.
        timer
            .update(Duration::from_secs(2), 3600)
            .expect("Failed to update timer");

        let expected = TimerType::DeadMan;
        let result = timer.get_type();
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        let expected = Duration::from_secs(3600);
        let result = timer.duration;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        let expected = false;
        let result = timer.expired();
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );
    }

    #[test]
    fn timer_expiration() {
        let mut config = get_test_config();

        let now_wall = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();

        // Set state for this test
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::Warning,
            last_modified: now_wall,
        });

        let mut timer = Timer::new(&config).expect("failed to create new timer");

        config.timer_warning = 1;
        // reset allows injection of (test) config
        timer.reset(&config).expect("failed to reset timer");

        // verify timer still in TimerType::Warning
        let expected = TimerType::Warning;
        let result = timer.timer_type;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        // verify timer still has expected timer_warning
        let expected = Duration::from_secs(config.timer_warning);
        let result = timer.duration;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        // Simulate elapsed time by directly manipulating the timer's state.
        timer
            .update(Duration::from_secs(2), 1)
            .expect("Failed to update timer");

        // Directly simulate the passage of time
        sleep(Duration::from_secs(2));

        let expected = true;
        let result = timer.expired();
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );
    }

    #[test]
    fn timer_remaining_percent() {
        let mut config = get_test_config();

        let now_wall = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();

        // Set state for this test
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::Warning,
            last_modified: now_wall,
        });

        let mut timer = Timer::new(&config).expect("failed to create new timer");

        config.timer_warning = 2;
        // reset allows injection of (test) config
        timer.reset(&config).expect("failed to reset timer");

        let expected = 0;
        let result = timer.remaining_percent();
        assert!(
            result > expected,
            "expected: timer.remaining_percent() > {:?} (got: {:?})",
            expected,
            result
        );

        // Directly simulate the passage of time
        sleep(Duration::from_secs(2));

        let result = timer.remaining_percent();
        assert!(
            result == expected,
            "expected: timer.remaining_percent() == {:?} (got: {:?})",
            expected,
            result
        );
    }

    #[test]
    fn timer_label() {
        let mut config = get_test_config();

        let now_wall = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();

        // Set state for this test
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::Warning,
            last_modified: now_wall,
        });

        let mut timer = Timer::new(&config).expect("failed to create new timer");

        config.timer_warning = 2;
        // reset allows injection of (test) config
        timer.reset(&config).expect("failed to reset timer");

        let expected = "0 second(s)";
        let result = timer.label();
        assert!(
            result != expected,
            "expected: timer.label() != {:?} (got: {:?})",
            expected,
            result
        );

        // Directly simulate the passage of time
        sleep(Duration::from_secs(2));

        let result = timer.label();
        assert!(
            result == expected,
            "expected: timer.label() == {:?} (got: {:?})",
            expected,
            result
        );
    }

    #[test]
    fn format_seconds_only() {
        let duration = ChronoDuration::try_seconds(45).unwrap();
        assert_eq!(format_duration(duration), "45 second(s)");
    }

    #[test]
    fn format_minutes_and_seconds() {
        let duration =
            ChronoDuration::try_minutes(5).unwrap() + ChronoDuration::try_seconds(30).unwrap();
        assert_eq!(format_duration(duration), "5 minute(s), 30 second(s)");
    }

    #[test]
    fn format_hours_minutes_and_seconds() {
        let duration = ChronoDuration::try_hours(2).unwrap()
            + ChronoDuration::try_minutes(15).unwrap()
            + ChronoDuration::try_seconds(10).unwrap();
        assert_eq!(
            format_duration(duration),
            "2 hour(s), 15 minute(s), 10 second(s)"
        );
    }

    #[test]
    fn format_days_hours_minutes() {
        let duration = ChronoDuration::try_days(1).unwrap()
            + ChronoDuration::try_hours(3).unwrap()
            + ChronoDuration::try_minutes(45).unwrap();
        assert_eq!(
            format_duration(duration),
            "1 day(s), 3 hour(s), 45 minute(s)"
        );
    }

    #[test]
    fn format_days_only() {
        let duration = ChronoDuration::try_days(4).unwrap();
        assert_eq!(format_duration(duration), "4 day(s)");
    }

    #[test]
    fn format_large_duration() {
        let duration = ChronoDuration::try_days(7).unwrap()
            + ChronoDuration::try_hours(23).unwrap()
            + ChronoDuration::try_minutes(59).unwrap()
            + ChronoDuration::try_seconds(59).unwrap();
        assert_eq!(
            format_duration(duration),
            "7 day(s), 23 hour(s), 59 minute(s), 59 second(s)"
        );
    }

    #[test]
    fn reset_warning_timer_resets_start_time() {
        let config = get_test_config();

        let now_wall = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();

        // Set state for this test
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::Warning,
            last_modified: now_wall - 60,
        });

        let mut timer = Timer::new(&config).expect("failed to create new timer");

        let original_start = timer.start;

        timer.reset(&config).expect("failed to reset timer");

        let expected = original_start;
        let result = timer.start;
        assert!(
            result > expected,
            "expected: timer.start > {:?} (got: {:?})",
            expected,
            result
        );

        let expected = Duration::from_secs(config.timer_warning);
        let result = timer.duration;
        assert!(
            result == expected,
            "expected: timer.duration == {:?} (got: {:?})",
            expected,
            result
        );

        let expected = TimerType::Warning;
        let result = timer.get_type();
        assert!(
            result == expected,
            "expected: timer.get_type() == {:?} (got: {:?})",
            expected,
            result
        );
    }

    #[test]
    fn reset_dead_man_timer_promotes_to_warning_and_resets() {
        let config = get_test_config();

        // Set state for this test
        let _guard = TestGuard::new(&State {
            timer_type: TimerType::DeadMan,
            last_modified: system_time_epoch()
                .expect("failed to get current time")
                .as_secs(),
        });

        let mut timer = Timer::new(&config).expect("failed to create new timer");

        timer.reset(&config).expect("failed to reset timer");

        let expected = Duration::from_secs(config.timer_warning);
        let result = timer.duration;
        assert!(
            result == expected,
            "expected: timer.duration == {:?} (got: {:?})",
            expected,
            result
        );

        let expected = TimerType::Warning;
        let result = timer.get_type();
        assert!(
            result == expected,
            "expected: timer.get_type() == {:?} (got: {:?})",
            expected,
            result
        );
    }
}
