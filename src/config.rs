//! Configuration module for the Dead Man's Switch
//! Contains functions and structs to handle the configuration.
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use directories_next::BaseDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use toml::{de::Error as DerTomlError, ser::Error as SerTomlError};

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
    /// The directory of the config file
    pub directory: Option<PathBuf>,
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
            directory: None, //When defining none, it will use the default directory.
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

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// IO operations on config module
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    /// TOML serialization
    #[error(transparent)]
    TomlSerError(#[from] SerTomlError),
    /// TOML deserialization
    #[error(transparent)]
    TomlDerError(#[from] DerTomlError),
}

/// Enum to represent the type of email to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Email {
    /// Send the warning email.
    Warning,
    /// Send the dead man's email.
    DeadMan,
}
impl Config {
    /// Load the configuration from the OS-agnostic config directory or from provided directory.
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
    pub fn config_path(self) -> Result<PathBuf, ConfigError> {
        let base_dir = if cfg!(test) {
            // Use a temporary directory for tests
            let config_dir = std::env::temp_dir().join("deadman_test");

            fs::create_dir_all(&config_dir).expect("Failed to create config directory");
            config_dir.join("config.toml")
        } else {
            if self.directory.is_none() {
                let config_dir = BaseDirs::new()
                    .expect("Failed to find home directory")
                    .config_dir()
                    .to_path_buf()
                    .join("deadman");

                fs::create_dir_all(&config_dir).expect("Failed to create config directory");
                config_dir.join("config.toml")
            } else {
                self.directory.clone().unwrap()
            }
        };
        Ok(base_dir)
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
    pub fn save_config(&self, config: &Config) -> Result<(), ConfigError> {
        let path = self.clone().config_path()?;
        let mut file = match path.is_file() {
            true => File::open(path)?,
            false => File::create(path)?,
        };
        let config = toml::to_string(config)?;

        file.write_all(config.as_bytes())?;

        Ok(())
    }

    pub fn check_path(self, provided_path: Option<PathBuf>) -> Result<PathBuf, ConfigError> {
        if provided_path.is_none() {
            _ = File::open(&self.clone().config_path()?);
            Ok(self.config_path()?)
        } else {
            let path = provided_path.unwrap();
            if path.is_dir() {
                let path = path.join("config.toml");
                //tries to open it
                _ = File::open(&path)?;
                Ok(path)
            } else {
                //tries to open it
                _ = File::open(&path)?;
                Ok(path)
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
    pub fn load_or_initialize_config(
        mut self,
        provided_path: Option<PathBuf>,
    ) -> Result<Config, ConfigError> {
        if provided_path.is_none() || self.directory.is_none() {
            let config = Config::default();
            self.directory = Some(self.clone().config_path()?);
            self.save_config(&config)?;

            Ok(config)
        } else {
            self.directory = Some(self.clone().check_path(provided_path)?);
            let config_path = self.config_path()?;
            let config = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&config)?;

            Ok(config)
        }
    }
}
#[cfg(test)]
mod test {
    use super::*;

    fn teardown(instance: Config) {
        // Cleanup test config file after each test to prevent state leakage
        let _ = fs::remove_file(instance.config_path().unwrap());
    }

    #[test]
    fn test_save_config() {
        let instance = Config::default();
        let config = instance.clone().load_or_initialize_config(None).unwrap();
        let config_path = instance.clone().config_path().unwrap();
        instance.save_config(&config).unwrap();

        let config = fs::read_to_string(config_path).unwrap();
        let config: Config = toml::from_str(&config).unwrap();
        assert_eq!(config, Config::default());
        teardown(instance);
    }

    #[test]
    fn test_load_or_initialize_config() {
        let instance = Config::default();
        instance.save_config(&instance).unwrap();
        let config = instance.clone().load_or_initialize_config(None).unwrap();
        assert_eq!(config, Config::default());
        teardown(instance);
    }
}
