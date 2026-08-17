//! Integration tests for `SpotifyProvider` against a local mock HTTP server.
//!
//! The service is injected via `SpotifyProvider::with_service`, with its
//! Web API pointed at the mock through `SpotifyService::with_test_web_api`.

#![cfg(feature = "spotify")]

mod common;

use common::{MockResponse, spawn_mock_server};
use sotf_audio::decoder::{AudioSource, ServiceId};
use sotf_federation::{
    LibraryProvider, ProviderError, SourceId, SourceType, SpotifyProvider, SpotifyProviderConfig,
};
use sotf_service_spotify::SpotifyService;

const IMAGE_BYTES: &[u8] = b"\xff\xd8\xff\xe0fake-jpeg-bytes";

fn album_json(host: &str) -> String {
    format!(
        r#"{{"id":"alb1","name":"The Wall","artists":[{{"name":"Pink Floyd"}}],
            "images":[{{"url":"http://{host}/image/alb1.jpg"}}],
            "release_date":"1979-11-30","total_tracks":2}}"#
    )
}

/// Mock Spotify Web API: saved albums, album metadata + tracks, and an image
/// endpoint for the album art test.
fn spawn_spotify_mock(
    saved_albums_body: impl Fn(&str) -> String + Send + Sync + 'static,
) -> common::MockServer {
    spawn_mock_server(move |req| {
        if req.method == "GET" && req.path.starts_with("/me/albums") {
            MockResponse::json(200, saved_albums_body(&req.host))
        } else if req.method == "GET" && req.path == "/albums/alb1" {
            MockResponse::json(200, album_json(&req.host))
        } else if req.method == "GET" && req.path.starts_with("/albums/alb1/tracks") {
            MockResponse::json(
                200,
                r#"{"items":[
                    {"id":"t1","name":"In the Flesh?","duration_ms":200000,"track_number":1,
                     "artists":[{"name":"Pink Floyd"}]},
                    {"id":"t2","name":"The Thin Ice","duration_ms":149000,"track_number":2,
                     "artists":[{"name":"Pink Floyd"}]}],
                   "next":null}"#,
            )
        } else if req.method == "GET" && req.path == "/image/alb1.jpg" {
            MockResponse::image(IMAGE_BYTES.to_vec())
        } else {
            MockResponse::json(404, r#"{"error":"unexpected path"}"#)
        }
    })
}

fn saved_albums_page(host: &str) -> String {
    format!(
        r#"{{"items":[{{"added_at":"2026-01-01T00:00:00Z","album":{}}}],"next":null}}"#,
        album_json(host)
    )
}

fn provider_for(server: &common::MockServer) -> SpotifyProvider {
    let service = SpotifyService::new().with_test_web_api(&server.base_url, "test-access-token");
    SpotifyProvider::with_service(SourceId("spotify:test".to_string()), service)
}

#[tokio::test]
async fn fetch_all_albums_maps_saved_albums_to_provider_albums() {
    let server = spawn_spotify_mock(saved_albums_page);
    let provider = provider_for(&server);

    assert_eq!(provider.source_type(), SourceType::Spotify);
    assert_eq!(provider.display_name(), "Spotify");
    let caps = provider.capabilities();
    assert!(!caps.seekable); // librespot PCM stream
    assert!(caps.has_album_art);
    assert!(!caps.writable);
    assert!(!caps.supports_events);

    let albums = provider.fetch_all_albums().await.expect("fetch_all_albums");
    assert_eq!(albums.len(), 1);
    let album = &albums[0];
    assert_eq!(album.external_id, "alb1");
    assert_eq!(album.title, "The Wall");
    assert_eq!(album.artist, "Pink Floyd");
    assert_eq!(album.year, Some(1979));
    assert!(album.album_art_url.is_some());

    assert_eq!(album.tracks.len(), 2);
    let track = &album.tracks[1];
    assert_eq!(track.external_id, "t2");
    assert_eq!(track.title, "The Thin Ice");
    assert_eq!(track.track_number, Some(2));
    assert_eq!(track.duration_secs, Some(149.0));
    assert_eq!(
        track.audio_source,
        AudioSource::ServiceStream {
            service: ServiceId::Spotify,
            track_id: "t2".to_string(),
        }
    );
}

