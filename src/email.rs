//! Email sending capabilities of the Dead Man's Switch.

use std::fs;

use anyhow::Result;
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    Message, SmtpTransport, Transport,
};

use crate::config::Config;

/// Send the email using the provided configuration.
///
/// ## Errors
///
/// - If the email fails to send.
/// - If the email cannot be created.
/// - If the attachment cannot be read.
///
/// ## Notes
///
/// If the attachment MIME type cannot be determined, it will default to
/// `application/octet-stream`.
pub fn send_email(config: &Config) -> Result<()> {
    let email = create_email(config)?;

    // SMTP client setup
    let creds = Credentials::new(config.username.clone(), config.password.clone());
    let tls = TlsParameters::new_rustls(config.smtp_server.clone())?;
    let mailer = SmtpTransport::relay(&config.smtp_server)?
        .port(config.smtp_port)
        .credentials(creds)
        .tls(Tls::Required(tls))
        .build();

    // Send the email
    mailer.send(&email)?;

    Ok(())
}

/// Create the email to send.
///
/// If an attachment is provided, the email will be created with the attachment.
fn create_email(config: &Config) -> Result<Message> {
    // Guaranteed config values
    let from = Mailbox::new(None, config.from.parse()?);
    let to = Mailbox::new(None, config.to.parse()?);

    // Email metadata
    let email_builder = Message::builder()
        .from(from)
        .to(to)
        .subject(&config.subject);

    // Email body
    let text_part = SinglePart::builder()
        .header(ContentType::TEXT_PLAIN)
        .body(config.message.clone());

    // Optional attachment
    match &config.attachment {
        Some(attachment) => {
            let filename = attachment
                .file_name()
                .expect("Failed to get filename")
                .to_string_lossy();
            let filebody = fs::read(attachment)?;
            let content_type = ContentType::parse(
                mime_guess::from_path(attachment)
                    .first_or_octet_stream()
                    .as_ref(),
            )?;

            let attachment_part =
                Attachment::new(filename.to_string()).body(filebody, content_type);

            let email = email_builder.multipart(
                MultiPart::mixed()
                    .singlepart(text_part)
                    .singlepart(attachment_part),
            )?;
            Ok(email)
        }
        None => {
            let email = email_builder.singlepart(text_part)?;
            Ok(email)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn get_test_config() -> Config {
        Config {
            username: "user@example.com".to_string(),
            password: "password".to_string(),
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            message: "This is a test message".to_string(),
            subject: "Test Subject".to_string(),
            to: "recipient@example.com".to_string(),
            from: "sender@example.com".to_string(),
            attachment: None,
            timer_warning: 60,
            timer_dead_man: 120,
        }
    }

    #[test]
    fn test_create_email_without_attachment() {
        let config = get_test_config();
        let email_result = create_email(&config);
        assert!(email_result.is_ok());
    }

    #[test]
    fn test_create_email_with_attachment() {
        let mut config = get_test_config();
        // Assuming there's a test file at this path
        config.attachment = Some(PathBuf::from("README.md"));
        let email_result = create_email(&config);
        assert!(email_result.is_ok());
    }
}
