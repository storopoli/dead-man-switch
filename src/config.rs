//! Configuration module for the Dead Man's Switch
//! Contains functions and structs to handle the configuration.
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::PathBuf;

use anyhow::Result;
use directories_next::BaseDirs;
use serde::{Deserialize, Serialize};

/// Configuration struct used for the application
///
/// ## Default
///
/// If the configuration file does not exist, it will be created with
/// the default values.
/// In other words, the default values are:
///
/// ```toml
/// username = ""
/// password = ""
/// smtp_server = ""
/// smtp_port = 0
/// message = ""
/// subject = ""
/// to = []
/// from = ""
/// timer_warning = 0
/// timer_dead_man = 0
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// The username for the email account
    pub username: String,
    /// The password for the email account
    pub password: String,
    /// The SMTP server to use
    pub smtp_server: String,
    /// The port to use for the SMTP server
    pub smtp_port: u16,
    /// The message to send in the email
    pub message: String,
    /// The subject of the email
    pub subject: String,
    /// The email address to send the email to
    pub to: String,
    /// The email address to send the email from
    pub from: String,
    /// Attachment to send with the email
    pub attachment: Option<PathBuf>,
    /// Timer in seconds for the warning email
    pub timer_warning: u64,
    /// Timer in seconds for the dead man's email
    pub timer_dead_man: u64,
}

/// Load the configuration from the OS-agnostic config directory.
///
/// Under the hood uses the [`directories_next`] crate to find the
/// home directory and the config.
///
/// ## Errors
///
/// - Fails if the home directory cannot be found
/// - Fails if the config directory cannot be created
fn config_path() -> Result<PathBuf> {
    let config_dir = BaseDirs::new()
        .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?
        .config_dir()
        .join("deadman");
    fs::create_dir_all(&config_dir).expect("Failed to create config directory");
    Ok(config_dir.join("config.toml"))
}

/// Save the configuration to the OS-agnostic config directory.
///
/// Under the hood uses the [`directories_next`] crate to find the
/// home directory and the config.
///
/// ## Errors
///
/// - Fails if the home directory cannot be found
/// - Fails if the config directory cannot be created
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = config_path()?;
    let mut file = File::create(config_path)?;
    let config = toml::to_string(config)?;
    file.write_all(config.as_bytes())?;
    Ok(())
}

/// Load the configuration from the OS-agnostic config directory.
///
/// Under the hood uses the [`directories_next`] crate to find the
/// home directory and the config.
///
/// ## Errors
///
/// - Fails if the home directory cannot be found
/// - Fails if the config directory cannot be created
///
/// ## Example
///
/// ```rust
/// use deadman::config::load_or_initialize_config;
/// let config = load_or_initialize_config().unwrap();
/// ```
pub fn load_or_initialize_config() -> Result<Config> {
    let config_path = config_path()?;
    if !config_path.exists() {
        let config = Config::default();
        save_config(&config)?;
        Ok(config)
    } else {
        let config = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&config)?;
        Ok(config)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_config_path() {
        let config_path = config_path().unwrap();
        let system_config_path = BaseDirs::new()
            .unwrap()
            .config_dir()
            .join("deadman")
            .join("config.toml");
        assert_eq!(config_path, system_config_path);
    }

    #[test]
    fn test_save_config() {
        let config = Config::default();
        save_config(&config).unwrap();
        let config_path = config_path().unwrap();
        let config = fs::read_to_string(config_path).unwrap();
        let config: Config = toml::from_str(&config).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn test_load_or_initialize_config() {
        let config = load_or_initialize_config().unwrap();
        assert_eq!(config, Config::default());
    }
}
