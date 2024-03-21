//! # Dead Man's Switch
//!
//! This is a simple implementation of a
//! [Dead Man's Switch](https://en.wikipedia.org/wiki/Dead_man%27s_switch)
//!
//! Use at your own risk.
//! Check the f*(as in friendly) code.

pub mod config;
pub mod email;

pub use config::{load_or_initialize_config, Email};
pub use email::send_email;
