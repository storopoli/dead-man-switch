pub use lettre::address::AddressError;
use lettre::{
    error::Error as LettreError,
    message::header::ContentTypeErr,
    transport::smtp::{self},
};
use thiserror::Error;
use toml::{de::Error as DerTomlError, ser::Error as SerTomlError};

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
}
