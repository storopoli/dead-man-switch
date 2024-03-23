//! This module contains the timer implementations
//!
//! Timers are created using the [`Timer`] struct.
//!
//! There are two types of timers:
//!
//! 1. The [`TimerType::Warning`] timer that emits a warning to the user's
//!    configured `From` email address upon expiration.
//! 1. The [`TimerType::DeadMan`] timer that will trigger the message and optional
//!   attachment to the user's configured `To` email address upon expiration.

use std::time::{Duration, Instant};

use anyhow::Result;

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
        format!("Time Left: {:?}", remaining)
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
}