#[tokio::test]
async fn resolve_source_defers_to_service_stream() {
    let server = spawn_spotify_mock(saved_albums_page);
    let provider = provider_for(&server);

    let source = provider.resolve_source("t1").await.expect("resolve_source");
    assert_eq!(
        source,
        AudioSource::ServiceStream {
            service: ServiceId::Spotify,
            track_id: "t1".to_string(),
        }
    );
}

#[tokio::test]
async fn fetch_album_art_downloads_cached_url() {
    let server = spawn_spotify_mock(saved_albums_page);
    let provider = provider_for(&server);
    provider.fetch_all_albums().await.expect("fetch_all_albums");

    let art = provider
        .fetch_album_art("alb1")
        .await
        .expect("fetch_album_art");
    assert_eq!(art.as_deref(), Some(IMAGE_BYTES));

    // Unknown album ids have no cached art URL.
    let art = provider
        .fetch_album_art("unknown")
        .await
        .expect("fetch_album_art");
    assert!(art.is_none());
}

#[tokio::test]
async fn empty_saved_albums_yield_no_albums() {
    let server = spawn_spotify_mock(|_host| r#"{"items":[],"next":null}"#.to_string());
    let provider = provider_for(&server);
    let albums = provider.fetch_all_albums().await.expect("fetch_all_albums");
    assert!(albums.is_empty());
}

#[tokio::test]
async fn new_fails_without_cached_credentials() {
    let dir = tempfile::tempdir().expect("tempdir");
    let result = SpotifyProvider::new(
        SourceId("spotify:test".to_string()),
        SpotifyProviderConfig {
            cache_dir: dir.path().to_path_buf(),
        },
    )
    .await;
    let err = result
        .err()
        .expect("construction must fail without cached credentials");
    assert!(
        err.to_string().contains("cached Spotify credentials"),
        "got: {err}"
    );
}

/// A failing saved-albums listing is a top-level scan error (HTTP 500 →
/// `NetworkError` in the service crate → `ProviderError::Network`).
#[tokio::test]
async fn saved_albums_listing_failure_surfaces_network_error() {
    let server = spawn_mock_server(move |req| {
        if req.method == "GET" && req.path.starts_with("/me/albums") {
            MockResponse::json(500, r#"{"error":"boom"}"#)
        } else {
            MockResponse::json(404, r#"{"error":"unexpected path"}"#)
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
#[tokio::test]
async fn album_tracks_failure_skips_only_that_album() {
    let server = spawn_mock_server(move |req| {
        if req.method == "GET" && req.path.starts_with("/me/albums") {
            MockResponse::json(
                200,
                format!(
                    r#"{{"items":[
                        {{"added_at":"2026-01-01T00:00:00Z","album":{}}},
                        {{"added_at":"2026-01-01T00:00:00Z","album":{}}}],
                       "next":null}}"#,
                    album_json(&req.host),
                    album_json(&req.host).replace("\"alb1\"", "\"alb2\""),
                ),
            )
        } else if req.method == "GET" && req.path.starts_with("/albums/alb1/tracks") {
            MockResponse::json(
                200,
                r#"{"items":[
                    {"id":"t1","name":"In the Flesh?","duration_ms":200000,"track_number":1,
                     "artists":[{"name":"Pink Floyd"}]}],
                   "next":null}"#,
            )
        } else if req.method == "GET" && req.path == "/albums/alb1" {
            MockResponse::json(200, album_json(&req.host))
        } else if req.method == "GET" && req.path.starts_with("/albums/alb2/tracks") {
            MockResponse::json(404, r#"{"error":"gone"}"#)
        } else if req.method == "GET" && req.path == "/albums/alb2" {
            MockResponse::json(200, album_json(&req.host).replace("\"alb1\"", "\"alb2\""))
        } else {
            MockResponse::json(404, r#"{"error":"unexpected path"}"#)
        }
    });
    let provider = provider_for(&server);

    let albums = provider.fetch_all_albums().await.expect("fetch_all_albums");
    assert_eq!(albums.len(), 1, "albums: {albums:?}");
    assert_eq!(albums[0].external_id, "alb1");
    assert_eq!(albums[0].tracks.len(), 1);
}
