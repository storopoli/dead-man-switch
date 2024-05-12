//! Configuration module for the Dead Man's Switch
//! Contains functions and structs to handle the configuration.
use directories_next::BaseDirs;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::PathBuf;
use thiserror::Error;

/// Configuration struct used for the application
///
/// ## Default
///
/// If the configuration file does not exist, it will be created with
/// the default values.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The username for the email account.
    pub username: String,
    /// The password for the email account.
    pub password: String,
    /// The SMTP server to use
    pub smtp_server: String,
    /// The port to use for the SMTP server.
    pub smtp_port: u16,
    /// The message to send in the email if you fail to check in
    /// after the `timer_warning` with the additional `timer_dead_man`
    /// seconds have passed.
    pub message: String,
    /// The warning message if you fail to check in `timer_warning` seconds.
    pub message_warning: String,
    /// The subject of the email if you fail to check in
    /// after the `timer_warning` with the additional `timer_dead_man`
    /// seconds have passed.
    pub subject: String,
    /// The subject of the email if you fail to check in `timer_warning` seconds.
    pub subject_warning: String,
    /// The email address to send the email to.
    pub to: String,
    /// The email address to send the email from.
    pub from: String,
    /// Attachment to send with the email.
    pub attachment: Option<PathBuf>,
    /// Timer in seconds for the warning email.
    pub timer_warning: u64,
    /// Timer in seconds for the dead man's email.
    pub timer_dead_man: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            username: "me@example.com".to_string(),
            password: "".to_string(),
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            message: "I'm probably dead, go to Central Park NY under bench #137 you'll find an age-encrypted drive. Password is our favorite music in Pascal case.".to_string(),
            message_warning: "Hey, you haven't checked in for a while. Are you okay?".to_string(),
            subject: "[URGENT] Something Happened to Me!".to_string(),
            subject_warning: "[URGENT] You need to check in!".to_string(),
            to: "someone@example.com".to_string(),
            from: "me@example.com".to_string(),
            attachment: None,
            timer_warning: 60 * 60 * 24 * 14, // 2 weeks
            timer_dead_man: 60 * 60 * 24 * 7, // 1 week
        }
    }
}

/// Enum to represent the type of email to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Email {
    /// Send the warning email.
    Warning,
    /// Send the dead man's email.
    DeadMan,
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
/// ## Notes
///
/// This function handles testing and non-testing environments.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to get home directory")]
    HomeDir,
    #[error("Failed to create config directory")]
    CreateConfigDir,
    #[error(transparent)]
    CreateFile(#[from] std::io::Error), 
    #[error("Failed to save config file")]
    SaveConfigFile,
    #[error("Failed to read config file")]
    ReadConfigFile,
    #[error("Failed to parse config file")]
    ParseConfigFile,
}

pub fn config_path() -> Result<PathBuf, ConfigError> {
    let base_dir = if cfg!(test) {
        // Use a temporary directory for tests
        std::env::temp_dir()
    } else {
        BaseDirs::new()
            .ok_or_else(|| ConfigError::HomeDir)?
            .config_dir()
            .to_path_buf()
    };

    let config_dir = base_dir.join(if cfg!(test) {
        "deadman_test"
    } else {
        "deadman"
    });

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

pub fn save_config(config: &Config) -> Result<(), ConfigError> {
    // Tries to catch the config path
    let config_path = config_path().map_err(|_| ConfigError::CreateConfigDir)?;
    // Tries to create the config file
    let mut file = File::create(&config_path).map_err(|_| ConfigError::CreateConfigFile)?;
    // Tries to serialize the config to a string
    let config = toml::to_string(config).map_err(|_| ConfigError::SaveConfigFile)?;
    // Tries to write the config to the file
    match file.write_all(config.as_bytes()) {
        Ok(_) => Ok(()),
        Err(_) => {
            return Err(ConfigError::SaveConfigFile);
        }
    }
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
/// use dead_man_switch::config::load_or_initialize_config;
/// let config = load_or_initialize_config().unwrap();
/// ```
pub fn load_or_initialize_config() -> Result<Config, ConfigError> {
    let config_path = config_path()?;
    if !config_path.exists() {
        let config = Config::default();
        save_config(&config)?;
        Ok(config)
    } else {
        // Tries to read the config file
        let config = fs::read_to_string(&config_path).map_err(|_| ConfigError::ReadConfigFile)?;
        let config: Config = toml::from_str(&config).map_err(|_| ConfigError::ParseConfigFile)?;
        Ok(config)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn teardown() {
        // Cleanup test config file after each test to prevent state leakage
        let _ = fs::remove_file(config_path().unwrap());
    }

    #[test]
    fn test_save_config() {
        let config = Config::default();
        save_config(&config).unwrap();
        let config_path = config_path().unwrap();
        let config = fs::read_to_string(config_path).unwrap();
        let config: Config = toml::from_str(&config).unwrap();
        assert_eq!(config, Config::default());
        teardown();
    }

    #[test]
    fn test_load_or_initialize_config() {
        let config = Config::default();
        save_config(&config).unwrap();
        let config = load_or_initialize_config().unwrap();
        assert_eq!(config, Config::default());
        teardown();
    }
}
