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

use std::time::{Duration, Instant};

use chrono::Duration as ChronoDuration;

/// The timer enum.
///
/// See [`timer`](crate::timer) module for more information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerType {
    /// The warning timer.
    Warning,
    /// Dead Man's Switch timer.
    DeadMan,
}

/// The timer struct.
///
/// Holds the [`TimerType`], current the duration, and the expiration time.
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
    pub fn new(timer_type: TimerType, duration: Duration) -> Self {
        Timer {
            timer_type,
            start: Instant::now(),
            duration,
        }
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

    /// Calculate the remaining time as a percentage
    pub fn remaining_percent(&self) -> u16 {
        let elapsed = self.start.elapsed().as_secs();
        let total = self.duration.as_secs();
        let remaining = if elapsed < total { total - elapsed } else { 0 };
        (remaining as f64 / total as f64 * 100.0) as u16
    }

    /// Update label based on the remaining time
    pub fn label(&self) -> String {
        let remaining = self.duration - self.start.elapsed();
        let remaining_chrono =
            ChronoDuration::try_seconds(remaining.as_secs() as i64).expect("Invalid duration");
        format_duration(remaining_chrono)
    }

    /// Update the timer logic for switching from [`TimerType::Warning`] to
    /// [`TimerType::DeadMan`].
    pub fn update(&mut self, elapsed: Duration, dead_man_duration: u64) {
        if self.timer_type == TimerType::Warning && elapsed >= self.duration {
            self.timer_type = TimerType::DeadMan;
            // Reset the start time for the DeadMan timer
            self.start = Instant::now();
            self.duration = Duration::from_secs(dead_man_duration);
        }
    }

    /// Check if the timer has expired.
    pub fn expired(&self) -> bool {
        self.start.elapsed() >= self.duration
    }

    /// Reset the timer and promotes the timer type from [`TimerType::DeadMan`]
    /// to [`TimerType::Warning`], if applicable.
    ///
    /// This is called when the user checks in.
    pub fn reset(&mut self, config: &crate::config::Config) {
        match self.get_type() {
            TimerType::Warning => {
                self.start = Instant::now();
            }
            TimerType::DeadMan => {
                self.timer_type = TimerType::Warning;
                self.start = Instant::now();
                self.duration = Duration::from_secs(config.timer_warning);
            }
        }
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
        parts.push(format!("{} day(s)", days));
    }
    if hours > 0 {
        parts.push(format!("{} hour(s)", hours));
    }
    if minutes > 0 {
        parts.push(format!("{} minute(s)", minutes));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{} second(s)", seconds + 1));
    }

    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_or_initialize_config;
    use std::thread::sleep;

    #[test]
    fn timer_creation() {
        let warning_timer = Timer::new(TimerType::Warning, Duration::from_secs(60));
        assert_eq!(warning_timer.get_type(), TimerType::Warning);
        assert!(warning_timer.duration == Duration::from_secs(60));
    }

    #[test]
    fn timer_elapsed_less_than_duration() {
        let timer = Timer::new(TimerType::Warning, Duration::from_secs(60));
        assert!(timer.elapsed() < Duration::from_secs(60));
    }

    #[test]
    fn timer_update_to_dead_man() {
        let mut timer = Timer::new(TimerType::Warning, Duration::from_secs(1));
        // Simulate elapsed time by directly manipulating the timer's state.
        timer.update(Duration::from_secs(2), 3600);
        assert_eq!(timer.get_type(), TimerType::DeadMan);
        assert_eq!(timer.duration, Duration::from_secs(3600));
        assert!(!timer.expired());
    }

    #[test]
    fn timer_expiration() {
        let timer = Timer::new(TimerType::Warning, Duration::from_secs(1));
        // Directly simulate the passage of time
        sleep(Duration::from_secs(2));
        assert!(timer.expired());
    }

    #[test]
    fn format_seconds_only() {
        let duration = ChronoDuration::try_seconds(45).unwrap();
        assert_eq!(format_duration(duration), "46 second(s)");
    }

    #[test]
    fn format_minutes_and_seconds() {
        let duration =
            ChronoDuration::try_minutes(5).unwrap() + ChronoDuration::try_seconds(30).unwrap();
        assert_eq!(format_duration(duration), "5 minute(s), 31 second(s)");
    }

    #[test]
    fn format_hours_minutes_and_seconds() {
        let duration = ChronoDuration::try_hours(2).unwrap()
            + ChronoDuration::try_minutes(15).unwrap()
            + ChronoDuration::try_seconds(10).unwrap();
        assert_eq!(
            format_duration(duration),
            "2 hour(s), 15 minute(s), 11 second(s)"
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
            "7 day(s), 23 hour(s), 59 minute(s), 60 second(s)"
        );
    }

    #[test]
    fn reset_warning_timer_resets_start_time() {
        let config = load_or_initialize_config().unwrap();

        let mut timer = Timer::new(
            TimerType::Warning,
            Duration::from_secs(config.timer_warning),
        );
        let original_start = timer.start;
        // Simulate time passing
        sleep(Duration::from_millis(100));
        timer.reset(&config);
        assert!(timer.start > original_start);
        assert_eq!(timer.duration, Duration::from_secs(config.timer_warning));
        assert_eq!(timer.get_type(), TimerType::Warning);
    }

    #[test]
    fn reset_dead_man_timer_promotes_to_warning_and_resets() {
        let config = load_or_initialize_config().unwrap();

        let mut timer = Timer::new(
            TimerType::DeadMan,
            Duration::from_secs(config.timer_dead_man),
        );
        // Simulate time passing
        sleep(Duration::from_millis(100));
        timer.reset(&config);
        assert_eq!(timer.get_type(), TimerType::Warning);
        assert_eq!(timer.duration, Duration::from_secs(config.timer_warning));
    }
}
