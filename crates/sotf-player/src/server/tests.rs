use super::api::api_auth_valid;
use super::api::api_clear_queue;
use super::api::api_header;
use super::api::api_json_response;
use super::api::api_media_auth_valid;
use super::api::api_media_source;
use super::api::api_parse_library_album_query;
use super::api::api_parse_range_header;
use super::api::api_response_status;
use super::api::api_state_json;
use super::dlna::dlna_advertised_ipv4;
use super::dlna::dlna_server_url;
use super::dlna::dlna_server_url_for_bind;
use super::generate::ensure_sotf_api_connection_config;
use super::handle::handle_sotf_api_request;
use super::misc::find_header_end;
use super::misc::normalize_certificate_fingerprint;
use super::misc::redact_api_path_secrets;
use super::mpd_player_adapter::MpdPlayerAdapter;
use super::parse::parse_api_request;
use super::parse::parse_sotf_api_connection_qr_payload;
use super::run::run_sotf_api_server;
use super::server_state::ServerState;
use super::server_state::build_sotf_api_tls_acceptor;
use super::sotf::sotf_api_connection_qr_payload;
use super::sotf::sotf_api_server_url_for_bind;
use super::sotf::sotf_api_server_url_for_bind_with_tls;
use super::sotf::sotf_api_server_url_for_settings;
use super::types::ApiRequest;
use super::validate::sotf_api_plaintext_warning;
use crate::federation_config::{ServerConfig, SotfApiSettings};
use crate::library::DirectoryInfo;
use crate::library::MusicLibrary;
use crate::library_stats::LibraryStats;
use crate::player::Player;
use crate::queue::Queue;
use crate::sotf_api_client::SotfApiClient;
use crate::sotf_server_event::SotfServerEvent;
use parking_lot::Mutex;
use serde_json::{Value, json};
use sotf_mpd::PlayerAdapter;
use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::net::TcpListener;

mod auth;
mod misc;
mod read;

use auth::auth_get;
use auth::auth_header;
use auth::auth_post;
use misc::api_settings;
use misc::stop_test_api_server;
use misc::test_state;
use misc::try_spawn_test_api_server;
use read::connect_sse_client;
use read::read_http_response;
use read::read_until;

#[test]
fn server_mode_api_defaults_enable_api_and_generate_token() {
    let mut config = ServerConfig::default();

    assert!(ensure_sotf_api_connection_config(&mut config));

    assert!(config.api.enabled);
    let token = config.api.auth_token.as_deref().unwrap();
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()));
}

#[test]
fn server_mode_api_defaults_preserve_existing_enabled_token() {
    let mut config = ServerConfig::default();
    config.api.enabled = true;
    config.api.auth_token = Some("existing-token".to_string());

    assert!(!ensure_sotf_api_connection_config(&mut config));

    assert!(config.api.enabled);
    assert_eq!(config.api.auth_token.as_deref(), Some("existing-token"));
}

#[test]
fn dlna_server_url_includes_configured_port() {
    let url = dlna_server_url(8200);

    assert!(url.starts_with("http://"));
    assert!(url.ends_with(":8200/"));
}

#[test]
fn dlna_server_url_uses_specific_bind_address() {
    let url = dlna_server_url_for_bind("192.168.1.42", 8200);

    assert_eq!(url, "http://192.168.1.42:8200/");
}

#[test]
fn sotf_api_server_url_includes_api_path() {
    let url = sotf_api_server_url_for_bind("192.168.1.42", 8732);

    assert_eq!(url, "https://192.168.1.42:8732/api/v1");
}

#[test]
fn sotf_api_server_url_can_be_plaintext_when_tls_disabled() {
    let settings = SotfApiSettings {
        enabled: true,
        bind_address: "192.168.1.42".to_string(),
        port: 8732,
        friendly_name: "SOTF".to_string(),
        tls_enabled: false,
        auth_token: Some("secret-token".to_string()),
    };

    assert_eq!(
        sotf_api_server_url_for_settings(&settings),
        "http://192.168.1.42:8732/api/v1"
    );
}

