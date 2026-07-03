use super::api::api_error_response;
use super::consts::API_MAX_CONCURRENT_CONNECTIONS;
use super::dlna::dlna_advertised_ipv4;
use super::dlna::dlna_server_url_for_bind;
use super::dlna_library_adapter::DlnaLibraryAdapter;
use super::generate::ensure_sotf_api_connection_config;
use super::handle::handle_sotf_api_connection;
use super::misc::get_local_ipv4;
use super::misc::initial_trusted_client_fingerprints;
use super::mpd::mpd_settings_to_config;
use super::mpd_player_adapter::MpdPlayerAdapter;
use super::server_state::ServerState;
use super::server_state::build_mpd_tls_acceptor;
use super::validate::validate_server_mode_config;
use super::validate::validate_sotf_api_token;
use crate::federation_config::{self, SotfApiSettings};
use crate::lan_discovery::run_sotf_lan_discovery;
use crate::library::MusicLibrary;
use crate::player::Player;
use crate::queue::Queue;
use parking_lot::Mutex;
use sotf_dlna::{DlnaDevice, DlnaMediaServer, MediaServerAdapter};
use sotf_mpd::{MpdServer, PlayerAdapter};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

pub(super) async fn run_sotf_api_server(
    settings: SotfApiSettings,
    state: Arc<ServerState>,
    listener: TcpListener,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let auth_token = validate_sotf_api_token(&settings)?;
    let connection_slots = Arc::new(Semaphore::new(API_MAX_CONCURRENT_CONNECTIONS));

    loop {
        tokio::select! {
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                let (mut stream, peer_addr) = accepted.map_err(|e| format!("accept: {e}"))?;
                let Ok(slot) = Arc::clone(&connection_slots).try_acquire_owned() else {
                    log::warn!("[server] rejecting SOTF API connection from {peer_addr}: connection limit reached");
                    let response = api_error_response(503, "too many concurrent connections");
                    let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, &response).await;
                    let _ = tokio::io::AsyncWriteExt::shutdown(&mut stream).await;
                    continue;
                };
                let state = Arc::clone(&state);
                let settings = settings.clone();
                let auth_token = auth_token.clone();
                tokio::spawn(async move {
                    let _slot = slot;
                    handle_sotf_api_connection(stream, peer_addr, state, settings, auth_token)
                        .await;
                });
            }
        }
    }

    Ok(())
}

