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

use config::load_or_initialize_config;
use email::send_email;

fn main() -> Result<()> {
    let config = load_or_initialize_config()?;
    send_email(&config)?;

    Ok(())
}