#[test]
fn sotf_api_server_url_uses_https_when_tls_enabled() {
    let url = sotf_api_server_url_for_bind_with_tls("192.168.1.42", 8732, true);

    assert_eq!(url, "https://192.168.1.42:8732/api/v1");
}

#[test]
fn sotf_api_plaintext_warning_ignores_loopback_binds() {
    let settings = SotfApiSettings {
        enabled: true,
        bind_address: "127.0.0.1".to_string(),
        port: 8732,
        friendly_name: "SOTF".to_string(),
        tls_enabled: false,
        auth_token: Some("secret-token".to_string()),
    };

    assert!(sotf_api_plaintext_warning(&settings).is_none());
}

#[test]
fn sotf_api_plaintext_warning_flags_non_loopback_binds() {
    let settings = SotfApiSettings {
        enabled: true,
        bind_address: "0.0.0.0".to_string(),
        port: 8732,
        friendly_name: "SOTF".to_string(),
        tls_enabled: false,
        auth_token: Some("secret-token".to_string()),
    };

    let warning = sotf_api_plaintext_warning(&settings).expect("non-loopback warning");
    assert!(warning.contains("plaintext HTTP"));
    assert!(warning.contains("0.0.0.0:8732"));
}

#[test]
fn sotf_api_connection_qr_payload_includes_url_and_token() {
    let settings = SotfApiSettings {
        enabled: true,
        bind_address: "192.168.1.42".to_string(),
        port: 8732,
        friendly_name: "Listening Room".to_string(),
        tls_enabled: true,
        auth_token: Some("secret-token".to_string()),
    };

    let payload = sotf_api_connection_qr_payload(&settings).unwrap();
    let json: Value = serde_json::from_str(&payload).unwrap();

    assert_eq!(json["kind"], "sotf-api-connection");
    assert_eq!(json["version"], 1);
    assert_eq!(json["name"], "Listening Room");
    assert_eq!(json["url"], "https://192.168.1.42:8732/api/v1");
    assert_eq!(json["auth"], "bearer");
    assert_eq!(json["token"], "secret-token");
}

#[test]
fn sotf_api_connection_qr_payload_round_trips() {
    let settings = SotfApiSettings {
        enabled: true,
        bind_address: "192.168.1.42".to_string(),
        port: 8732,
        friendly_name: "Listening Room".to_string(),
        tls_enabled: true,
        auth_token: Some("secret-token".to_string()),
    };

    let payload = sotf_api_connection_qr_payload(&settings).unwrap();
    let parsed = parse_sotf_api_connection_qr_payload(&payload).unwrap();

    assert_eq!(parsed.name, "Listening Room");
    assert_eq!(parsed.url, "https://192.168.1.42:8732/api/v1");
    assert_eq!(parsed.token, "secret-token");
}

#[test]
fn sotf_api_connection_qr_payload_rejects_wrong_kind() {
    let err = parse_sotf_api_connection_qr_payload(
            r#"{"kind":"not-sotf","version":1,"auth":"bearer","url":"http://host:8732/api/v1","token":"secret"}"#,
        )
        .unwrap_err();

    assert!(err.contains("not a SOTF API connection"));
}

#[test]
fn sotf_api_connection_qr_payload_requires_token() {
    let err = parse_sotf_api_connection_qr_payload(
            r#"{"kind":"sotf-api-connection","version":1,"auth":"bearer","url":"http://host:8732/api/v1","token":""}"#,
        )
        .unwrap_err();

    assert!(err.contains("missing the API token"));
}

#[test]
fn dlna_advertised_ipv4_uses_specific_bind_address() {
    assert_eq!(
        dlna_advertised_ipv4("192.168.1.42"),
        Ipv4Addr::new(192, 168, 1, 42)
    );
}

