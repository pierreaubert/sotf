//! Integration tests for `TidalProvider` against a local mock HTTP server.
//!
//! The service is injected via `TidalProvider::with_service`, pointed at the
//! mock through `with_api_base` / `with_auth_base`.

#![cfg(feature = "tidal")]

mod common;

use common::{MockResponse, spawn_mock_server};
use sotf_audio::decoder::{AudioSource, ServiceId};
use sotf_federation::{
    LibraryProvider, ProviderError, SourceId, SourceType, TidalProvider, TidalProviderConfig,
    TidalTokenPersister,
};
use sotf_service_tidal::TidalService;
use sotf_services::{ServiceCredentials, StreamingService};
use std::sync::{Arc, Mutex};

const SESSION_BODY: &str = r#"{"userId": 42, "countryCode": "US"}"#;

const TOKEN_BODY: &str = r#"{"access_token": "refreshed-access-token", "refresh_token": "refreshed-refresh-token", "token_type": "Bearer"}"#;

const ALBUM_ITEM: &str = r#"{"created": "2026-01-01T00:00:00.000Z", "item":
    {"id": 7, "title": "The Wall", "artist": {"name": "Pink Floyd"},
     "cover": "ab12cd34-5678-90ef-1234-567890abcdef", "numberOfTracks": 2,
     "releaseDate": "1979-11-30"}}"#;

const ALBUM_ITEM_8: &str = r#"{"created": "2026-01-01T00:00:00.000Z", "item":
    {"id": 8, "title": "Animals", "artist": {"name": "Pink Floyd"},
     "cover": "bb12cd34-5678-90ef-1234-567890abcdef", "numberOfTracks": 5,
     "releaseDate": "1977-01-21"}}"#;

const ALBUM_TRACKS_BODY: &str = r#"{"items": [
    {"id": 101, "title": "In the Flesh?", "duration": 200, "trackNumber": 1,
     "artist": {"name": "Pink Floyd"},
     "album": {"title": "The Wall", "cover": "ab12cd34-5678-90ef-1234-567890abcdef"}},
    {"id": 102, "title": "The Thin Ice", "duration": 149, "trackNumber": 2,
     "artist": {"name": "Pink Floyd"},
     "album": {"title": "The Wall", "cover": "ab12cd34-5678-90ef-1234-567890abcdef"}}
]}"#;

fn favorites_body(items: &[&str], total: u64) -> String {
    format!(
        r#"{{"limit": 50, "offset": 0, "totalNumberOfItems": {total}, "items": [{}]}}"#,
        items.join(",")
    )
}

/// Mock Tidal API: /sessions authenticates as user 42, favorites serve the
/// given album items, /albums/7/tracks serves two tracks.
fn spawn_tidal_mock(album_items: Vec<String>) -> common::MockServer {
    spawn_mock_server(move |req| {
        if req.method == "GET" && req.path.starts_with("/sessions") {
            MockResponse::json(200, SESSION_BODY)
        } else if req.method == "POST" && req.path == "/token" {
            MockResponse::json(200, TOKEN_BODY)
        } else if req.method == "GET" && req.path.starts_with("/users/42/favorites/albums") {
            let total = album_items.len() as u64;
            let refs: Vec<&str> = album_items.iter().map(|s| s.as_str()).collect();
            MockResponse::json(200, favorites_body(&refs, total))
        } else if req.method == "GET" && req.path.starts_with("/albums/7/tracks") {
            MockResponse::json(200, ALBUM_TRACKS_BODY)
        } else {
            MockResponse::json(404, "{}")
        }
    })
}

