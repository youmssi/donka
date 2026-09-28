//! Outgoing email.
//!
//! Modules depend on the [`Mailer`] trait; `apps/app` wires the SMTP
//! implementation and tests use [`testing::RecordingMailer`].

use async_trait::async_trait;
use lettre::message::{header::ContentType, Mailbox};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::time::Duration;

/// A plain-text email to one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("invalid mail settings: {0}")]
    InvalidSettings(String),
    #[error("invalid email: {0}")]
    InvalidEmail(String),
    /// Worth retrying: connection trouble or a temporary (4xx) server answer.
    #[error("sending failed: {0}")]
    Send(String),
    /// The server refused the message for good (5xx), e.g. an unknown mailbox.
    #[error("rejected by the mail server: {0}")]
    Rejected(String),
}

impl MailError {
    /// Sending the same email again cannot succeed.
    pub fn is_permanent(&self) -> bool {
        !matches!(self, Self::Send(_))
    }
}

#[async_trait]
pub trait Mailer: Send + Sync {
    async fn send(&self, email: &Email) -> Result<(), MailError>;
}

/// Gives up on an unresponsive server so one stuck send does not hold the outbox.
const SMTP_TIMEOUT: Duration = Duration::from_secs(30);

pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpMailer {
    /// `url` follows lettre's format: `smtp://host:25` (no TLS),
    /// `smtp://user:pass@host:587?tls=required` (STARTTLS) or
    /// `smtps://user:pass@host:465` (implicit TLS). Call it inside a Tokio
    /// runtime: the connection pool starts a background task.
    pub fn new(url: &str, from: &str) -> Result<Self, MailError> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url)
            // The URL may hold credentials: never echo it.
            .map_err(|_| {
                MailError::InvalidSettings(
                    "the SMTP URL must look like smtp://host:port, smtp://user:pass@host:587?tls=required or smtps://user:pass@host:465".into(),
                )
            })?
            .timeout(Some(SMTP_TIMEOUT))
            .build();
        let from = from
            .parse()
            .map_err(|err| MailError::InvalidSettings(format!("sender address: {err}")))?;
        Ok(Self { transport, from })
    }
}

#[async_trait]
impl Mailer for SmtpMailer {
    async fn send(&self, email: &Email) -> Result<(), MailError> {
        let to: Mailbox = email
            .to
            .parse()
            .map_err(|err| MailError::InvalidEmail(format!("{err}")))?;
        let message = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(&email.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(email.text.clone())
            .map_err(|err| MailError::InvalidEmail(err.to_string()))?;
        self.transport
            .send(message)
            .await
            .map(|_| ())
            .map_err(|err| {
                if err.is_permanent() {
                    MailError::Rejected(err.to_string())
                } else {
                    MailError::Send(err.to_string())
                }
            })
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod testing {
    use super::{async_trait, Email, MailError, Mailer};
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Mutex;

    const DELIVER: u8 = 0;
    const UNREACHABLE: u8 = 1;
    const REJECT: u8 = 2;

    /// Keeps sent emails in memory; can be told to fail like an unreachable
    /// server or one that refuses the message.
    #[derive(Default)]
    pub struct RecordingMailer {
        sent: Mutex<Vec<Email>>,
        mode: AtomicU8,
    }

    impl RecordingMailer {
        pub fn sent(&self) -> Vec<Email> {
            self.sent
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }

        /// Transient failure (retried) while `true`.
        pub fn set_unreachable(&self, unreachable: bool) {
            let mode = if unreachable { UNREACHABLE } else { DELIVER };
            self.mode.store(mode, Ordering::SeqCst);
        }

        /// Permanent failure (not retried) while `true`.
        pub fn set_rejecting(&self, rejecting: bool) {
            let mode = if rejecting { REJECT } else { DELIVER };
            self.mode.store(mode, Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl Mailer for RecordingMailer {
        async fn send(&self, email: &Email) -> Result<(), MailError> {
            match self.mode.load(Ordering::SeqCst) {
                UNREACHABLE => return Err(MailError::Send("connection refused".into())),
                REJECT => return Err(MailError::Rejected("550 mailbox unavailable".into())),
                _ => {}
            }
            self.sent
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(email.clone());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn accepts_the_three_connection_styles() {
        for url in [
            "smtp://localhost:1025",
            "smtp://user:pass@mail.bank.example:587?tls=required",
            "smtps://user:pass@mail.bank.example",
        ] {
            assert!(
                SmtpMailer::new(url, "Donka <donka@bank.example>").is_ok(),
                "{url}"
            );
        }
    }

    #[test]
    fn only_transport_trouble_is_worth_retrying() {
        assert!(!MailError::Send("timed out".into()).is_permanent());
        assert!(MailError::Rejected("550".into()).is_permanent());
        assert!(MailError::InvalidEmail("no @".into()).is_permanent());
    }

    #[tokio::test]
    async fn rejects_bad_settings_without_echoing_credentials() {
        let Err(MailError::InvalidSettings(message)) =
            SmtpMailer::new("http://user:hunter2@host", "donka@bank.example")
        else {
            panic!("expected invalid settings");
        };
        assert!(!message.contains("hunter2"));
        assert!(matches!(
            SmtpMailer::new("smtp://localhost:1025", "not an address"),
            Err(MailError::InvalidSettings(_))
        ));
    }
}
