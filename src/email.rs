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

use crate::config::{Config, Email};

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
pub fn send_email(config: &Config, email_type: Email) -> Result<()> {
    let email = create_email(config, email_type)?;

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
fn create_email(config: &Config, email_type: Email) -> Result<Message> {
    // Guaranteed config values
    let from = Mailbox::new(None, config.from.parse()?);
    let to = Mailbox::new(None, config.to.parse()?);

    // Adjust the email builder based on the email type
    let email_builder = Message::builder().from(from).to(to);
    let email_builder = match email_type {
        Email::Warning => email_builder.subject(&config.subject_warning),
        Email::DeadMan => email_builder.subject(&config.subject),
    };

    // Prepare the email body
    let text_part = SinglePart::builder()
        .header(ContentType::TEXT_PLAIN)
        .body(match email_type {
            Email::Warning => config.message_warning.clone(),
            Email::DeadMan => config.message.clone(),
        });

    // Conditionally add the attachment for DeadMan email type
    if let Email::DeadMan = email_type {
        if let Some(attachment) = &config.attachment {
            let filename = attachment
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Failed to get filename"))?
                .to_string_lossy();
            let filebody = fs::read(attachment)?;
            let content_type = ContentType::parse(
                mime_guess::from_path(attachment)
                    .first_or_octet_stream()
                    .as_ref(),
            )?;

            // Create the attachment part
            let attachment_part =
                Attachment::new(filename.to_string()).body(filebody, content_type);

            // Construct and return the email with the attachment
            let email = email_builder.multipart(
                MultiPart::mixed()
                    .singlepart(text_part)
                    .singlepart(attachment_part),
            )?;
            return Ok(email);
        }
    }

    // For Warning email type or DeadMan without an attachment
    let email = email_builder.singlepart(text_part)?;
    Ok(email)
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
            message_warning: "This is a test warning message".to_string(),
            subject: "Test Subject".to_string(),
            subject_warning: "Test Warning Subject".to_string(),
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
        let email_result = create_email(&config, Email::Warning);
        assert!(email_result.is_ok());
        let email_result = create_email(&config, Email::DeadMan);
        assert!(email_result.is_ok());
    }

    #[test]
    fn test_create_email_with_attachment() {
        let mut config = get_test_config();
        // Assuming there's a test file at this path
        config.attachment = Some(PathBuf::from("README.md"));
        let email_result = create_email(&config, Email::Warning);
        assert!(email_result.is_ok());
        let email_result = create_email(&config, Email::DeadMan);
        assert!(email_result.is_ok());
    }
}