#[test]
fn certificate_fingerprint_normalization_accepts_common_hex_formats() {
    let compact = "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
    let colon = "aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99";
    let expected = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";

    assert_eq!(
        normalize_certificate_fingerprint(compact).unwrap(),
        expected
    );
    assert_eq!(normalize_certificate_fingerprint(colon).unwrap(), expected);
}

#[test]
fn sotf_api_auth_accepts_only_bearer_token() {
    let headers = vec![("authorization".to_string(), "Bearer secret".to_string())];
    assert!(api_auth_valid(&headers, "secret"));
    assert!(!api_auth_valid(&headers, "other"));

    let headers = vec![("authorization".to_string(), "Basic secret".to_string())];
    assert!(!api_auth_valid(&headers, "secret"));
}

#[test]
fn sotf_api_media_auth_accepts_query_token() {
    let request = ApiRequest {
        method: "GET".to_string(),
        path: "/api/v1/media/track-1?token=secret".to_string(),
        headers: vec![],
        body: vec![],
    };
    assert!(api_media_auth_valid(&request, "secret"));
    assert!(!api_media_auth_valid(&request, "other"));
}

#[test]
fn sotf_api_log_path_redacts_tokens() {
    assert_eq!(
        redact_api_path_secrets("/api/v1/media/track-1?token=secret&foo=bar"),
        "/api/v1/media/track-1?token=%3Credacted%3E&foo=bar"
    );
}

#[test]
fn sotf_api_log_path_redacts_secret_query_keys_case_insensitively() {
    let redacted = redact_api_path_secrets(
        "/api/v1/pair?Auth_Token=secret&refresh_token=r1&client_secret=s2&image_api_key=k3&secret=s4&foo=bar",
    );

    assert_eq!(
        redacted,
        "/api/v1/pair?Auth_Token=%3Credacted%3E&refresh_token=%3Credacted%3E&client_secret=%3Credacted%3E&image_api_key=%3Credacted%3E&secret=%3Credacted%3E&foo=bar"
    );
    assert!(!redacted.contains("Auth_Token=secret"));
    assert!(!redacted.contains("r1"));
    assert!(!redacted.contains("s2"));
    assert!(!redacted.contains("k3"));
    assert!(!redacted.contains("s4"));
}

