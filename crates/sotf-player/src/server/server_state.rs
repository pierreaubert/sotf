use super::generate::generate_pairing_nonce;
use crate::federation_config::{self, ServerConfig};
use crate::library::MusicLibrary;
use crate::player::Player;
use crate::queue::Queue;
use crate::sotf_server_event::{EventBroadcaster, SotfServerEvent};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;

/// Shared state for the headless server adapters.
pub(super) struct ServerState {
    pub(super) player: Mutex<Player>,
    pub(super) library: Mutex<MusicLibrary>,
    pub(super) queue: Mutex<Queue>,
    /// Playlist version counter — incremented on every queue mutation.
    pub(super) playlist_version: std::sync::atomic::AtomicU32,
    /// Library version counter for remote client cache invalidation.
    pub(super) library_version: std::sync::atomic::AtomicU64,
    /// Broadcast channel for server-sent events.
    pub(super) events: EventBroadcaster,
    /// Whether pairing mode is currently open.
    pub(super) pairing_mode: std::sync::atomic::AtomicBool,
    /// Pairing nonce/short code — valid only while pairing_mode is true.
    pub(super) pairing_nonce: parking_lot::Mutex<String>,
    /// Trusted client certificate store for mTLS.
    pub(super) trusted_clients: parking_lot::Mutex<sotf_tls::TrustedClientStore>,
    /// Live trusted fingerprints used by the MPD mTLS verifier.
    pub(super) trusted_client_fingerprints: Arc<std::sync::Mutex<HashSet<String>>>,
    /// Server TLS certificate fingerprint (for QR code / manual verification).
    pub(super) server_fingerprint: String,
}

impl ServerState {
    /// Broadcast a server event to all connected clients.
    /// Silently ignored if there are no active subscribers.
    pub(super) fn broadcast(&self, event: SotfServerEvent) {
        let _ = self.events.send(event);
    }

    /// Generate a fresh pairing nonce.
    pub(super) fn refresh_pairing_nonce(&self) -> String {
        let nonce = generate_pairing_nonce();
        *self.pairing_nonce.lock() = nonce.clone();
        nonce
    }
}

pub(super) fn insert_live_trusted_client(
    state: &ServerState,
    fingerprint: &str,
) -> Result<(), String> {
    let mut trusted = state
        .trusted_client_fingerprints
        .lock()
        .map_err(|e| format!("trusted fingerprint lock poisoned: {e}"))?;
    trusted.insert(fingerprint.to_string());
    Ok(())
}

pub(super) fn remove_live_trusted_client(
    state: &ServerState,
    fingerprint: &str,
) -> Result<(), String> {
    let mut trusted = state
        .trusted_client_fingerprints
        .lock()
        .map_err(|e| format!("trusted fingerprint lock poisoned: {e}"))?;
    trusted.remove(fingerprint);
    Ok(())
}

pub(super) fn build_mpd_tls_acceptor(
    config: &ServerConfig,
    cert_store: &sotf_tls::CertStore,
    state: &Arc<ServerState>,
) -> Result<tokio_rustls::TlsAcceptor, Box<dyn std::error::Error>> {
    let tls_config = match config.mpd.auth_mode {
        federation_config::MpdAuthMode::Certificate => {
            let trusted = Arc::clone(&state.trusted_client_fingerprints);
            sotf_tls::build_server_tls_config_mtls(
                cert_store.cert_clone(),
                cert_store.key_clone(),
                trusted,
            )?
        }
        federation_config::MpdAuthMode::Password => {
            if config
                .mpd
                .password
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            {
                return Err("MPD password authentication requires a non-empty password".into());
            }
            sotf_tls::build_server_tls_config(cert_store.cert_clone(), cert_store.key_clone())?
        }
    };

    eprintln!(
        "MPD TLS certificate fingerprint: {}",
        cert_store.server_fingerprint()
    );
    Ok(tokio_rustls::TlsAcceptor::from(tls_config))
}
