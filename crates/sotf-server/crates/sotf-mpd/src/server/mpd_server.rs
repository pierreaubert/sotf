use super::line_read::handle_session;
use super::misc::wait_for_cancel;
use super::mpd_server_config::MpdServerConfig;
use super::types::MpdAuthMode;
use crate::handler::PlayerAdapter;
use std::sync::Arc;
use tokio::net::TcpListener;

/// The MPD protocol server.
pub struct MpdServer {
    pub(super) config: MpdServerConfig,
    pub(super) adapter: Arc<dyn PlayerAdapter>,
    #[cfg(feature = "tls")]
    pub(super) tls_acceptor: Option<tokio_rustls::TlsAcceptor>,
}

impl MpdServer {
    #[must_use]
    pub fn new(adapter: Arc<dyn PlayerAdapter>) -> Self {
        Self {
            config: MpdServerConfig::default(),
            adapter,
            #[cfg(feature = "tls")]
            tls_acceptor: None,
        }
    }

    #[must_use]
    pub fn with_config(config: MpdServerConfig, adapter: Arc<dyn PlayerAdapter>) -> Self {
        Self {
            config,
            adapter,
            #[cfg(feature = "tls")]
            tls_acceptor: None,
        }
    }

    /// Set the TLS acceptor for encrypted connections.
    ///
    /// When set and `config.tls_enabled` is true, all connections are TLS-wrapped.
    #[cfg(feature = "tls")]
    pub fn set_tls_acceptor(&mut self, acceptor: tokio_rustls::TlsAcceptor) {
        self.tls_acceptor = Some(acceptor);
    }

    /// Start the MPD server. This runs until the cancellation token is triggered.
    ///
    /// # Errors
    /// Returns an error if the server cannot bind to the configured address.
    pub async fn run(&self, cancel: tokio::sync::watch::Receiver<bool>) -> Result<(), String> {
        let addr = format!("{}:{}", self.config.bind_address, self.config.port);

        // Refuse to start in a misconfigured TLS state: if the integrator asked
        // for TLS (`tls_enabled = true`) but never installed an acceptor, every
        // connection would silently fall through to plaintext while the
        // surrounding configuration suggests otherwise. That is the worst
        // failure mode for a network-facing service, so bail out instead of
        // silently accepting cleartext on `0.0.0.0:6600`.
        #[cfg(feature = "tls")]
        {
            if self.config.tls_enabled && self.tls_acceptor.is_none() {
                return Err(format!(
                    "MPD configured with tls_enabled=true but no TLS acceptor installed; \
                     refusing to bind {addr} to avoid silently accepting plaintext. \
                     Call MpdServer::set_tls_acceptor() before run(), or set tls_enabled=false."
                ));
            }
        }

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind to {addr}: {e}"))?;

        let tls_mode = self.tls_mode_label();
        log::info!("[MPD] Listening on {addr} ({tls_mode})");

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer)) => {
                            log::debug!("[MPD] New connection from {peer}");
                            let adapter = Arc::clone(&self.adapter);
                            let cancel = cancel.clone();
                            // mTLS: client is authenticated by TLS handshake, no password needed.
                            let password = if self.config.auth_mode == MpdAuthMode::Certificate {
                                None
                            } else {
                                self.config.password.clone()
                            };

                            #[cfg(feature = "tls")]
                            let tls_acceptor = if self.config.tls_enabled {
                                self.tls_acceptor.clone()
                            } else {
                                None
                            };

                            tokio::spawn(async move {
                                let result = {
                                    #[cfg(feature = "tls")]
                                    {
                                        if let Some(acceptor) = tls_acceptor {
                                            match sotf_tls::tls_accept(&acceptor, stream).await {
                                                Ok(tls_stream) => {
                                                    let (reader, writer) = tokio::io::split(tls_stream);
                                                    handle_session(reader, writer, adapter, cancel, password).await
                                                }
                                                Err(e) => {
                                                    log::debug!("[MPD] TLS handshake failed from {peer}: {e}");
                                                    return;
                                                }
                                            }
                                        } else {
                                            let (reader, writer) = stream.into_split();
                                            handle_session(reader, writer, adapter, cancel, password).await
                                        }
                                    }
                                    #[cfg(not(feature = "tls"))]
                                    {
                                        let (reader, writer) = stream.into_split();
                                        handle_session(reader, writer, adapter, cancel, password).await
                                    }
                                };

                                if let Err(e) = result {
                                    log::debug!("[MPD] Session error from {peer}: {e}");
                                }
                                log::debug!("[MPD] Connection closed from {peer}");
                            });
                        }
                        Err(e) => {
                            log::warn!("[MPD] Accept error: {e}");
                        }
                    }
                }
                _ = wait_for_cancel(&cancel) => {
                    log::info!("[MPD] Server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    pub(super) fn tls_mode_label(&self) -> &'static str {
        #[cfg(feature = "tls")]
        {
            if self.config.tls_enabled && self.tls_acceptor.is_some() {
                return "TLS";
            }
        }
        "plain"
    }
}
