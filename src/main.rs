//! # Dead Man's Switch
//!
//! This is a simple implementation of a
//! [Dead Man's Switch](https://en.wikipedia.org/wiki/Dead_man%27s_switch).
//!
//! Use at your own risk.
//! Check the f****(as in friendly) code.

#[cfg(feature = "tui")]
use thiserror::Error;
#[derive(Error, Debug)]
pub enum MainError {
    #[error("Failed to run everything 😔")]
    InitingError,
}

#[cfg(feature = "tui")]
use dead_man_switch::run;

/// The main function.
///
/// This function executes the main loop of the application
/// by calling the [`run`] function.
#[cfg(feature = "tui")]
fn main() -> Result<(), MainError> {
    match run() {
        Ok(_) => Ok(()),
        Err(_) => Err(MainError::InitingError),
    }
}