/// Run the app in headless server mode.
///
/// Loads the music library from the database, ensures the SOTF API is enabled,
/// starts enabled servers (SOTF API, MPD, DLNA), and blocks until a shutdown
/// signal (SIGINT/SIGTERM) is received.
pub fn run_server_mode() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::config::load_server_config()?;
    if ensure_sotf_api_connection_config(&mut config) {
        match crate::config::save_server_config(&config) {
            Ok(()) => {
                log::info!("[server] Enabled SOTF API defaults in server config");
                eprintln!(
                    "SOTF API enabled on {}:{}",
                    config.api.bind_address, config.api.port
                );
            }
            Err(err) => {
                log::warn!("[server] Failed to persist SOTF API defaults: {}", err);
                eprintln!("Warning: could not save SOTF API defaults: {err}");
            }
        }
    }
    let config_dir =
        crate::config::get_app_config_dir().ok_or("Could not determine config directory")?;
    let trusted_clients = sotf_tls::TrustedClientStore::load(&config_dir)?;

    validate_server_mode_config(&config, &trusted_clients)?;

    // Load library from database
    let mut library = MusicLibrary::with_database()?;
    library.load_from_database()?;
    let album_count = library.albums.len();
    log::info!("[server] Library loaded: {} albums", album_count);
    eprintln!("Library loaded: {} albums", album_count);

    let player = Player::new();
    let event_broadcaster = crate::sotf_server_event::new_event_broadcaster(64);

    // Load or generate server certificate
    let cert_store = sotf_tls::CertStore::load_or_generate(&config_dir)?;
    let server_fingerprint = cert_store.server_fingerprint();

    log::info!("[server] Trusted clients loaded: {}", trusted_clients.len());
    let initial_mpd_trusted_client_fingerprints =
        initial_trusted_client_fingerprints(&config, &trusted_clients);
    if config.mpd.enabled
        && config.mpd.tls_enabled
        && config.mpd.auth_mode == federation_config::MpdAuthMode::Certificate
        && initial_mpd_trusted_client_fingerprints.is_empty()
    {
        eprintln!(
            "MPD certificate auth has no trusted clients yet. MPD will listen, but clients must pair through the SOTF API before they can connect."
        );
        log::warn!("[server] MPD mTLS starting with no trusted client fingerprints");
    }
    let trusted_client_fingerprints = Arc::new(std::sync::Mutex::new(
        initial_mpd_trusted_client_fingerprints,
    ));

    let state = Arc::new(ServerState {
        player: Mutex::new(player),
        library: Mutex::new(library),
        queue: Mutex::new(Queue::new()),
        playlist_version: std::sync::atomic::AtomicU32::new(1),
        library_version: std::sync::atomic::AtomicU64::new(1),
        events: event_broadcaster,
        library_scan_active: std::sync::atomic::AtomicBool::new(false),
        pairing_mode: std::sync::atomic::AtomicBool::new(false),
        pairing_nonce: parking_lot::Mutex::new(String::new()),
        trusted_clients: parking_lot::Mutex::new(trusted_clients),
        trusted_client_fingerprints,
        server_fingerprint: server_fingerprint.clone(),
    });

    let mpd_tls_acceptor = if config.mpd.enabled && config.mpd.tls_enabled {
        Some(build_mpd_tls_acceptor(&config, &cert_store, &state)?)
    } else {
        None
    };

    if config.api.enabled {
        validate_sotf_api_token(&config.api)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    }

    // Build a tokio runtime for the async servers
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        // Register signal handler
        let tx = shutdown_tx.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            log::info!("[server] Shutdown signal received");
            eprintln!("\nShutting down...");
            let _ = tx.send(true);
        });

        let mut handles = Vec::new();

        // Start MPD server
        if config.mpd.enabled {
            let mpd_config = mpd_settings_to_config(&config, &state);
            let adapter: Arc<dyn PlayerAdapter> = Arc::new(MpdPlayerAdapter {
                state: Arc::clone(&state),
            });
            let mut server = MpdServer::with_config(mpd_config, adapter);
            if let Some(acceptor) = mpd_tls_acceptor.clone() {
                server.set_tls_acceptor(acceptor);
            }
            let cancel = shutdown_rx.clone();

            eprintln!(
                "MPD server listening on {}:{}",
                config.mpd.bind_address, config.mpd.port
            );

            handles.push(tokio::spawn(async move {
                if let Err(e) = server.run(cancel).await {
                    log::error!("[server] MPD server error: {}", e);
                    eprintln!("MPD server error: {}", e);
                }
            }));
        }

        // Start DLNA server
        if config.dlna.enabled {
            let device = DlnaDevice::new_server(&config.dlna.friendly_name, config.dlna.port);
            let adapter: Arc<dyn MediaServerAdapter> = Arc::new(DlnaLibraryAdapter {
                state: Arc::clone(&state),
            });
            let server = DlnaMediaServer::new(device, adapter);
            let cancel = shutdown_rx.clone();
            let bind_address = config.dlna.bind_address.clone();
            let local_ip = dlna_advertised_ipv4(&bind_address);
            let dlna_url = dlna_server_url_for_bind(&bind_address, config.dlna.port);

            eprintln!(
                "DLNA server '{}' listening on {}:{} (URL: {})",
                config.dlna.friendly_name, bind_address, config.dlna.port, dlna_url
            );

            handles.push(tokio::spawn(async move {
                if let Err(e) = server.run(&bind_address, local_ip, cancel).await {
                    log::error!("[server] DLNA server error: {}", e);
                    eprintln!("DLNA server error: {}", e);
                }
            }));
        }

        // Start SOTF LAN control API
        if config.api.enabled {
            let api_config = config.api.clone();
            let cancel = shutdown_rx.clone();
            let api_state = Arc::clone(&state);
            let api_bind_addr = format!("{}:{}", api_config.bind_address, api_config.port);
            if let Some(api_listener) = match TcpListener::bind(&api_bind_addr).await {
                Ok(listener) => Some(listener),
                Err(e) => {
                    log::error!("[server] SOTF API bind error: {}", e);
                    eprintln!("SOTF API bind error on {api_bind_addr}: {e}");
                    let _ = shutdown_tx.send(true);
                    None
                }
            } {
                eprintln!(
                    "SOTF API '{}' listening on {}:{}",
                    api_config.friendly_name, api_config.bind_address, api_config.port
                );

                let (api_discovery_tx, api_discovery_rx) = tokio::sync::watch::channel(false);
                let global_cancel = shutdown_rx.clone();
                let discovery_cancel_tx = api_discovery_tx.clone();
                tokio::spawn(async move {
                    let mut global_cancel = global_cancel;
                    let _ = global_cancel.changed().await;
                    let _ = discovery_cancel_tx.send(true);
                });

                let api_discovery_tx_on_exit = api_discovery_tx.clone();
                handles.push(tokio::spawn(async move {
                    if let Err(e) =
                        run_sotf_api_server(api_config, api_state, api_listener, cancel).await
                    {
                        log::error!("[server] SOTF API server error: {}", e);
                        eprintln!("SOTF API server error: {}", e);
                    }
                    let _ = api_discovery_tx_on_exit.send(true);
                }));

                let discovery_config = config.api.clone();
                let discovery_ip = get_local_ipv4();
                let pairing_enabled = state
                    .pairing_mode
                    .load(std::sync::atomic::Ordering::Relaxed);
                eprintln!(
                    "SOTF API discovery advertising _sotf._tcp for {}:{}",
                    discovery_ip, discovery_config.port
                );
                handles.push(tokio::spawn(async move {
                    if let Err(e) = run_sotf_lan_discovery(
                        discovery_config,
                        discovery_ip,
                        pairing_enabled,
                        api_discovery_rx,
                    )
                    .await
                    {
                        log::warn!("[server] SOTF API discovery error: {}", e);
                        eprintln!("SOTF API discovery warning: {}", e);
                    }
                }));
            }
        }

        eprintln!("Server mode running. Press Ctrl-C to stop.");

        // Wait for all server tasks to finish (they exit on shutdown signal)
        for handle in handles {
            let _ = handle.await;
        }
    });

    Ok(())
}
