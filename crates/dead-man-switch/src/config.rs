//! Configuration module for the Dead Man's Switch
//! Contains functions and structs to handle the configuration.
use crate::app;
use crate::error::ConfigError;

use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Configuration struct used for the application
///
/// # Default
///
/// If the configuration file does not exist, it will be created with
/// the default values.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct Config {
    /// The username for the email account.
    pub username: String,
    /// The password for the email account.
    pub password: String,
    /// The SMTP server to use
    pub smtp_server: String,
    /// The port to use for the SMTP server.
    pub smtp_port: u16,
    /// The timeout to use for the SMTP server.
    pub smtp_check_timeout: Option<u64>,
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
    pub attachment: Option<String>,
    /// Timer in seconds for the warning email.
    pub timer_warning: u64,
    /// Timer in seconds for the dead man's email.
    pub timer_dead_man: u64,
    /// Web interface password
    pub web_password: String,
    /// Cookie expiration to avoid need for login
    pub cookie_exp_days: u64,
    /// Log level for the web interface.
    pub log_level: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        // Use the WEB_PASSWORD environment variable if set, otherwise generate
        // a cryptographically secure random password. This prevents the security
        // vulnerability of having a hardcoded default password that attackers
        // could exploit.
        let web_password = env::var("WEB_PASSWORD")
            .ok()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        Self {
            username: "me@example.com".to_string(),
            password: "".to_string(),
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_check_timeout: Some(5),
            message: "I'm probably dead, go to Central Park NY under bench #137 you'll find an age-encrypted drive. Password is our favorite music in Pascal case.".to_string(),
            message_warning: "Hey, you haven't checked in for a while. Are you okay?".to_string(),
            subject: "[URGENT] Something Happened to Me!".to_string(),
            subject_warning: "[URGENT] You need to check in!".to_string(),
            to: "someone@example.com".to_string(),
            from: "me@example.com".to_string(),
            attachment: None,
            timer_warning: 60 * 60 * 24 * 14, // 2 weeks
            timer_dead_man: 60 * 60 * 24 * 7, // 1 week
            web_password,
            cookie_exp_days: 7,
            log_level: None,
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

/// Returns the name of the config file
fn file_name() -> &'static str {
    "config.toml"
}

/// Get the configuration file path
///
/// # Errors
///
/// - Fails if the home directory cannot be found
///
pub fn file_path() -> Result<PathBuf, ConfigError> {
    let path = app::file_path(file_name())?;
    Ok(path)
}

/// Save the configuration file.
///
/// # Errors
///
/// - Fails if the home directory cannot be found
/// - Fails if the config directory cannot be created
///
pub fn save(config: &Config) -> Result<(), ConfigError> {
    let file_path = file_path()?;
    let mut file = File::create(file_path)?;
    let config = toml::to_string(config)?;

    file.write_all(config.as_bytes())?;

    Ok(())
}

/// Load or initialize the configuration file.
///
/// # Errors
///
/// - Fails if the home directory cannot be found
/// - Fails if the config directory cannot be created
///
/// # Example
///
/// ```rust
/// use dead_man_switch::config::load_or_initialize;
/// let config = load_or_initialize().unwrap();
/// ```
///
pub fn load_or_initialize() -> Result<Config, ConfigError> {
    let file_path = file_path()?;
    if !file_path.exists() {
        let config = Config::default();
        save(&config)?;

        Ok(config)
    } else {
        let config_str = fs::read_to_string(&file_path)?;
        let config: Config = toml::from_str(&config_str)?;

        Ok(config)
    }
}

