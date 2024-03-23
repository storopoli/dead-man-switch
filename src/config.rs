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
    /// Additional field for String SMTP port representation
    pub smtp_port_str: String,
    /// Additional field for String warning timer representation
    pub timer_warning_str: String,
    /// Additional field for String dead man's timer representation
    pub timer_dead_man_str: String,
    /// Additional field for String attachment representation
    pub attachment_str: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            username: "me@example.com".to_string(),
            password: "".to_string(),
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            message: "I'm probably dead, go to Central Park NY under bench #137 you'll find an age-encrypted drive. Password is our favorite music in Pascal case".to_string(),
            message_warning: "Hey, you haven't checked in for a while. Are you okay?".to_string(),
            subject: "[URGENT] Something Happened to Me!".to_string(),
            subject_warning: "[URGENT] You need to check in!".to_string(),
            to: "someone@example.com".to_string(),
            from: "me@example.com".to_string(),
            attachment: None,
            timer_warning: 60 * 60 * 24 * 14, // 2 weeks
            timer_dead_man: 60 * 60 * 24 * 7, // 1 week
            // Initialize new fields with string representations
            smtp_port_str: 587.to_string(),
            timer_warning_str: (60 * 60 * 24 * 14).to_string(), // 2 weeks
            timer_dead_man_str: (60 * 60 * 24 * 7).to_string(), // 1 week
            attachment_str: None, // No attachment by default
        }
    }
}

/// Iterator for [`Config`].
#[derive(Debug, Clone)]
pub struct ConfigIterator<'a> {
    config: &'a Config,
    index: usize,
}

/// Constructor for the ConfigIterator that takes a [`Config`].
impl<'a> ConfigIterator<'a> {
    pub fn new(config: &'a Config) -> ConfigIterator {
        ConfigIterator { config, index: 0 }
    }
}

/// Implementation of the Iterator trait for [`ConfigIterator`].
impl<'a> Iterator for ConfigIterator<'a> {
    type Item = (&'a str, &'a str);
    fn next(&mut self) -> Option<Self::Item> {
        let result = match self.index {
            0 => Some(("Username", &self.config.username as &str)),
            1 => Some(("Password", &self.config.password as &str)),
            2 => Some(("SMTP Server", &self.config.smtp_server as &str)),
            3 => Some(("SMTP Port", &self.config.smtp_port_str as &str)),
            4 => Some(("Dead Man's Message", &self.config.message as &str)),
            5 => Some(("Warning Message", &self.config.message_warning as &str)),
            6 => Some(("Dead Man's Subject", &self.config.subject as &str)),
            7 => Some(("Warning Subject", &self.config.subject_warning as &str)),
            8 => Some(("To", &self.config.to as &str)),
            9 => Some(("From", &self.config.from as &str)),
            10 => match self.config.attachment_str.as_deref() {
                Some(attachment) => Some(("Attachment File", attachment)),
                None => Some(("Attachment File", "No attachment")),
            },
            11 => Some(("Warning Timer", &self.config.timer_warning_str as &str)),
            12 => Some(("Dead Man's Timer", &self.config.timer_dead_man_str as &str)),
            _ => None,
        };
        self.index += 1;
        result
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
fn config_path() -> Result<PathBuf> {
    let base_dir = if cfg!(test) {
        // Use a temporary directory for tests
        std::env::temp_dir()
    } else {
        BaseDirs::new()
            .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?
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
        let config = load_or_initialize_config().unwrap();
        assert_eq!(config, Config::default());
        teardown();
    }

    #[test]
    fn test_config_iterator() {
        let config = Config::default();
        let mut iter = ConfigIterator::new(&config);
        assert_eq!(iter.next(), Some(("Username", &config.username as &str)));
        assert_eq!(iter.next(), Some(("Password", &config.password as &str)));
        assert_eq!(
            iter.next(),
            Some(("SMTP Server", &config.smtp_server as &str))
        );
        assert_eq!(
            iter.next(),
            Some(("SMTP Port", &config.smtp_port_str as &str))
        );
        assert_eq!(
            iter.next(),
            Some(("Dead Man's Message", &config.message as &str))
        );
        assert_eq!(
            iter.next(),
            Some(("Warning Message", &config.message_warning as &str))
        );
        assert_eq!(
            iter.next(),
            Some(("Dead Man's Subject", &config.subject as &str))
        );
        assert_eq!(
            iter.next(),
            Some(("Warning Subject", &config.subject_warning as &str))
        );
        assert_eq!(iter.next(), Some(("To", &config.to as &str)));
        assert_eq!(iter.next(), Some(("From", &config.from as &str)));
        assert_eq!(iter.next(), Some(("Attachment File", "No attachment")));
        assert_eq!(
            iter.next(),
            Some(("Warning Timer", &config.timer_warning_str as &str))
        );
        assert_eq!(
            iter.next(),
            Some(("Dead Man's Timer", &config.timer_dead_man_str as &str))
        );
    }
}
