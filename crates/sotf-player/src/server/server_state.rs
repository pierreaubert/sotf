use super::generate::generate_pairing_nonce;
use crate::federation_config::{self, ServerConfig};
use crate::library::MusicLibrary;
use crate::library_scanner::{LibraryScanMessage, LibraryScanner};
use crate::player::Player;
use crate::queue::Queue;
use crate::sotf_server_event::{EventBroadcaster, SotfServerEvent};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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
    /// Prevents overlapping library scans from racing database reloads/events.
    pub(super) library_scan_active: AtomicBool,
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

    /// Advance the library cache version and notify remote clients.
    pub(super) fn mark_library_changed(&self) -> u64 {
        let library_version = self.library_version.fetch_add(1, Ordering::Relaxed) + 1;
        self.broadcast(SotfServerEvent::LibraryChanged { library_version });
        library_version
    }

    /// Notify remote clients about library scanner progress.
    pub(super) fn report_scanner_progress(&self, done: usize, total: usize) {
        self.broadcast(SotfServerEvent::ScannerProgress { done, total });
    }

    /// Reload the durable library and notify remote clients that caches are stale.
    pub(super) fn reload_library_from_database(&self) -> Result<u64, String> {
        let mut library =
            MusicLibrary::with_database().map_err(|e| format!("failed to open library DB: {e}"))?;
        library
            .load_from_database()
            .map_err(|e| format!("failed to reload library DB: {e}"))?;
        *self.library.lock() = library;
        Ok(self.mark_library_changed())
    }

    /// Start a background library scan and bridge scanner messages to SSE.
    pub(super) fn start_library_scan(state: &Arc<Self>, force: bool) -> Result<(), String> {
        let directories: Vec<_> = state
            .library
            .lock()
            .directories
            .iter()
            .map(|directory| directory.path.clone())
            .collect();
        if directories.is_empty() {
            return Err("no library directories configured".to_string());
        }

        state
            .library_scan_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "library scan already running".to_string())?;

        let state_for_thread = Arc::clone(state);
        std::thread::Builder::new()
            .name("sotf-library-scan-events".to_string())
            .spawn(move || {
                let scanner = if force {
                    LibraryScanner::start_force(directories)
                } else {
                    LibraryScanner::start(directories)
                };

                loop {
                    while let Some(message) = scanner.try_recv() {
                        if state_for_thread.handle_library_scan_message(message) {
                            state_for_thread
                                .library_scan_active
                                .store(false, Ordering::Release);
                            return;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            })
            .map_err(|e| {
                state.library_scan_active.store(false, Ordering::Release);
                format!("failed to start library scan event bridge: {e}")
            })?;

        Ok(())
    }

    fn handle_library_scan_message(&self, message: LibraryScanMessage) -> bool {
        match message {
            LibraryScanMessage::Progress {
                tracks,
                total_files,
                ..
            } => {
                self.report_scanner_progress(tracks, total_files.max(tracks));
                false
            }
            LibraryScanMessage::Complete { tracks, .. } => {
                self.report_scanner_progress(tracks, tracks);
                if let Err(err) = self.reload_library_from_database() {
                    self.broadcast(SotfServerEvent::Error { message: err });
                }
                true
            }
            LibraryScanMessage::Error { message } => {
                self.broadcast(SotfServerEvent::Error { message });
                true
            }
        }
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
