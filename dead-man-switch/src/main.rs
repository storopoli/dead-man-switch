//! # Dead Man's Switch
//!
//! This is a simple implementation of a
//! [Dead Man's Switch](https://en.wikipedia.org/wiki/Dead_man%27s_switch).
//!
//! Use at your own risk.
//! Check the f****(as in friendly) code.

use std::error::Error;

use dead_man_tui::run;

/// The main function.
///
/// This function executes the main loop of the application
/// by calling the [`run`] function.
fn main() -> Result<(), Box<dyn Error>> {
    run()?;
    Ok(())
}
