pub use lettre::address::AddressError;
use lettre::{
    error::Error as LettreError,
    message::header::ContentTypeErr,
    transport::smtp::{self},
};
use thiserror::Error;
use toml::{de::Error as DerTomlError, ser::Error as SerTomlError};

/// Home Directory Errors
#[derive(Error, Debug)]
pub enum HomeDirError {
    /// IO operations on home directory.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Failed to find home directory.
    #[error("Failed to find home directory")]
    HomeDirNotFound,
}

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// IO operations on config module.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// TOML serialization.
    #[error(transparent)]
    TomlSerialization(#[from] SerTomlError),

    /// TOML deserialization.
    #[error(transparent)]
    TomlDeserialization(#[from] DerTomlError),

    /// Attachment not found.
    #[error("Attachment not found")]
    AttachmentNotFound,

    /// Config directory errors.
    #[error(transparent)]
    ConfigFile(#[from] HomeDirError),
}

// Timer errors
#[derive(Error, Debug)]
pub enum TimerError {
    /// IO operations on timer module.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// TOML serialization.
    #[error(transparent)]
    TomlSerialization(#[from] SerTomlError),

    /// TOML deserialization.
    #[error(transparent)]
    TomlDeserialization(#[from] DerTomlError),

    /// Config error
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// Config directory errors.
    #[error(transparent)]
    ConfigFile(#[from] HomeDirError),

    /// SystemTime error
    #[error("System time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),
}

/// Errors that can occur when sending an email.
#[derive(Error, Debug)]
pub enum EmailError {
    /// TLS error when sending the email.
    #[error(transparent)]
    TlsError(#[from] smtp::Error),

    /// Error when parsing email addresses.
    #[error(transparent)]
    EmailError(#[from] AddressError),

    /// Error when building the email.
    #[error(transparent)]
    BuilderError(#[from] LettreError),

    /// Error when reading the attachment.
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    /// Error when determining the content type of the attachment.
    #[error(transparent)]
    InvalidContent(#[from] ContentTypeErr),

    /// Error when determining the content type of the attachment.
    #[error(transparent)]
    AttachmentPath(#[from] ConfigError),

    /// Timeout error
    #[error("timeout")]
    Timeout,

    /// Disconnected error
    #[error("disconnected")]
    Disconnected,

    #[error("smtp error: {0}")]
    SmtpError(smtp::Error),
}

/// TUI Error type.
#[derive(Error, Debug)]
pub enum TuiError {
    /// IO Error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// [`ConfigError`] blanket error conversion.
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// [`EmailError`] blanket error conversion.
    #[error(transparent)]
    Email(#[from] EmailError),

    /// [`TimerError`] blanket error conversion.
    #[error(transparent)]
    Timer(#[from] TimerError),
}
