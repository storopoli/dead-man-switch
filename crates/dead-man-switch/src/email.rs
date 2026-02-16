//! Email sending capabilities of the Dead Man's Switch.

use crate::config::{Config, Email, attachment_path};
use crate::error::{AddressError, EmailError};

use lettre::{
    Message, SmtpTransport, Transport,
    message::{Attachment, Mailbox, MultiPart, SinglePart, header::ContentType},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use std::fs;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

impl Config {
    pub fn setup_smtp_client(&self) -> Result<SmtpTransport, EmailError> {
        let creds = Credentials::new(self.username.clone(), self.password.clone());
        let tls = TlsParameters::new_rustls(self.smtp_server.clone())?;
        let mailer = SmtpTransport::relay(&self.smtp_server)?
            .port(self.smtp_port)
            .credentials(creds)
            .tls(Tls::Required(tls))
            .build();

        Ok(mailer)
    }

    pub fn check_smtp_connection(&self) -> Result<(), EmailError> {
        let mailer = self.setup_smtp_client()?;
        let exit_flag = Arc::new(AtomicBool::new(false)); // owned by the main thread
        let exit_flag_clone = Arc::clone(&exit_flag); // moved into the spawned thread

        let (tx, rx) = channel();
        thread::spawn(move || {
            let res = mailer.test_connection();
            // suppress the send if exit_flag_clone is true
            if !exit_flag_clone.load(Ordering::SeqCst) {
                let _ = tx.send(res);
            }
        });

        let timeout = Duration::from_secs(self.smtp_check_timeout.unwrap_or(5));
        match rx.recv_timeout(timeout) {
            Ok(Ok(true)) => Ok(()),
            Ok(Ok(false)) => {
                exit_flag.store(true, Ordering::SeqCst);
                Err(EmailError::Timeout)
            }
            Ok(Err(e)) => {
                exit_flag.store(true, Ordering::SeqCst);
                Err(EmailError::SmtpError(e))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                exit_flag.store(true, Ordering::SeqCst);
                Err(EmailError::Timeout)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                exit_flag.store(true, Ordering::SeqCst);
                Err(EmailError::Disconnected)
            }
        }
    }

    /// Send the email using the provided configuration.
    ///
    /// # Errors
    ///
    /// - If the email fails to send.
    /// - If the email cannot be created.
    /// - If the attachment cannot be read.
    ///
    /// # Notes
    ///
    /// If the attachment MIME type cannot be determined, it will default to
    /// `application/octet-stream`.
    pub fn send_email(&self, email_type: Email) -> Result<(), EmailError> {
        let email = self.create_email(email_type)?;
        let mailer = self.setup_smtp_client()?;

        // Send the email
        mailer.send(&email)?;
        Ok(())
    }
    /// Create the email to send.
    ///
    /// If an attachment is provided, the email will be created with the attachment.
    fn create_email(&self, email_type: Email) -> Result<Message, EmailError> {
        // Guaranteed config values
        let from = Mailbox::new(None, self.from.parse()?);
        // Adjust the email to based on the email type
        let to = match email_type {
            Email::Warning => &self.from,
            Email::DeadMan => &self.to,
        };

        // parse the comma‐separated list into a Vec<Mailbox>
        let mailboxes: Result<Vec<Mailbox>, AddressError> = to
            .split(',')
            .map(str::trim)
            .map(|addr| addr.parse::<Mailbox>())
            .collect();
        let mailboxes = mailboxes?;

        // Adjust the email builder based on the email type
        let mut email_builder = Message::builder().from(from);

        // Add recipients
        for mbox in mailboxes {
            email_builder = email_builder.to(mbox);
        }

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
                let attachment_path = attachment_path(self)?;
                let filename = attachment_path
                    .file_name()
                    .ok_or_else(|| IoError::new(IoErrorKind::NotFound, "Failed to get filename"))?
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_config() -> Config {
        Config {
            username: "user@example.com".to_string(),
            password: "password".to_string(),
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_check_timeout: Some(5),
            message: "This is a test message".to_string(),
            message_warning: "This is a test warning message".to_string(),
            subject: "Test Subject".to_string(),
            subject_warning: "Test Warning Subject".to_string(),
            to: "recipient@example.com, recipient2@example.com".to_string(),
            from: "sender@example.com".to_string(),
            attachment: None,
            timer_warning: 60,
            timer_dead_man: 120,
            web_password: "password".to_string(),
            cookie_exp_days: 7,
            log_level: None,
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
        config.attachment = Some("Cargo.toml".into());
        let email_result = config.create_email(Email::Warning);
        assert!(email_result.is_ok());
        let email_result = config.create_email(Email::DeadMan);
        assert!(email_result.is_ok());
    }

    #[test]
    fn test_setup_smtp_client() {
        let config = Config::default();

        // placeholder: just verifying function signature at present
        let client = config.setup_smtp_client();

        assert!(client.is_ok());
    }

    #[test]
    fn test_check_smtp_connection() {
        let mut config = Config::default();
        config.username = "test_username".to_string();
        config.password = "test_password".to_string();
        config.smtp_server = "test_smtp_server".to_string();
        config.smtp_port = 432;
        config.smtp_check_timeout = Some(1);

        // placeholder: just verifying function signature at present
        let client = config.check_smtp_connection();

        assert!(client.is_err());
    }
}