fn provider_for(server: &common::MockServer) -> TidalProvider {
    let mut service = TidalService::new()
        .with_client_id("test-client-id")
        .with_api_base(&server.base_url)
        .with_auth_base(&server.base_url);
    service
        .authenticate(ServiceCredentials::AccessToken(
            "access-token-1".to_string(),
        ))
        .expect("access token auth");
    TidalProvider::with_service(SourceId("tidal:test".to_string()), service)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fetch_all_albums_maps_favorites_to_provider_albums() {
    let server = spawn_tidal_mock(vec![ALBUM_ITEM.to_string()]);
    let provider = provider_for(&server);

    assert_eq!(provider.source_type(), SourceType::Tidal);
    assert_eq!(provider.display_name(), "Tidal");
    let caps = provider.capabilities();
    assert!(caps.seekable);
    assert!(caps.has_album_art);
    assert!(!caps.writable);
    assert!(!caps.supports_events);
    assert!(provider.is_available().await);

    let albums = provider.fetch_all_albums().await.expect("fetch_all_albums");
    assert_eq!(albums.len(), 1);
    let album = &albums[0];
    assert_eq!(album.external_id, "7");
    assert_eq!(album.title, "The Wall");
    assert_eq!(album.artist, "Pink Floyd");
    assert_eq!(album.year, Some(1979));
    assert!(album.album_art_url.is_some());

    assert_eq!(album.tracks.len(), 2);
    let track = &album.tracks[0];
    assert_eq!(track.external_id, "101");
    assert_eq!(track.title, "In the Flesh?");
    assert_eq!(track.artist.as_deref(), Some("Pink Floyd"));
    assert_eq!(track.track_number, Some(1));
    assert_eq!(track.duration_secs, Some(200.0));
    assert_eq!(
        track.audio_source,
        AudioSource::ServiceStream {
            service: ServiceId::Tidal,
            track_id: "101".to_string(),
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resolve_source_defers_to_service_stream() {
    let server = spawn_tidal_mock(vec![ALBUM_ITEM.to_string()]);
    let provider = provider_for(&server);

    let source = provider
        .resolve_source("101")
        .await
        .expect("resolve_source");
    assert_eq!(
        source,
        AudioSource::ServiceStream {
            service: ServiceId::Tidal,
            track_id: "101".to_string(),
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_favorites_yield_no_albums() {
    let server = spawn_tidal_mock(vec![]);
    let provider = provider_for(&server);
    let albums = provider.fetch_all_albums().await.expect("fetch_all_albums");
    assert!(albums.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fetch_album_art_returns_none_for_unknown_album() {
    let server = spawn_tidal_mock(vec![ALBUM_ITEM.to_string()]);
    let provider = provider_for(&server);
    // No fetch_all_albums call → no cached art URL.
    let art = provider
        .fetch_album_art("7")
        .await
        .expect("fetch_album_art");
    assert!(art.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connect_with_refreshes_tokens_then_validates() {
    let server = spawn_tidal_mock(vec![ALBUM_ITEM.to_string()]);
    let config = TidalProviderConfig {
        access_token: "stale-access-token".to_string(),
        refresh_token: "stale-refresh-token".to_string(),
        client_id: "test-client-id".to_string(),
        country_code: "US".to_string(),
        quality: "LOSSLESS".to_string(),
    };
    let service = TidalProvider::connect_with(
        &config,
        Some(&server.base_url),
        Some(&server.base_url),
        None,
    )
    .expect("connect_with");
    // The refresh flow replaced both tokens and captured the user id.
    assert_eq!(service.access_token(), Some("refreshed-access-token"));
    assert_eq!(service.refresh_token(), Some("refreshed-refresh-token"));
    assert_eq!(service.user_id(), Some(42));

    let provider = TidalProvider::with_service(SourceId("tidal:test".to_string()), service);
    assert!(provider.is_available().await);
    let albums = provider.fetch_all_albums().await.expect("fetch_all_albums");
    assert_eq!(albums.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_fails_without_tokens() {
    let result = TidalProvider::new(
        SourceId("tidal:test".to_string()),
        TidalProviderConfig {
            access_token: String::new(),
            refresh_token: String::new(),
            client_id: "test-client-id".to_string(),
            country_code: "US".to_string(),
            quality: "LOSSLESS".to_string(),
        },
    )
    .await;
    let err = result.err().expect("construction must fail without tokens");
    assert!(err.to_string().contains("access token"), "got: {err}");
}

fn stale_config(client_id: &str) -> TidalProviderConfig {
    TidalProviderConfig {
        access_token: "stale-access-token".to_string(),
        refresh_token: "stale-refresh-token".to_string(),
        client_id: client_id.to_string(),
        country_code: "US".to_string(),
        quality: "LOSSLESS".to_string(),
    }
}

/// The Critical fix: a scan-path connect rotates the single-use refresh
/// token, so the rotated pair must be reported to the persister callback.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connect_with_reports_rotated_tokens_to_persister() {
    type PersisterCalls = Arc<Mutex<Vec<(String, Option<String>)>>>;
    let server = spawn_tidal_mock(vec![]);
    let calls: PersisterCalls = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&calls);
    let persister: TidalTokenPersister = Arc::new(move |access: &str, refresh: Option<&str>| {
        captured
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((access.to_string(), refresh.map(str::to_string)));
    });

    let service = TidalProvider::connect_with(
        &stale_config("test-client-id"),
        Some(&server.base_url),
        Some(&server.base_url),
        Some(&persister),
    )
    .expect("connect_with");

    let calls = calls.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(calls.len(), 1, "persister calls: {calls:?}");
    assert_eq!(calls[0].0, "refreshed-access-token");
    assert_eq!(calls[0].1.as_deref(), Some("refreshed-refresh-token"));
    drop(calls);
    assert_eq!(service.access_token(), Some("refreshed-access-token"));
}

/// No persister → no callback, connect still succeeds (existing behavior).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connect_with_without_persister_does_not_require_one() {
    let server = spawn_tidal_mock(vec![]);
    let service = TidalProvider::connect_with(
        &stale_config("test-client-id"),
        Some(&server.base_url),
        Some(&server.base_url),
        None,
    )
    .expect("connect_with");
    assert_eq!(service.access_token(), Some("refreshed-access-token"));
}

/// Empty / whitespace client id must keep the built-in default instead of
/// overriding it (aligned with `ServiceManager::connect_tidal`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connect_with_skips_blank_client_id() {
    let token_bodies: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&token_bodies);
    let server = spawn_mock_server(move |req| {
        if req.method == "POST" && req.path == "/token" {
            captured
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(req.body.clone());
            MockResponse::json(200, TOKEN_BODY)
        } else if req.method == "GET" && req.path.starts_with("/sessions") {
            MockResponse::json(200, SESSION_BODY)
        } else {
            MockResponse::json(404, "{}")
        }
    });

    // Control: an explicit client id is sent to the auth server.
    TidalProvider::connect_with(
        &stale_config("test-client-id"),
        Some(&server.base_url),
        Some(&server.base_url),
        None,
    )
    .expect("connect_with explicit client id");
    // A whitespace-only client id must not reach the auth server.
    TidalProvider::connect_with(
        &stale_config("   "),
        Some(&server.base_url),
        Some(&server.base_url),
        None,
    )
    .expect("connect_with blank client id");

    let bodies = token_bodies.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(bodies.len(), 2, "token requests: {bodies:?}");
    assert!(
        bodies[0].contains("client_id=test-client-id"),
        "explicit client id missing: {}",
        bodies[0]
    );
    assert!(
        !bodies[1].contains("client_id=+") && !bodies[1].contains("client_id=%20"),
        "blank client id leaked into the token request: {}",
        bodies[1]
    );
}

/// A failing favorites listing is a top-level scan error (HTTP 500 →
/// `NetworkError` in the service crate → `ProviderError::Network`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn favorites_listing_failure_surfaces_network_error() {
    let server = spawn_mock_server(move |req| {
        if req.method == "GET" && req.path.starts_with("/sessions") {
            MockResponse::json(200, SESSION_BODY)
        } else if req.method == "GET" && req.path.starts_with("/users/42/favorites/albums") {
            MockResponse::json(500, r#"{"error":"boom"}"#)
        } else {
            MockResponse::json(404, "{}")
        }
    });
    let provider = provider_for(&server);

    let err = provider
        .fetch_all_albums()
        .await
        .expect_err("listing failure must abort the scan");
    assert!(
        matches!(err, ProviderError::Network(_)),
        "expected ProviderError::Network, got: {err}"
    );
}

/// One album's track listing 404s while another succeeds → the scan returns
/// the good album (partial degradation instead of zeroing out the source).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn album_tracks_failure_skips_only_that_album() {
    let server = spawn_mock_server(move |req| {
        if req.method == "GET" && req.path.starts_with("/sessions") {
            MockResponse::json(200, SESSION_BODY)
        } else if req.method == "GET" && req.path.starts_with("/users/42/favorites/albums") {
            MockResponse::json(200, favorites_body(&[ALBUM_ITEM, ALBUM_ITEM_8], 2))
        } else if req.method == "GET" && req.path.starts_with("/albums/7/tracks") {
            MockResponse::json(200, ALBUM_TRACKS_BODY)
        } else if req.method == "GET" && req.path.starts_with("/albums/8/tracks") {
            MockResponse::json(404, r#"{"error":"gone"}"#)
        } else {
            MockResponse::json(404, "{}")
        }
    });
    let provider = provider_for(&server);

    let albums = provider.fetch_all_albums().await.expect("fetch_all_albums");
    assert_eq!(albums.len(), 1, "albums: {albums:?}");
    assert_eq!(albums[0].external_id, "7");
    assert_eq!(albums[0].tracks.len(), 2);
}
