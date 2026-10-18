//! Typed client/server loopback contract tests.
//!
//! These tests spawn the real SOTF API server on an ephemeral loopback
//! port and drive it through `SotfApiClient` — the same client the GPUI
//! and mobile remote-control surfaces use — so request/response shapes,
//! authentication, queue semantics, pairing, and the SSE event stream are
//! verified across the actual wire instead of only at the handler level.

use super::super::mpd_player_adapter::MpdPlayerAdapter;
use super::super::run::run_sotf_api_server;
use super::misc::{
    album, api_settings, stop_test_api_server, test_state, try_spawn_test_api_server,
};
use crate::sotf_api_client::{SotfApiClient, SotfApiClientError, SotfApiStreamEvent};
use crate::sotf_server_event::SotfServerEvent;
use sotf_mpd::{MpdAuthMode, MpdServer, MpdServerConfig, PlayerAdapter};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

// Generous ceiling for loopback round trips; a healthy local server answers
// in milliseconds, so hitting this bound means the contract is broken.
const LOOPBACK_EVENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

const VALID_FINGERPRINT: &str = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:\
AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";

fn seeded_state() -> Arc<super::super::server_state::ServerState> {
    let state = test_state();
    let mut first = album("Loopback Alpha", "Client Server Band", 2021);
    first.id = Some(42);
    let mut second = album("Loopback Beta", "Client Server Band", 2023);
    second.id = Some(43);
    state.library.lock().albums = vec![first, second];
    state
}

fn client_for(addr: std::net::SocketAddr, token: &str) -> SotfApiClient {
    SotfApiClient::new(format!("http://{addr}"), token).unwrap()
}

#[test]
fn typed_client_browses_library_and_manages_queue_over_loopback() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = seeded_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };
        let client = client_for(addr, "secret");

        let health = client.health().await.unwrap();
        assert!(health.ok);

        let capabilities = client.capabilities().await.unwrap();
        assert!(capabilities.features.events);

        let listed = client.library_albums_page(0, 10, None, None).await.unwrap();
        assert_eq!(listed.total, 2);
        assert_eq!(listed.albums.len(), 2);
        let alpha = listed
            .albums
            .iter()
            .find(|album| album.title == "Loopback Alpha")
            .expect("seeded album must be listed");
        assert_eq!(alpha.id, "id:42");

        let tracks = client.album_tracks(&alpha.id).await.unwrap();
        assert_eq!(tracks.tracks.len(), 1);
        assert_eq!(
            tracks.tracks[0].title.as_deref(),
            Some("Loopback Alpha track")
        );

        let added = client.queue_add_album("id:42", false).await.unwrap();
        assert!(added.ok);
        assert_eq!(added.command, "queue.add-album");

        let queue = client.queue().await.unwrap();
        assert_eq!(queue.items.len(), 1);
        assert_eq!(queue.items[0].album.as_deref(), Some("Loopback Alpha"));

        let volume = client.set_volume(37).await.unwrap();
        assert!(volume.ok);
        let state_after = client.state().await.unwrap();
        assert_eq!(state_after.playback.volume, 37);
        assert_eq!(state_after.playback.playlist_length, 1);
        assert_eq!(state_after.library.albums, 2);

        let cleared = client.queue_clear().await.unwrap();
        assert!(cleared.ok);
        assert!(client.queue().await.unwrap().items.is_empty());

        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn typed_client_rejects_wrong_token_but_health_stays_public() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = seeded_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };
        let client = client_for(addr, "wrong-token");

        assert!(client.health().await.unwrap().ok);

        let err = client.state().await.unwrap_err();
        assert!(
            matches!(err, SotfApiClientError::Api { status: 401, .. }),
            "expected typed 401 error, got {err:?}"
        );
        assert!(!err.to_string().contains("wrong-token"));

        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn typed_client_receives_queue_event_over_sse() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = seeded_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };
        let client = client_for(addr, "secret");

        let mut events = client.events_stream().await.unwrap();
        client.queue_add_album("id:43", false).await.unwrap();

        let deadline = tokio::time::Instant::now() + LOOPBACK_EVENT_TIMEOUT;
        let mut saw_queue_event = false;
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline - tokio::time::Instant::now();
            match tokio::time::timeout(remaining, events.recv()).await {
                Ok(Some(Ok(SotfApiStreamEvent::Server(SotfServerEvent::QueueChanged {
                    ..
                })))) => {
                    saw_queue_event = true;
                    break;
                }
                Ok(Some(Ok(SotfApiStreamEvent::State(state))))
                    if state.playback.playlist_length >= 1 =>
                {
                    saw_queue_event = true;
                    break;
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => break,
            }
        }
        assert!(saw_queue_event, "queue mutation must surface over SSE");

        drop(events);
        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn typed_client_pairing_flow_over_loopback() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = seeded_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };
        let client = client_for(addr, "secret");

        assert!(!client.pairing_status().await.unwrap().pairing_enabled);

        let enabled = client.enable_pairing().await.unwrap();
        assert!(enabled.ok && enabled.pairing_enabled);
        let nonce = enabled.nonce.expect("enabling pairing returns a nonce");

        let rejected = client
            .complete_pairing("wrong-nonce", VALID_FINGERPRINT, "Loopback Phone")
            .await
            .unwrap_err();
        assert!(matches!(
            rejected,
            SotfApiClientError::Api { status: 400, .. }
        ));

        let completed = client
            .complete_pairing(&nonce, VALID_FINGERPRINT, "Loopback Phone")
            .await
            .unwrap();
        assert!(completed.ok);

        let trusted = client.trusted_clients().await.unwrap();
        assert_eq!(trusted.clients.len(), 1);
        assert_eq!(trusted.clients[0].fingerprint, VALID_FINGERPRINT);
        assert_eq!(trusted.clients[0].name, "Loopback Phone");

        assert!(
            client
                .revoke_trusted_client(VALID_FINGERPRINT)
                .await
                .unwrap()
                .ok
        );
        assert!(client.trusted_clients().await.unwrap().clients.is_empty());

        assert!(client.disable_pairing().await.unwrap().ok);

        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn typed_client_recovers_after_server_restart_on_same_port() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = seeded_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };
        let client = client_for(addr, "secret");
        assert!(client.health().await.unwrap().ok);

        stop_test_api_server(shutdown_tx, server_handle).await;
        assert!(client.state().await.is_err());

        // Rebind the same loopback address; the OS may need a moment to
        // release the listener after shutdown.
        let mut listener = None;
        for _ in 0..50 {
            match TcpListener::bind(addr).await {
                Ok(bound) => {
                    listener = Some(bound);
                    break;
                }
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
            }
        }
        let listener = listener.expect("restarted server must rebind its address");
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let server_handle = tokio::spawn(run_sotf_api_server(
            api_settings(Some("secret")),
            Arc::clone(&state),
            listener,
            None,
            shutdown_rx,
        ));

        let recovered = client.state().await.unwrap();
        assert_eq!(recovered.library.albums, 2);

        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

