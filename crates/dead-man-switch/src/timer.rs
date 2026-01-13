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
use crate::config;
use crate::error::TimerError;

use chrono::Duration as ChronoDuration;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// The timer struct.
///
/// Holds the [`TimerType`], current start and expiration times.
/// See [`timer`](crate::timer) module for more information.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Timer {
    /// The timer type.
    pub timer_type: TimerType,
    /// The start time.
    pub start: u64,
    /// The duration.
    pub duration: u64,
}

pub fn system_time_epoch() -> Result<Duration, TimerError> {
    let now = SystemTime::now();
    let since_the_epoch = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TimerError::SystemTime)?;

    Ok(since_the_epoch)
}

/// Load or initialize the persisted state file.
///
/// # Errors
///
/// - Fails if the home directory cannot be found
/// - Fails if the state directory cannot be created
///
pub fn load_or_initialize() -> Result<Timer, TimerError> {
    let config = config::load_or_initialize()?;
    let file_path = file_path()?;
    if !file_path.exists() {
        let timer = Timer {
            timer_type: TimerType::Warning,
            start: system_time_epoch()?.as_secs(),
            duration: config.timer_warning,
        };

        update_persisted_state(&timer)?;

        Ok(timer)
    } else {
        let timer_str = fs::read_to_string(&file_path)?;
        let timer: Timer = toml::from_str(timer_str.as_str())?;

        Ok(timer)
    }
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

/// Save the persisted state file.
///
/// # Errors
///
/// - Fails if the home directory cannot be found
/// - Fails if the state directory cannot be created
///
pub fn update_persisted_state(timer: &Timer) -> Result<(), TimerError> {
    let file_path = file_path()?;
    let mut file = File::create(file_path)?;
    let timer_str = toml::to_string(timer)?;

    file.write_all(timer_str.as_bytes())?;

    Ok(())
}

impl Timer {
    /// Create a new timer.
    pub fn new() -> Result<Self, TimerError> {
        let timer = load_or_initialize()?;

        Ok(timer)
    }

    /// Get the type of the timer.
    /// Returns [`TimerType`].
    pub fn get_type(&self) -> TimerType {
        self.timer_type
    }

    /// Get the elapsed time.
    pub fn elapsed(&self) -> u64 {
        match SystemTime::now().duration_since(UNIX_EPOCH + Duration::from_secs(self.start)) {
            Ok(dur) => dur.as_secs(),
            Err(_) => 0,
        }
    }

    /// Calculate the remaining time
    pub fn remaining_chrono(&self) -> ChronoDuration {
        let elapsed = self.elapsed();
        if elapsed < self.duration {
            let remaining = self.duration.saturating_sub(elapsed);
            return ChronoDuration::try_seconds(remaining as i64).unwrap_or(ChronoDuration::zero());
        }

        ChronoDuration::zero()
    }

    /// Calculate the remaining time as a percentage
    pub fn remaining_percent(&self) -> u16 {
        let remaining_chrono = self.remaining_chrono();

        if remaining_chrono > ChronoDuration::zero() {
            return (remaining_chrono.num_seconds() as f64 / self.duration as f64 * 100.0) as u16;
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
    pub fn update(&mut self, elapsed: u64, dead_man_duration: u64) -> Result<(), TimerError> {
        if self.timer_type == TimerType::Warning && elapsed >= self.duration {
            self.timer_type = TimerType::DeadMan;
            self.start = system_time_epoch()?.as_secs();
            self.duration = dead_man_duration;
        }

        update_persisted_state(self)?;

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
        match self.get_type() {
            TimerType::Warning => {
                self.start = system_time_epoch()?.as_secs();
            }
            TimerType::DeadMan => {
                self.timer_type = TimerType::Warning;
                self.start = system_time_epoch()?.as_secs();
                self.duration = config.timer_warning;
            }
        }

        update_persisted_state(self)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use std::thread::sleep;

    fn get_test_config() -> Config {
        Config {
            username: "user@example.com".to_string(),
            password: "password".to_string(),
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
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
        fn new(t: &Timer) -> Self {
            // setup before test

            let file_path = file_path().expect("setup: failed file_path()");

            // Ensure parent directory exists
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).expect("setup: failed to create dir");
            }
            let mut file = File::create(file_path).expect("setup: failed to create file");
            let t_str = toml::to_string(t).expect("setup: failed to convert data");
            file.write_all(t_str.as_bytes())
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
    fn load_timer_from_path(path: &PathBuf) -> Timer {
        let timer_str = fs::read_to_string(path).expect("helper: error reading timer data");
        let timer: Timer = toml::from_str(&timer_str).expect("helper: error parsing timer data");
        timer
    }

    #[test]
    fn file_path_in_test_mode() {
        // This test verifies that file_path() uses temp directory in test mode
        let result = file_path();
        assert!(result.is_ok());

        let result = result.unwrap();
        let expected = format!("{}_test", app::name());
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

        let result = result.unwrap().as_secs();
        let expected = 1767225600; // 2026-01-01
        assert!(
            result > expected,
            "expected: (after 2026-01-01) '{:?}' got: '{:?}')",
            expected,
            result
        );

        let expected = 4102444800; // 2100-01-01
        assert!(
            result < expected,
            "expected: (before 2100-01-01) '{:?}' got: '{:?}')",
            expected,
            result
        );
    }

    #[test]
    fn update_persisted_state_ok() {
        // Set state for this test
        let timer = Timer {
            timer_type: TimerType::Warning,
            start: 1,
            duration: 2,
        };

        let result = update_persisted_state(&timer);
        assert!(result.is_ok());

        let test_path = file_path().unwrap();
        let loaded_timer = load_timer_from_path(&test_path);
        // Compare against the original timer instance
        assert_eq!(loaded_timer, timer);

        // Cleanup any created directories
        cleanup_test_dir_parent(&test_path);
    }

    #[test]
    fn timer_guard_ok() {
        // This test verifies that the guard is working as expected
        // by saving a timer and reading it back

        // Set state for this test
        let timer = Timer {
            timer_type: TimerType::Warning,
            start: 1,
            duration: 2,
        };
        let _guard = TestGuard::new(&timer);

        // Compare against the same timer instance saved by guard
        let test_path = file_path().unwrap();
        let loaded_timer = load_timer_from_path(&test_path);
        assert_eq!(loaded_timer, timer);
    }

    #[test]
    fn load_or_initialize_with_existing_file() {
        // Set state for this test
        let existing_timer = Timer {
            timer_type: TimerType::DeadMan,
            start: 3,
            duration: 4,
        };
        let _guard = TestGuard::new(&existing_timer);

        // With timer data persisted, we should see a timer with those values
        let timer = load_or_initialize().unwrap();

        // Compare loaded timer against the existing timer data that was saved
        assert_eq!(timer, existing_timer);
    }

    #[test]
    fn load_or_initialize_with_no_existing_file() {
        let config = Config::default();

        // With no previous data persisted, we should see a timer with defaults
        let timer = load_or_initialize().unwrap();

        let result = timer.timer_type;
        let expected = TimerType::Warning;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        let result = timer.duration;
        let expected = config.timer_warning;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        // checking time can be tricky in tests, so just check that it's within 10 seconds
        let result = timer.start;
        let expected = system_time_epoch()
            .expect("failed to get current time")
            .as_secs();
        assert!(
            result > (expected - 10) && result < (expected + 10),
            "expected (within 10 of): '{:?}' got: '{:?}')",
            expected,
            result
        );

        // Cleanup any created directories
        let test_path = file_path().unwrap();
        cleanup_test_dir_parent(&test_path);
    }

    #[test]
    fn timer_remaining_chrono() {
        // Set state for this test
        let _guard = TestGuard::new(&Timer {
            start: system_time_epoch()
                .expect("failed to get current time")
                .as_secs(),
            timer_type: TimerType::Warning,
            duration: 2,
        });

        let timer = Timer::new().expect("failed to create new timer");

        let result = timer.remaining_chrono();
        let time_delta = chrono::TimeDelta::new(1, 0).expect("failed creating time delta");
        assert!(
            result > time_delta,
            "expected: timer.remaining_chrono() > 1 (got: {:?})",
            result
        );

        sleep(Duration::from_secs(2));

        let result = timer.remaining_chrono();
        assert!(
            result < time_delta,
            "expected: timer.remaining_chrono() < 1 (got: {:?})",
            result
        );
    }

    #[test]
    fn timer_elapsed_less_than_duration() {
        // Set state for this test
        let _guard = TestGuard::new(&Timer {
            start: system_time_epoch()
                .expect("failed to get current time")
                .as_secs(),
            timer_type: TimerType::Warning,
            duration: 60,
        });

        let timer = Timer::new().expect("failed to create new timer");

        let result = timer.elapsed();
        assert!(
            result < 2,
            "expected: timer.elapsed() < 2 (got: {:?})",
            result
        );

        sleep(Duration::from_secs_f64(1.5));

        let result = timer.elapsed();
        assert!(
            result >= 1,
            "expected: timer.elapsed() >= 1 (got: {:?})",
            result
        );
    }

    #[test]
    fn timer_update_to_dead_man() {
        // Set state for this test
        let _guard = TestGuard::new(&Timer {
            start: system_time_epoch()
                .expect("failed to get current time")
                .as_secs(),
            timer_type: TimerType::Warning,
            duration: 1,
        });

        let mut timer = Timer::new().expect("failed to create new timer");

        // Simulate elapsed time by directly manipulating the timer's state.
        timer.update(2, 3600).expect("Failed to update timer");

        let result = timer.get_type();
        let expected = TimerType::DeadMan;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        let result = timer.duration;
        let expected = 3600;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );

        let result = timer.expired();
        let expected = false;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );
    }

    #[test]
    fn timer_expiration() {
        // Set state for this test
        let _guard = TestGuard::new(&Timer {
            start: system_time_epoch()
                .expect("failed to get current time")
                .as_secs(),
            timer_type: TimerType::Warning,
            duration: 0,
        });

        let timer = Timer::new().expect("failed to create new timer");

        // Directly simulate the passage of time
        sleep(Duration::from_secs(1));

        let result = timer.expired();
        let expected = true;
        assert!(
            result == expected,
            "expected: '{:?}' got: '{:?}')",
            expected,
            result
        );
    }

    #[test]
    fn timer_remaining_percent() {
        // Set state for this test
        let _guard = TestGuard::new(&Timer {
            start: system_time_epoch()
                .expect("failed to get current time")
                .as_secs(),
            timer_type: TimerType::Warning,
            duration: 2,
        });

        let timer = Timer::new().expect("failed to create new timer");

        let result = timer.remaining_percent();
        assert!(
            result > 0,
            "expected: timer.remaining_percent() > 0 (got: {:?})",
            result
        );

        // Directly simulate the passage of time
        sleep(Duration::from_secs(2));

        let result = timer.remaining_percent();
        assert!(
            result == 0,
            "expected: timer.remaining_percent() == 0 (got: {:?})",
            result
        );
    }

    #[test]
    fn timer_label() {
        // Set state for this test
        let _guard = TestGuard::new(&Timer {
            start: system_time_epoch()
                .expect("failed to get current time")
                .as_secs(),
            timer_type: TimerType::Warning,
            duration: 2,
        });

        let timer = Timer::new().expect("failed to create new timer");

        let result = timer.label();
        let expected = "0 second(s)";
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

        // Set state for this test
        let _guard = TestGuard::new(&Timer {
            start: 1767225600, // 2026-01-01
            timer_type: TimerType::Warning,
            duration: config.timer_warning,
        });

        let mut timer = Timer::new().expect("failed to create new timer");

        let original_start = timer.start;

        timer.reset(&config).expect("failed to reset timer");

        let result = timer.start;
        let expected = original_start;
        assert!(
            result > expected,
            "expected: timer.start > {:?} (got: {:?})",
            expected,
            result
        );

        let result = timer.duration;
        let expected = config.timer_warning;
        assert!(
            result == expected,
            "expected: timer.duration == {:?} (got: {:?})",
            expected,
            result
        );

        let result = timer.get_type();
        let expected = TimerType::Warning;
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
        let _guard = TestGuard::new(&Timer {
            start: 1767225600, // 2026-01-01
            timer_type: TimerType::DeadMan,
            duration: config.timer_dead_man,
        });

        let mut timer = Timer::new().expect("failed to create new timer");

        timer.reset(&config).expect("failed to reset timer");

        let result = timer.duration;
        let expected = config.timer_warning;
        assert!(
            result == expected,
            "expected: timer.duration == {:?} (got: {:?})",
            expected,
            result
        );

        let result = timer.get_type();
        let expected = TimerType::Warning;
        assert!(
            result == expected,
            "expected: timer.get_type() == {:?} (got: {:?})",
            expected,
            result
        );
    }
}
