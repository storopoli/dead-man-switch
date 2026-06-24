//! Tor (arti) integration for the Dead Man's Switch.
//!
//! This module is only compiled when the `tor` feature is enabled. It provides
//! two capabilities, both backed by [arti](https://gitlab.torproject.org/tpo/core/arti):
//!
//! - **Inbound**: bootstrap a [`TorClient`] and launch an onion service so the
//!   web UI can be reached over Tor (see [`bootstrap_tor_client`] and
//!   [`launch_onion_service`]).
//! - **Outbound**: deliver the notification emails over Tor by opening a
//!   [`DataStream`] to the SMTP server and driving lettre's async SMTP
//!   connection over it (see [`send_email_tor`]).
//!
//! arti is kept on its default `native-tls` stack; the only rustls provider in
//! the dependency tree is lettre's `ring`, which is also used for the outbound
//! STARTTLS upgrade. This avoids a `CryptoProvider` conflict.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arti_client::config::CfgPath;
use arti_client::{DataStream, TorClient, TorClientConfig};
use futures::Stream;
use safelog::DisplayRedacted;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tor_hsservice::{HsNickname, OnionServiceConfig, RunningOnionService, StreamRequest};
use tor_rtcompat::PreferredRuntime;

use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::transport::smtp::client::{AsyncSmtpConnection, AsyncTokioStream, TlsParameters};
use lettre::transport::smtp::extension::ClientId;

use crate::config::{Config, Email};
use crate::error::{EmailError, HomeDirError};

/// A bootstrapped Tor client using the preferred (tokio) runtime.
pub type DmsTorClient = TorClient<PreferredRuntime>;

