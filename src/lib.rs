//! # Dead Man's Switch
//!
//! This is a simple implementation of a
//! [Dead Man's Switch](https://en.wikipedia.org/wiki/Dead_man%27s_switch)
//!
//! Use at your own risk.
//! Check the f*(as in friendly) code.

#[cfg(feature = "cli")]
pub mod cli;
pub mod config;
pub mod email;
pub mod timer;
#[cfg(feature = "tui")]
pub mod tui;
#[cfg(feature = "tui")]
pub use tui::run;
