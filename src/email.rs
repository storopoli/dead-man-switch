//! Email sending capabilities of the Dead Man's Switch.

use std::{fs, str::FromStr};

use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    Address, Message, SmtpTransport, Transport,
};
use thiserror::Error;

use crate::config::{Config, Email};

#[derive(Error, Debug)]
pub enum EmailError {
    #[error("Failed to send email")]
    SendError,
    #[error("Failed to create email")]
    CreateError,
    #[error("Failed to read attachment")]
    ReadError,
}

impl Config {
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
    pub fn send_email(&self, email_type: Email) -> Result<(), EmailError> {
        let email = self.create_email(email_type)?;

        // SMTP client setup
        let creds = Credentials::new(self.username.clone(), self.password.clone());
        let tls = TlsParameters::new_rustls(self.smtp_server.clone())
            .map_err(|_| EmailError::SendError)?;
        let mailer = SmtpTransport::relay(&self.smtp_server)
            .map_err(|_| EmailError::SendError)?
            .port(self.smtp_port)
            .credentials(creds)
            .tls(Tls::Required(tls))
            .build();

        //Tries to Send the email
        match mailer.send(&email) {
            Ok(_) => Ok(()),
            Err(_) => return Err(EmailError::SendError),
        }
    }
    /// Create the email to send.
    ///
    /// If an attachment is provided, the email will be created with the attachment.
    fn create_email(&self, email_type: Email) -> Result<Message, EmailError> {
        // Guaranteed config values
        let from = Mailbox::new(
            None,
            self.from.parse().map_err(|_| EmailError::CreateError)?,
        );
        // Adjust the email to based on the email type
        let to = match email_type {
            Email::Warning => Address::from_str(&self.from).map_err(|_| EmailError::CreateError)?,
            Email::DeadMan => Address::from_str(&self.to).map_err(|_| EmailError::CreateError)?,
        };
        let to = Mailbox::new(None, to);

        // Adjust the email builder based on the email type
        let email_builder = Message::builder().from(from).to(to);
        let email_builder = match email_type {
            Email::Warning => email_builder.subject(&self.subject_warning),
            Email::DeadMan => email_builder.subject(&self.subject),
        };

        // Prepare the email body
        let text_part =
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(match email_type {
                    Email::Warning => self.message_warning.clone(),
                    Email::DeadMan => self.message.clone(),
                });

        // Conditionally add the attachment for DeadMan email type
        if let Email::DeadMan = email_type {
            if let Some(attachment) = &self.attachment {
                let filename = attachment
                    .file_name()
                    .ok_or_else(|| EmailError::ReadError)?
                    .to_string_lossy();
                let filebody = fs::read(attachment).map_err(|_| EmailError::ReadError)?;
                let content_type = ContentType::parse(
                    mime_guess::from_path(attachment)
                        .first_or_octet_stream()
                        .as_ref(),
                )
                .map_err(|_| EmailError::ReadError)?;

                // Create the attachment part
                let attachment_part =
                    Attachment::new(filename.to_string()).body(filebody, content_type);

                // Construct and return the email with the attachment
                let email = email_builder
                    .multipart(
                        MultiPart::mixed()
                            .singlepart(text_part)
                            .singlepart(attachment_part),
                    )
                    .map_err(|_| EmailError::CreateError)?;
                return Ok(email);
            }
        }

        // For Warning email type or DeadMan without an attachment
        match email_builder.singlepart(text_part) {
            Ok(email) => Ok(email),
            Err(_) => Err(EmailError::CreateError),
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
        let email_result = config.create_email(Email::Warning);
        assert!(email_result.is_ok());
        let email_result = config.create_email(Email::DeadMan);
        assert!(email_result.is_ok());
    }

    #[test]
    fn test_create_email_with_attachment() {
        let mut config = get_test_config();
        // Assuming there's a test file at this path
        config.attachment = Some(PathBuf::from("README.md"));
        let email_result = config.create_email(Email::Warning);
        assert!(email_result.is_ok());
        let email_result = config.create_email(Email::DeadMan);
        assert!(email_result.is_ok());
    }
}