/// Parses the attachment path from the [`Config`].
///
/// # Errors
///
/// - If the attachment path is not found.
/// - If the attachment path is not a valid path.
///
pub fn attachment_path(config: &Config) -> Result<PathBuf, ConfigError> {
    let attachment_path = config
        .attachment
        .as_ref()
        .ok_or(ConfigError::AttachmentNotFound)?;
    Ok(PathBuf::from(attachment_path))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::Path;

    struct TestGuard;
    impl TestGuard {
        fn new(c: &Config) -> Self {
            // setup before test

            let file_path = file_path().expect("setup: failed file_path()");

            // Ensure parent directory exists
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).expect("setup: failed to create dir");
            }
            let mut file = File::create(file_path).expect("setup: failed to create file");
            let c_str = toml::to_string(c).expect("setup: failed to convert data");
            file.write_all(c_str.as_bytes())
                .expect("setup: failed to write data");
            file.sync_all()
                .expect("setup: failed to ensure file written to disk");

            TestGuard
        }
    }
    impl Drop for TestGuard {
        fn drop(&mut self) {
            // clean-up after a test
            let file_path = file_path().expect("teardown: failed file_path()");
            cleanup_test_dir_parent(file_path.as_path());
        }
    }

    // helper
    fn cleanup_test_dir(dir: &Path) {
        if let Some(parent) = dir.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    // helper
    fn cleanup_test_dir_parent(dir: &Path) {
        if let Some(parent) = dir.parent() {
            cleanup_test_dir(parent)
        }
    }

    // helper
    fn load_config_from_path(path: &PathBuf) -> Config {
        let config_str = fs::read_to_string(path).expect("helper: error reading config data");
        let config: Config =
            toml::from_str(&config_str).expect("helper: error parsing config data");
        config
    }

    #[test]
    fn file_path_in_test_mode() {
        // This test verifies that file_path() uses temp directory in test mode
        let result = file_path();
        assert!(result.is_ok());

        let result = result.unwrap();
        let expected = format!("{}_test", app::name());
        assert!(result.to_string_lossy().contains(expected.as_str()));

        // It should also of course contain the actual file name
        let expected = Path::new(app::name()).join(file_name());
        assert!(result
            .to_string_lossy()
            .contains(expected.to_string_lossy().as_ref()));

        // Cleanup any created directories
        cleanup_test_dir_parent(&result);
    }

    #[test]
    fn save_config() {
        // Set state for this test
        let mut config = Config::default();
        config.message = "test save".to_string();

        let result = save(&config);
        assert!(result.is_ok());

        let test_path = file_path().unwrap();
        // Compare against the same config instance that was saved,
        // not a new default (which would have a different random password)
        let loaded_config = load_config_from_path(&test_path);
        // Compare against the original config instance
        assert_eq!(loaded_config, config);

        // Cleanup any created directories
        cleanup_test_dir_parent(&test_path);
    }

    #[test]
    fn timer_guard_ok() {
        // This test verifies that the guard is working as expected
        // by saving a timer and reading it back

        // Set state for this test
        let mut config = Config::default();
        config.message = "test guard".to_string();
        let _guard = TestGuard::new(&config);

        // Compare against the same config instance saved by guard
        let test_path = file_path().unwrap();
        let loaded_config = load_config_from_path(&test_path);
        assert_eq!(loaded_config, config);
    }

    #[test]
    fn load_or_initialize_with_existing_file() {
        // Set state for this test
        let mut existing_config = Config::default();
        existing_config.message = "test load".to_string();
        let _guard = TestGuard::new(&existing_config);

        // With config data persisted, we should see a config with those values
        let config = load_or_initialize().unwrap();

        // Compare against the same config instance that was saved,
        // not a new default (which would have a different random password)
        assert_eq!(config, existing_config);
    }

    #[test]
    fn load_or_initialize_with_no_existing_file() {
        let mut config_default = Config::default();

        // With no previous data persisted, we should see a config with defaults
        let mut config = load_or_initialize().unwrap();

        // deal with the the random web_password generation
        config_default.web_password = "".to_string();
        config.web_password = "".to_string();

        assert_eq!(config, config_default);

        // Cleanup any created directories
        let test_path = file_path().unwrap();
        cleanup_test_dir_parent(&test_path);
    }

    #[test]
    fn example_config_is_valid() {
        let example_config = fs::read_to_string("../../config.example.toml").unwrap();
        let config: Result<Config, toml::de::Error> = toml::from_str(&example_config);
        assert!(config.is_ok());
    }
}