/// Errors for the Tor subsystem.
#[derive(thiserror::Error, Debug)]
pub enum TorError {
    /// Error originating from arti itself (bootstrap, connect, launch).
    #[error(transparent)]
    Arti(#[from] arti_client::Error),

    /// Error from the async SMTP path used for outbound email over Tor.
    #[error(transparent)]
    Smtp(#[from] lettre::transport::smtp::Error),

    /// Error while building the email message.
    #[error(transparent)]
    Email(#[from] EmailError),

    /// Error resolving the app's home/config directory.
    #[error(transparent)]
    HomeDir(#[from] HomeDirError),

    /// A configuration or builder error (nickname, onion config, etc.).
    #[error("tor configuration error: {0}")]
    Config(String),

    /// The onion service is disabled in arti's configuration, or no Tor client
    /// was available when one was required.
    #[error("onion service is disabled or no Tor client is available")]
    ServiceDisabled,
}

/// A launched onion service together with its stream of incoming requests.
///
/// The held [`Arc<RunningOnionService>`] must stay alive for the service to
/// keep running: dropping it tears the service down.
pub struct OnionEndpoint {
    /// The running onion service. Keep alive to keep serving.
    pub service: Arc<RunningOnionService>,
    /// Stream of accepted application streams; each becomes one HTTP connection.
    pub streams: Pin<Box<dyn Stream<Item = StreamRequest> + Send>>,
}

impl OnionEndpoint {
    /// The stable `<base32>.onion` address, if the identity key is published yet.
    ///
    /// Returns `None` until arti has the service's identity key available.
    pub fn onion_address(&self) -> Option<String> {
        onion_address_of(&self.service)
    }
}

/// The `.onion` address of a running onion service, if available yet.
///
/// `HsId` is redacted by default (safelog) to avoid accidental logging, so we
/// render it unredacted to obtain the actual `.onion` address.
pub fn onion_address_of(service: &RunningOnionService) -> Option<String> {
    service
        .onion_address()
        .map(|id| id.display_unredacted().to_string())
}

/// Directory where arti persists its state and keystore.
///
/// Defaults to a `tor` subdirectory of the app's config directory so the
/// `.onion` address remains stable across restarts.
pub fn default_state_dir() -> Result<PathBuf, HomeDirError> {
    crate::app::file_path("tor")
}

/// Build a [`TorClientConfig`] that stores state and keys under `state_dir`.
fn build_client_config(state_dir: &Path) -> Result<TorClientConfig, TorError> {
    let mut builder = TorClientConfig::builder();
    builder
        .storage()
        .state_dir(CfgPath::new_literal(state_dir.to_path_buf()))
        .cache_dir(CfgPath::new_literal(state_dir.join("cache")));
    builder.build().map_err(|e| TorError::Config(e.to_string()))
}

/// Bootstrap a Tor client, persisting state under `state_dir`
/// (defaults to [`default_state_dir`]).
///
/// Bootstrapping builds a full Tor client (directory manager, guards, circuit
/// pool) and can take several seconds; run it off the critical path. The
/// returned client is cheap to clone and should be reused for both the onion
/// service and outbound email.
pub async fn bootstrap_tor_client(
    state_dir: Option<PathBuf>,
) -> Result<Arc<DmsTorClient>, TorError> {
    let dir = match state_dir {
        Some(dir) => dir,
        None => default_state_dir()?,
    };
    let config = build_client_config(&dir)?;
    // `create_bootstrapped` already returns an `Arc<TorClient<_>>`.
    let client = TorClient::create_bootstrapped(config).await?;
    Ok(client)
}

/// Launch an onion service on an already-bootstrapped client.
///
/// `nickname` identifies the service and is used as the keystore subdirectory,
/// so reusing the same nickname yields the same `.onion` address.
pub fn launch_onion_service(
    client: &DmsTorClient,
    nickname: &str,
) -> Result<OnionEndpoint, TorError> {
    let nickname: HsNickname = nickname
        .parse()
        .map_err(|e| TorError::Config(format!("invalid onion service nickname: {e}")))?;
    let svc_config = OnionServiceConfig::builder()
        .nickname(nickname)
        .build()
        .map_err(|e| TorError::Config(e.to_string()))?;

    let (service, rend_requests) = client
        .launch_onion_service(svc_config)?
        .ok_or(TorError::ServiceDisabled)?;

    let streams = Box::pin(tor_hsservice::handle_rend_requests(rend_requests));
    Ok(OnionEndpoint { service, streams })
}

/// Accept one incoming onion stream, yielding a tokio [`DataStream`].
///
/// Thin wrapper so the web crate need not depend on `tor-cell` directly.
pub async fn accept_stream(request: StreamRequest) -> Result<DataStream, TorError> {
    use tor_cell::relaycell::msg::Connected;
    request
        .accept(Connected::new_empty())
        .await
        .map_err(|e| TorError::Config(e.to_string()))
}

/// Adapts an arti [`DataStream`] into a stream lettre can drive.
///
/// arti's [`DataStream`] is a tokio `AsyncRead`/`AsyncWrite` but is not a
/// socket, so it lacks `peer_addr`. lettre only uses `peer_addr` for
/// diagnostics, so a synthetic address is fine.
#[derive(Debug)]
pub struct TorSmtpStream(pub DataStream);

impl AsyncRead for TorSmtpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for TorSmtpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

impl AsyncTokioStream for TorSmtpStream {
    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        // DataStream is not a socket; lettre uses this only for diagnostics.
        Ok(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0))
    }
}

/// Send a notification email over Tor.
///
/// Opens a [`DataStream`] to the configured SMTP server through `client`, then
/// drives lettre's async SMTP connection over it (EHLO → STARTTLS → AUTH →
/// MAIL/RCPT/DATA). The message is built with the same `create_email` helper
/// used by the clearnet path, so behaviour is identical apart from transport.
///
/// Assumes STARTTLS on the submission port (the config default is 587).
pub async fn send_email_tor(
    config: &Config,
    client: &DmsTorClient,
    email_type: Email,
) -> Result<(), TorError> {
    // Reuse the existing message construction.
    let message = config.create_email(email_type)?;
    let envelope = message.envelope().clone();
    let body = message.formatted();

    // Open a Tor stream to the SMTP server.
    let data_stream = client
        .connect((config.smtp_server.as_str(), config.smtp_port))
        .await?;
    let transport: Box<dyn AsyncTokioStream> = Box::new(TorSmtpStream(data_stream));

    // Drive SMTP over the Tor stream.
    let hello = ClientId::default();
    let mut conn = AsyncSmtpConnection::connect_with_transport(transport, &hello).await?;

    let tls = TlsParameters::new_rustls(config.smtp_server.clone())?;
    conn.starttls(tls, &hello).await?;

    let creds = Credentials::new(config.username.clone(), config.password.clone());
    conn.auth(&[Mechanism::Plain, Mechanism::Login], &creds)
        .await?;
    conn.send(&envelope, &body).await?;
    let _ = conn.quit().await;

    Ok(())
}