/// Read one MPD response: lines up to and including the terminating
/// `OK` (or an `ACK` error line).
async fn mpd_read_response(reader: &mut BufReader<tokio::io::ReadHalf<TcpStream>>) -> String {
    let mut out = String::new();
    let deadline = tokio::time::Instant::now() + LOOPBACK_EVENT_TIMEOUT;
    loop {
        let mut line = String::new();
        let read = tokio::time::timeout(
            deadline - tokio::time::Instant::now(),
            reader.read_line(&mut line),
        )
        .await
        .expect("MPD response timed out")
        .expect("MPD read failed");
        assert!(read > 0, "MPD connection closed mid-response: {out}");
        out.push_str(&line);
        if line == "OK\n" || line.starts_with("ACK ") {
            return out;
        }
    }
}

#[test]
fn api_and_mpd_share_queue_state_across_protocols() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = seeded_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };

        // The MPD server binds its configured port itself; reserve a free
        // loopback port first, then hand it over.
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mpd_port = probe.local_addr().unwrap().port();
        drop(probe);
        let adapter: Arc<dyn PlayerAdapter> = Arc::new(MpdPlayerAdapter {
            state: Arc::clone(&state),
        });
        let mpd = MpdServer::with_config(
            MpdServerConfig {
                bind_address: "127.0.0.1".to_string(),
                port: mpd_port,
                tls_enabled: false,
                auth_mode: MpdAuthMode::Password,
                password: None,
                ..Default::default()
            },
            adapter,
        );
        let (mpd_shutdown_tx, mpd_shutdown_rx) = tokio::sync::watch::channel(false);
        let mpd_handle = tokio::spawn(async move { mpd.run(mpd_shutdown_rx).await });

        // Queue an album through the typed HTTP API client...
        let client = client_for(addr, "secret");
        client.queue_add_album("id:42", false).await.unwrap();

        // ...then observe and mutate the same queue over raw MPD.
        let mut stream = None;
        for _ in 0..50 {
            match TcpStream::connect(("127.0.0.1", mpd_port)).await {
                Ok(connected) => {
                    stream = Some(connected);
                    break;
                }
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
            }
        }
        let stream = stream.expect("MPD server must accept loopback connections");
        let (read_half, mut write_half) = tokio::io::split(stream);
        let mut reader = BufReader::new(read_half);

        let mut greeting = String::new();
        reader.read_line(&mut greeting).await.unwrap();
        assert!(greeting.starts_with("OK MPD "), "greeting: {greeting}");

        write_half.write_all(b"playlistinfo\n").await.unwrap();
        let playlist = mpd_read_response(&mut reader).await;
        assert!(
            playlist.contains("Loopback Alpha"),
            "MPD must see the album queued via the HTTP API: {playlist}"
        );

        write_half.write_all(b"status\n").await.unwrap();
        let status = mpd_read_response(&mut reader).await;
        assert!(status.contains("playlistlength: 1"), "status: {status}");

        write_half.write_all(b"clear\n").await.unwrap();
        let cleared = mpd_read_response(&mut reader).await;
        assert!(cleared.ends_with("OK\n"), "clear: {cleared}");

        // The HTTP API observes the MPD-side mutation.
        assert!(client.queue().await.unwrap().items.is_empty());

        write_half.write_all(b"close\n").await.unwrap();
        let _ = mpd_shutdown_tx.send(true);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), mpd_handle).await;
        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}
