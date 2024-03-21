//! # Dead Man's Switch
//!
//! This is a simple implementation of a
//! [Dead Man's Switch](https://en.wikipedia.org/wiki/Dead_man%27s_switch)
//!
//! Use at your own risk.
//! Check the f*(as in friendly) code.

use anyhow::Result;

mod config;
mod email;

fn main() -> Result<()> {
    let _ = config::load_or_initialize_config()?;
    Ok(())
}