#[test]
fn sotf_api_parses_request_with_body() {
    let raw =
            b"POST /api/v1/volume HTTP/1.1\r\nHost: localhost\r\nContent-Length: 13\r\n\r\n{\"volume\":42}";
    let header_end = find_header_end(raw).unwrap();
    let request = parse_api_request(raw, header_end).unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/api/v1/volume");
    assert_eq!(api_header(&request.headers, "host"), Some("localhost"));
    assert_eq!(request.body, br#"{"volume":42}"#);
}

#[test]
fn sotf_api_response_status_parser_reads_status_line() {
    let response = api_json_response(401, json!({ "ok": false }));
    assert_eq!(api_response_status(&response), Some(401));
    assert_eq!(api_response_status(b"not http"), None);
}

#[test]
fn sotf_api_media_range_parser_handles_common_forms() {
    assert_eq!(api_parse_range_header(None, 10).unwrap(), None);
    assert_eq!(
        api_parse_range_header(Some("bytes=2-5"), 10).unwrap(),
        Some((2, 5))
    );
    assert_eq!(
        api_parse_range_header(Some("bytes=6-"), 10).unwrap(),
        Some((6, 9))
    );
    assert_eq!(
        api_parse_range_header(Some("bytes=-4"), 10).unwrap(),
        Some((6, 9))
    );
    assert!(api_parse_range_header(Some("items=0-1"), 10).is_err());
    assert!(api_parse_range_header(Some("bytes=10-12"), 10).is_err());
    assert!(api_parse_range_header(Some("bytes=1-0"), 10).is_err());
    assert!(api_parse_range_header(Some("bytes=0-1,3-4"), 10).is_err());
}

#[test]
fn sotf_api_media_source_uses_cached_lookup_until_library_changes() {
    let state = misc::test_state();
    let track_path = std::path::PathBuf::from("/music/cached.flac");
    state.library.lock().albums = vec![crate::library::Album {
        title: "Cached Album".to_string(),
        uuid: Some("album-cache".to_string()),
        tracks: vec![crate::library::Track {
            path: track_path.clone(),
            uuid: Some("track-cache".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    }];

    let source = api_media_source(&state, "uuid:track-cache").expect("cached track source");
    assert_eq!(source.path, track_path);
    assert_eq!(state.media_source_index_rebuilds_for_test(), 1);

    let source = api_media_source(&state, "track-track-cache").expect("legacy media track id");
    assert_eq!(source.path, track_path);
    assert_eq!(
        state.media_source_index_rebuilds_for_test(),
        1,
        "unchanged library version should not rescan albums/tracks"
    );

    state.mark_library_changed();
    let source = api_media_source(&state, "uuid:track-cache").expect("rebuilt track source");
    assert_eq!(source.path, track_path);
    assert_eq!(state.media_source_index_rebuilds_for_test(), 2);
}

#[test]
fn sotf_api_state_uses_cached_library_stats() {
    let state = misc::test_state();
    {
        let mut library = state.library.lock();
        library.albums = vec![crate::library::Album {
            title: "One Visible Track".to_string(),
            tracks: vec![crate::library::Track::default()],
            ..Default::default()
        }];
        let mut stats = LibraryStats::compute(&library.albums);
        stats.total_tracks = 42;
        stats.valid = true;
        library.set_stats_cache_for_test(stats);
    }

    let adapter = MpdPlayerAdapter {
        state: Arc::clone(&state),
    };
    let body = api_state_json(&state, &adapter);

    assert_eq!(body["library"]["albums"], 1);
    assert_eq!(body["library"]["tracks"], 42);
}

#[test]
fn broadcast_events_on_volume_change() {
    let state = Arc::new(ServerState {
        player: Mutex::new(Player::new()),
        library: Mutex::new(MusicLibrary::default()),
        media_source_index: Mutex::new(Default::default()),
        queue: Mutex::new(Queue::new()),
        playlist_version: std::sync::atomic::AtomicU32::new(1),
        library_version: std::sync::atomic::AtomicU64::new(1),
        events: crate::sotf_server_event::new_event_broadcaster(64),
        library_scan_active: std::sync::atomic::AtomicBool::new(false),
        pairing_mode: std::sync::atomic::AtomicBool::new(false),
        pairing_nonce: parking_lot::Mutex::new(String::new()),
        pairing_enabled_at: parking_lot::Mutex::new(None),
        trusted_clients: parking_lot::Mutex::new(
            sotf_tls::TrustedClientStore::load(std::env::temp_dir().as_path()).unwrap(),
        ),
        trusted_client_fingerprints: Arc::new(std::sync::Mutex::new(HashSet::new())),
        server_fingerprint: "AA:BB:CC".to_string(),
    });
    let mut rx = state.events.subscribe();
    let adapter = MpdPlayerAdapter {
        state: Arc::clone(&state),
    };

    // Set volume to 50 first, then change by +10
    adapter.set_volume(50).unwrap();
    let _ = rx.try_recv(); // consume VolumeChanged from set_volume
    adapter.volume_change(10).unwrap();
    let event = rx.try_recv().expect("expected an event");
    assert_eq!(event, SotfServerEvent::VolumeChanged { volume: 60 });
}

#[test]
fn broadcast_events_on_queue_clear() {
    let state = Arc::new(ServerState {
        player: Mutex::new(Player::new()),
        library: Mutex::new(MusicLibrary::default()),
        media_source_index: Mutex::new(Default::default()),
        queue: Mutex::new(Queue::new()),
        playlist_version: std::sync::atomic::AtomicU32::new(1),
        library_version: std::sync::atomic::AtomicU64::new(1),
        events: crate::sotf_server_event::new_event_broadcaster(64),
        library_scan_active: std::sync::atomic::AtomicBool::new(false),
        pairing_mode: std::sync::atomic::AtomicBool::new(false),
        pairing_nonce: parking_lot::Mutex::new(String::new()),
        pairing_enabled_at: parking_lot::Mutex::new(None),
        trusted_clients: parking_lot::Mutex::new(
            sotf_tls::TrustedClientStore::load(std::env::temp_dir().as_path()).unwrap(),
        ),
        trusted_client_fingerprints: Arc::new(std::sync::Mutex::new(HashSet::new())),
        server_fingerprint: "AA:BB:CC".to_string(),
    });
    let mut rx = state.events.subscribe();

    api_clear_queue(&state).unwrap();
    // api_clear_queue broadcasts PlaybackChanged then QueueChanged
    let event1 = rx.try_recv().expect("expected first event after clear");
    assert!(matches!(event1, SotfServerEvent::PlaybackChanged));
    let event2 = rx.try_recv().expect("expected second event after clear");
    assert!(matches!(event2, SotfServerEvent::QueueChanged { .. }));
}

#[test]
fn api_library_albums_rejects_invalid_bounds_and_sort() {
    assert!(api_parse_library_album_query("/api/v1/library/albums?limit=0").is_err());
    assert!(api_parse_library_album_query("/api/v1/library/albums?offset=-1").is_err());
    assert!(api_parse_library_album_query("/api/v1/library/albums?sort=random").is_err());
}

#[test]
fn sotf_api_serves_parallel_clients_while_sse_client_is_connected() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = test_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };

        let sse_client = connect_sse_client(addr).await;
        let mut handles = Vec::new();
        for idx in 0..8 {
            let path = if idx % 2 == 0 {
                "/api/v1/state"
            } else {
                "/api/v1/queue"
            };
            handles.push(tokio::spawn(read_http_response(addr, auth_get(path))));
        }

        for handle in handles {
            let response = handle.await.unwrap();
            assert!(response.starts_with("HTTP/1.1 200 OK"));
        }

        drop(sse_client);
        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn sotf_api_http_playback_endpoints_round_trip() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = test_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };

        let volume =
            read_http_response(addr, auth_post("/api/v1/volume", r#"{"volume":37}"#)).await;
        assert!(volume.starts_with("HTTP/1.1 200 OK"));
        assert!(volume.contains("\"command\":\"volume\""));

        for (path, command) in [
            ("/api/v1/pause", "pause"),
            ("/api/v1/resume", "resume"),
            ("/api/v1/stop", "stop"),
        ] {
            let response = read_http_response(addr, auth_post(path, "{}")).await;
            assert!(
                response.starts_with("HTTP/1.1 200 OK"),
                "{path}: {response}"
            );
            assert!(
                response.contains(&format!("\"command\":\"{command}\"")),
                "{path}: {response}"
            );
        }

        let state_response = read_http_response(addr, auth_get("/api/v1/state")).await;
        assert!(state_response.starts_with("HTTP/1.1 200 OK"));
        assert!(state_response.contains("\"volume\":37"));

        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn sotf_api_capabilities_advertise_p1_server_surfaces() {
    let state = test_state();
    let request = ApiRequest {
        method: "GET".to_string(),
        path: "/api/v1/capabilities".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let response =
        handle_sotf_api_request(request, &state, &api_settings(Some("secret")), "secret");
    let response = String::from_utf8(response).unwrap();

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("\"outputs\":true"));
    assert!(response.contains("\"plugin_graph\":true"));
    assert!(response.contains("\"plugin_presets\":true"));
    assert!(response.contains("Connection: close"));
}

#[test]
fn sotf_api_output_plugin_graph_and_preset_endpoints_exist() {
    let state = test_state();

    for (path, expected) in [
        ("/api/v1/outputs", "\"outputs\""),
        ("/api/v1/plugin-graph", "\"nodes\""),
        ("/api/v1/plugin-presets", "\"plugin_types\""),
    ] {
        let request = ApiRequest {
            method: "GET".to_string(),
            path: path.to_string(),
            headers: auth_header(),
            body: Vec::new(),
        };
        let response =
            handle_sotf_api_request(request, &state, &api_settings(Some("secret")), "secret");
        let response = String::from_utf8(response).unwrap();
        assert!(
            response.starts_with("HTTP/1.1 200 OK"),
            "{path}: {response}"
        );
        assert!(response.contains(expected), "{path}: {response}");
        assert!(response.contains("Connection: close"));
    }
}

#[test]
fn sotf_api_plugin_presets_supports_filter_and_errors_on_unknown_plugin() {
    let state = test_state();

    let request = ApiRequest {
        method: "GET".to_string(),
        path: "/api/v1/plugin-presets?plugin_type=eq".to_string(),
        headers: auth_header(),
        body: Vec::new(),
    };
    let response =
        handle_sotf_api_request(request, &state, &api_settings(Some("secret")), "secret");
    let response = String::from_utf8(response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.contains("\"plugin_type\":\"eq\""), "{response}");
    assert!(!response.contains("\"plugin_types\""), "{response}");

    let request = ApiRequest {
        method: "GET".to_string(),
        path: "/api/v1/plugin-presets?plugin_type=definitely_not_a_plugin".to_string(),
        headers: auth_header(),
        body: Vec::new(),
    };
    let response =
        handle_sotf_api_request(request, &state, &api_settings(Some("secret")), "secret");
    let response = String::from_utf8(response).unwrap();
    assert!(
        response.starts_with("HTTP/1.1 400 Bad Request"),
        "{response}"
    );
    assert!(response.contains("unknown plugin type"), "{response}");
}

#[test]
fn client_server_p1_source_contracts_are_gated_or_nonblocking() {
    let service_manager = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/service_manager.rs"),
    )
    .unwrap();
    assert!(service_manager.contains("#[cfg(feature = \"tidal\")]"));
    assert!(service_manager.contains("#[cfg(not(feature = \"tidal\"))]"));
    assert!(service_manager.contains("#[cfg(feature = \"spotify\")]"));
    assert!(service_manager.contains("#[cfg(not(feature = \"spotify\"))]"));

    let streaming_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sotf-server/crates/sotf-streaming/src");
    let http_source =
        std::fs::read_to_string(streaming_root.join("http_source/http_media_source.rs")).unwrap();
    assert!(
        http_source.contains("fn schedule_reconnect")
            && !http_source.contains("std::thread::sleep(Duration::from_millis(delay_ms));"),
        "HttpMediaSource reconnect must not sleep on the decoder read thread"
    );

    let mpd_source = std::fs::read_to_string(streaming_root.join("mpd_source.rs")).unwrap();
    assert!(
        mpd_source.contains("open_httpd_stream_with_retry")
            && !mpd_source.contains("from_millis(200)"),
        "MPD stream startup must retry readiness instead of sleeping a fixed 200 ms"
    );
}

#[test]
fn sotf_api_http_queue_endpoints_round_trip() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = test_state();
        {
            let mut library = state.library.lock();
            library.albums.push(crate::library::Album {
                id: Some(42),
                title: "Socket Album".to_string(),
                tracks: vec![crate::library::Track {
                    title: Some("Socket Track".to_string()),
                    album_artist: Some("Socket Artist".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            });
        }

        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };

        let add = read_http_response(
            addr,
            auth_post(
                "/api/v1/queue/add-album",
                r#"{"album_id":"id:42","play_now":false}"#,
            ),
        )
        .await;
        assert!(add.starts_with("HTTP/1.1 200 OK"));
        assert!(add.contains("\"command\":\"queue.add-album\""));

        let queue = read_http_response(addr, auth_get("/api/v1/queue")).await;
        assert!(queue.starts_with("HTTP/1.1 200 OK"));
        assert!(queue.contains("Socket Album"));

        let clear = read_http_response(addr, auth_post("/api/v1/queue/clear", "{}")).await;
        assert!(clear.starts_with("HTTP/1.1 200 OK"));
        assert!(clear.contains("\"command\":\"queue.clear\""));

        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn sotf_api_http_media_range_and_auth_round_trip() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = test_state();
        let temp = tempfile::tempdir().unwrap();
        let media_path = temp.path().join("track.raw");
        std::fs::write(&media_path, b"0123456789").unwrap();
        {
            let mut library = state.library.lock();
            library.albums.push(crate::library::Album {
                id: Some(7),
                title: "Media Album".to_string(),
                tracks: vec![crate::library::Track {
                    path: media_path,
                    title: Some("Media Track".to_string()),
                    uuid: Some("media-track".to_string()),
                    album_artist: Some("Media Artist".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            });
        }

        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };

        let unauthorized =
            read_http_response(addr, "GET /api/v1/media/uuid:media-track HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".to_string()).await;
        assert!(unauthorized.starts_with("HTTP/1.1 401 Unauthorized"));

        let partial = read_http_response(
            addr,
            "GET /api/v1/media/uuid:media-track?token=secret HTTP/1.1\r\nHost: localhost\r\nRange: bytes=2-5\r\nConnection: close\r\n\r\n".to_string(),
        )
        .await;
        assert!(partial.starts_with("HTTP/1.1 206 Partial Content"));
        assert!(partial.contains("Content-Range: bytes 2-5/10"));
        assert!(partial.ends_with("2345"));

        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn sotf_api_tls_health_round_trip() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = test_state();
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("failed to bind TLS test API server: {err}"),
        };
        let addr = listener.local_addr().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let cert_store = sotf_tls::CertStore::load_or_generate(tmp.path()).unwrap();
        let tls_acceptor = build_sotf_api_tls_acceptor(&cert_store).unwrap();
        let mut settings = api_settings(Some("secret"));
        settings.tls_enabled = true;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let server_handle = tokio::spawn(run_sotf_api_server(
            settings,
            state,
            listener,
            Some(tls_acceptor),
            shutdown_rx,
        ));

        let client =
            SotfApiClient::new_with_tofu_dir(format!("https://{addr}"), "secret", tmp.path())
                .unwrap();
        let health = client.health().await.unwrap();
        assert_eq!(health.service, "sotf");
        assert!(health.ok);

        let tofu_store = sotf_tls::TofuStore::load(tmp.path()).unwrap();
        assert!(matches!(
            tofu_store.check(
                &format!("{}:{}", addr.ip(), addr.port()),
                &cert_store.server_fingerprint()
            ),
            sotf_tls::TofuResult::Trusted
        ));

        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn sotf_api_broadcasts_events_to_multiple_sse_clients() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = test_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };

        let mut first = connect_sse_client(addr).await;
        let mut second = connect_sse_client(addr).await;

        state.broadcast(SotfServerEvent::VolumeChanged { volume: 77 });

        let first_event = read_until(&mut first, "event: volume_changed").await;
        let second_event = read_until(&mut second, "event: volume_changed").await;
        assert!(first_event.contains("\"volume\":77"));
        assert!(second_event.contains("\"volume\":77"));

        drop(first);
        drop(second);
        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn sotf_api_emits_library_changed_when_library_version_advances() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = test_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };

        let mut client = connect_sse_client(addr).await;
        let version = state.mark_library_changed();

        let event = read_until(&mut client, "event: library_changed").await;
        assert!(event.contains(&format!("\"library_version\":{version}")));

        drop(client);
        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn sotf_api_emits_scanner_progress_updates() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = test_state();
        let (addr, shutdown_tx, server_handle) =
            match try_spawn_test_api_server(Arc::clone(&state)).await {
                Ok(server) => server,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(err) => panic!("failed to bind test API server: {err}"),
            };

        let mut client = connect_sse_client(addr).await;
        state.report_scanner_progress(3, 9);

        let event = read_until(&mut client, "event: scanner_progress").await;
        assert!(event.contains("\"done\":3"));
        assert!(event.contains("\"total\":9"));

        drop(client);
        stop_test_api_server(shutdown_tx, server_handle).await;
    });
}

#[test]
fn sotf_api_library_scan_requires_configured_directories() {
    let state = test_state();
    let request = ApiRequest {
        method: "POST".to_string(),
        path: "/api/v1/library/scan".to_string(),
        headers: auth_header(),
        body: b"{}".to_vec(),
    };

    let response =
        handle_sotf_api_request(request, &state, &api_settings(Some("secret")), "secret");
    let response = String::from_utf8(response).unwrap();

    assert_eq!(api_response_status(response.as_bytes()), Some(409));
    assert!(response.contains("no library directories configured"));
}

#[test]
fn sotf_api_library_scan_rejects_overlapping_scans() {
    let state = test_state();
    state.library.lock().directories.push(DirectoryInfo {
        path: std::env::temp_dir(),
        file_count: 0,
        album_count: 0,
        last_scanned: None,
        expanded: false,
        subdirectories: Vec::new(),
        children_loaded: true,
    });
    state
        .library_scan_active
        .store(true, std::sync::atomic::Ordering::Release);

    let request = ApiRequest {
        method: "POST".to_string(),
        path: "/api/v1/library/scan".to_string(),
        headers: auth_header(),
        body: br#"{"force":true}"#.to_vec(),
    };

    let response =
        handle_sotf_api_request(request, &state, &api_settings(Some("secret")), "secret");
    let response = String::from_utf8(response).unwrap();

    assert_eq!(api_response_status(response.as_bytes()), Some(409));
    assert!(response.contains("library scan already running"));
}

#[test]
fn pairing_enable_disable_cycle() {
    let state = test_state();

    // Enable
    let req = ApiRequest {
        method: "POST".to_string(),
        path: "/api/v1/pairing/enable".to_string(),
        headers: auth_header(),
        body: Vec::new(),
    };
    let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
    let resp_str = String::from_utf8(resp).unwrap();
    assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
    assert!(resp_str.contains("\"pairing_enabled\":true"));
    assert!(
        state
            .pairing_mode
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    // Disable
    let req = ApiRequest {
        method: "POST".to_string(),
        path: "/api/v1/pairing/disable".to_string(),
        headers: auth_header(),
        body: Vec::new(),
    };
    let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
    let resp_str = String::from_utf8(resp).unwrap();
    assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
    assert!(resp_str.contains("\"pairing_enabled\":false"));
    assert!(
        !state
            .pairing_mode
            .load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[test]
fn pairing_revoke_client() {
    let state = test_state();
    let fp = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";
    state.trusted_clients.lock().add(fp, "Test").unwrap();
    state
        .trusted_client_fingerprints
        .lock()
        .unwrap()
        .insert(fp.to_string());
    assert!(state.trusted_clients.lock().contains(fp));

    let req = ApiRequest {
        method: "DELETE".to_string(),
        path: format!("/api/v1/pairing/clients/{}", fp),
        headers: auth_header(),
        body: Vec::new(),
    };
    let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
    let resp_str = String::from_utf8(resp).unwrap();
    assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
    assert!(!state.trusted_clients.lock().contains(fp));
    assert!(
        !state
            .trusted_client_fingerprints
            .lock()
            .unwrap()
            .contains(fp)
    );
}
