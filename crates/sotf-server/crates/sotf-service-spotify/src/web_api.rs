//! Spotify Web API (`api.spotify.com/v1`) client.
//!
//! Uses the OAuth access token obtained at login for search and library
//! endpoints. All methods are sync (the `StreamingService` trait is sync);
//! async HTTP is driven through the shared `AsyncRuntime`.

use crate::async_runtime::AsyncRuntime;
use crate::consts::{SPOTIFY_API_BASE, SPOTIFY_CLIENT_ID, SPOTIFY_TOKEN_URL, read_bounded_json};
use crate::misc::{parse_release_year, truncate_for_log};
use serde::Deserialize;
use sotf_services::*;
use std::sync::Arc;

/// Page size used for paged endpoints (Spotify's maximum).
const PAGE_LIMIT: u32 = 50;

/// Hard cap on how many `next` pages we follow — guards against a server
/// that never nulls out `next`.
const MAX_PAGES: usize = 20;

// ---------------------------------------------------------------------------
// Spotify JSON shapes (only the fields we consume)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SpotifyImage {
    url: String,
}

#[derive(Debug, Deserialize)]
struct SpotifyArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct SpotifyAlbum {
    id: String,
    name: String,
    #[serde(default)]
    artists: Vec<SpotifyArtist>,
    #[serde(default)]
    images: Vec<SpotifyImage>,
    release_date: Option<String>,
    total_tracks: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SpotifyTrack {
    id: Option<String>,
    name: Option<String>,
    duration_ms: Option<f64>,
    track_number: Option<u32>,
    #[serde(default)]
    artists: Vec<SpotifyArtist>,
    /// Absent on the simplified track objects returned by
    /// `/albums/{id}/tracks`.
    album: Option<SpotifyAlbum>,
}

/// Paging envelope. Items are `Option<T>` because Spotify returns literal
/// `null` entries for content that became unavailable (e.g. removed saved
/// tracks).
#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct Page<T> {
    #[serde(default)]
    items: Vec<Option<T>>,
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchTracksResponse {
    tracks: Option<Page<SpotifyTrack>>,
}

#[derive(Debug, Deserialize)]
struct SearchAlbumsResponse {
    albums: Option<Page<SpotifyAlbum>>,
}

#[derive(Debug, Deserialize)]
struct SavedAlbumEntry {
    album: SpotifyAlbum,
}

#[derive(Debug, Deserialize)]
struct SavedTrackEntry {
    track: SpotifyTrack,
}

// ---------------------------------------------------------------------------
// Mapping to sotf-services types
// ---------------------------------------------------------------------------

fn map_track(track: &SpotifyTrack, fallback_album: Option<&SpotifyAlbum>) -> ServiceTrack {
    let album = track.album.as_ref().or(fallback_album);
    ServiceTrack {
        id: track.id.clone().unwrap_or_default(),
        title: track.name.clone().unwrap_or_default(),
        artist: track
            .artists
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default(),
        album: album.map(|a| a.name.clone()).unwrap_or_default(),
        duration_secs: track.duration_ms.unwrap_or(0.0) / 1000.0,
        track_number: track.track_number,
        album_art_url: album.and_then(|a| a.images.first()).map(|i| i.url.clone()),
        // Spotify via librespot tops out at Vorbis ~320 kbps.
        available_qualities: vec![AudioQuality::Low, AudioQuality::Normal, AudioQuality::High],
    }
}

fn map_album(album: &SpotifyAlbum) -> ServiceAlbum {
    ServiceAlbum {
        id: album.id.clone(),
        title: album.name.clone(),
        artist: album
            .artists
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default(),
        year: album.release_date.as_deref().and_then(parse_release_year),
        track_count: album.total_tracks.unwrap_or(0),
        album_art_url: album.images.first().map(|i| i.url.clone()),
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// A pagination `next` URL is only followed when it stays on the same origin
/// (scheme, host, port) as the configured API base. A bare `starts_with`
/// prefix check would let `https://api.spotify.com.evil.example/…` through
/// and leak the bearer token to a third party.
fn is_same_origin(api_base: &str, next: &str) -> bool {
    let (Ok(base), Ok(next)) = (url::Url::parse(api_base), url::Url::parse(next)) else {
        return false;
    };
    base.scheme() == next.scheme()
        && base.host_str() == next.host_str()
        && base.port_or_known_default() == next.port_or_known_default()
}

pub(crate) struct SpotifyWebApi {
    client: reqwest::Client,
    rt: Arc<AsyncRuntime>,
    /// Interior mutability so a 401-triggered refresh can install the new
    /// token without needing `&mut self` on the sync trait surface.
    access_token: std::sync::RwLock<String>,
    api_base: String,
    /// OAuth token endpoint used for refresh grants.
    token_url: String,
    /// Refresh material; `None` when the service only holds a bare access
    /// token (e.g. `authenticate(AccessToken)`), in which case a 401 stays
    /// an `AuthError`.
    refresh: Option<std::sync::Mutex<RefreshState>>,
}

/// Refresh-grant state. Rotated tokens are persisted back to `cache_dir`
/// (when set) so the next restart restores the fresh pair.
struct RefreshState {
    refresh_token: String,
    cache_dir: Option<std::path::PathBuf>,
}

impl std::fmt::Debug for SpotifyWebApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let token = self
            .access_token
            .read()
            .map(|t| redact_secret(&t))
            .unwrap_or_else(|_| "<locked>".to_string());
        f.debug_struct("SpotifyWebApi")
            .field("access_token", &token)
            .field("api_base", &self.api_base)
            .field("refresh", &self.refresh.is_some())
            .finish()
    }
}

impl SpotifyWebApi {
    pub(crate) fn new(access_token: String, rt: Arc<AsyncRuntime>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            rt,
            access_token: std::sync::RwLock::new(access_token),
            api_base: SPOTIFY_API_BASE.to_string(),
            token_url: SPOTIFY_TOKEN_URL.to_string(),
            refresh: None,
        }
    }

    /// Test seam: point the client at a mock server. Used by this crate's
    /// unit tests and by downstream integration tests via
    /// [`crate::SpotifyService::with_test_web_api`].
    pub(crate) fn with_api_base(mut self, api_base: &str) -> Self {
        self.api_base = api_base.trim_end_matches('/').to_string();
        self
    }

    /// Test seam: point the refresh grant at a mock token endpoint. Mirrors
    /// the real layout, where the token endpoint lives at `/api/token`.
    #[cfg(test)]
    pub(crate) fn with_auth_base(mut self, auth_base: &str) -> Self {
        self.token_url = format!("{}/api/token", auth_base.trim_end_matches('/'));
        self
    }

    /// Attach a refresh token so a 401 can be recovered by refreshing once
    /// and retrying (see `get_json`).
    pub(crate) fn with_refresh(
        mut self,
        cache_dir: Option<std::path::PathBuf>,
        refresh_token: String,
    ) -> Self {
        self.refresh = Some(std::sync::Mutex::new(RefreshState {
            refresh_token,
            cache_dir,
        }));
        self
    }

    fn bearer_token(&self) -> String {
        self.access_token
            .read()
            .map(|t| t.clone())
            .unwrap_or_default()
    }

    /// Attempt one refresh-token grant; on success installs the new access
    /// token and persists the rotated pair. Returns false when no refresh
    /// token is available or the grant failed (the caller then surfaces the
    /// original 401).
    async fn refresh_access_token(&self) -> bool {
        let Some(refresh) = &self.refresh else {
            return false;
        };
        // Grab the grant material and drop the guard before awaiting (a std
        // mutex guard must not be held across an await point).
        let (refresh_token, cache_dir) = match refresh.lock() {
            Ok(state) => (state.refresh_token.clone(), state.cache_dir.clone()),
            Err(_) => return false,
        };
        match crate::oauth::refresh_access_token(&self.token_url, SPOTIFY_CLIENT_ID, &refresh_token)
            .await
        {
            Ok(token) => {
                if let Ok(mut state) = refresh.lock() {
                    state.refresh_token = token.refresh_token.clone();
                }
                if let Some(dir) = &cache_dir
                    && let Err(e) = token.save(dir)
                {
                    log::warn!("[Spotify] Failed to persist refreshed Web API token: {e}");
                }
                if let Ok(mut guard) = self.access_token.write() {
                    *guard = token.access_token;
                }
                true
            }
            Err(e) => {
                log::warn!("[Spotify] Web API token refresh failed: {e}");
                false
            }
        }
    }

    /// GET `url` (absolute) with bearer auth, decoding the body as bounded
    /// JSON. Non-2xx responses become `AuthError` (401/403) or `NetworkError`
    /// with the response body truncated for safe logging.
    ///
    /// On a 401 the token is refreshed once and the request retried exactly
    /// once; a second 401 (or a failed refresh) surfaces the auth error.
    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        query: &[(String, String)],
    ) -> Result<T, ServiceError> {
        let mut resp = self.send_get(url, query).await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED && self.refresh_access_token().await {
            resp = self.send_get(url, query).await?;
        }
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let body = truncate_for_log(&body, 512);
            let make_error: fn(String) -> ServiceError = if status
                == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                ServiceError::AuthError
            } else {
                ServiceError::NetworkError
            };
            return Err(make_error(format!(
                "Spotify API error: HTTP {status} ({body})"
            )));
        }
        read_bounded_json(resp).await
    }

    async fn send_get(
        &self,
        url: &str,
        query: &[(String, String)],
    ) -> Result<reqwest::Response, ServiceError> {
        let mut request = self.client.get(url).bearer_auth(self.bearer_token());
        if !query.is_empty() {
            request = request.query(query);
        }
        request
            .send()
            .await
            .map_err(|e| ServiceError::NetworkError(e.to_string()))
    }

    /// Flatten a paged endpoint, following `next` links (bounded by
    /// `MAX_PAGES`, and only while they stay on the same origin as
    /// `api_base`).
    async fn fetch_all_pages<T: serde::de::DeserializeOwned>(
        &self,
        mut page: Page<T>,
    ) -> Result<Vec<T>, ServiceError> {
        let mut out: Vec<T> = Vec::new();
        let mut pages = 0usize;
        loop {
            out.extend(page.items.into_iter().flatten());
            pages += 1;
            let Some(next) = page.next else { break };
            if pages >= MAX_PAGES {
                log::warn!("[Spotify] pagination capped at {MAX_PAGES} pages");
                break;
            }
            if !is_same_origin(&self.api_base, &next) {
                return Err(ServiceError::NetworkError(format!(
                    "Refusing to follow pagination URL outside the API base: {}",
                    truncate_for_log(&next, 128)
                )));
            }
            page = self.get_json(&next, &[]).await?;
        }
        Ok(out)
    }

    pub(crate) fn search_tracks(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<ServiceTrack>, ServiceError> {
        self.rt.block_on(async {
            let resp: SearchTracksResponse = self
                .get_json(
                    &format!("{}/search", self.api_base),
                    &[
                        ("q".to_string(), query.to_string()),
                        ("type".to_string(), "track".to_string()),
                        ("limit".to_string(), limit.min(PAGE_LIMIT).to_string()),
                    ],
                )
                .await?;
            Ok(resp
                .tracks
                .map(|p| {
                    p.items
                        .into_iter()
                        .flatten()
                        .map(|t| map_track(&t, None))
                        .collect()
                })
                .unwrap_or_default())
        })
    }

    pub(crate) fn search_albums(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<ServiceAlbum>, ServiceError> {
        self.rt.block_on(async {
            let resp: SearchAlbumsResponse = self
                .get_json(
                    &format!("{}/search", self.api_base),
                    &[
                        ("q".to_string(), query.to_string()),
                        ("type".to_string(), "album".to_string()),
                        ("limit".to_string(), limit.min(PAGE_LIMIT).to_string()),
                    ],
                )
                .await?;
            Ok(resp
                .albums
                .map(|p| {
                    p.items
                        .into_iter()
                        .flatten()
                        .map(|a| map_album(&a))
                        .collect()
                })
                .unwrap_or_default())
        })
    }

    pub(crate) fn album_tracks(&self, album_id: &str) -> Result<Vec<ServiceTrack>, ServiceError> {
        self.rt.block_on(async {
            // The paged track objects are "simplified" (no album field), so
            // fetch the album first to fill in album name / art.
            let album: SpotifyAlbum = self
                .get_json(&format!("{}/albums/{album_id}", self.api_base), &[])
                .await?;
            let first: Page<SpotifyTrack> = self
                .get_json(
                    &format!("{}/albums/{album_id}/tracks", self.api_base),
                    &[
                        ("limit".to_string(), PAGE_LIMIT.to_string()),
                        ("offset".to_string(), "0".to_string()),
                    ],
                )
                .await?;
            let tracks = self.fetch_all_pages(first).await?;
            Ok(tracks.iter().map(|t| map_track(t, Some(&album))).collect())
        })
    }

    pub(crate) fn saved_albums(&self) -> Result<Vec<ServiceAlbum>, ServiceError> {
        self.rt.block_on(async {
            let first: Page<SavedAlbumEntry> = self
                .get_json(
                    &format!("{}/me/albums", self.api_base),
                    &[
                        ("limit".to_string(), PAGE_LIMIT.to_string()),
                        ("offset".to_string(), "0".to_string()),
                    ],
                )
                .await?;
            let entries = self.fetch_all_pages(first).await?;
            Ok(entries.iter().map(|e| map_album(&e.album)).collect())
        })
    }

    pub(crate) fn saved_tracks(&self) -> Result<Vec<ServiceTrack>, ServiceError> {
        self.rt.block_on(async {
            let first: Page<SavedTrackEntry> = self
                .get_json(
                    &format!("{}/me/tracks", self.api_base),
                    &[
                        ("limit".to_string(), PAGE_LIMIT.to_string()),
                        ("offset".to_string(), "0".to_string()),
                    ],
                )
                .await?;
            let entries = self.fetch_all_pages(first).await?;
            Ok(entries.iter().map(|e| map_track(&e.track, None)).collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{MockRequest, spawn_mock_server};

    fn test_api(server_base: &str) -> SpotifyWebApi {
        let rt = Arc::new(AsyncRuntime::new().unwrap());
        SpotifyWebApi::new("test-access-token".to_string(), rt).with_api_base(server_base)
    }

    fn assert_bearer(req: &MockRequest) {
        assert!(
            req.headers
                .to_lowercase()
                .contains("authorization: bearer test-access-token"),
            "missing bearer token, headers: {}",
            req.headers
        );
    }

    #[test]
    fn search_tracks_maps_spotify_json() {
        let server = spawn_mock_server(|req| {
            assert_bearer(req);
            assert!(req.path.starts_with("/search"), "got: {}", req.path);
            assert!(req.path.contains("type=track"), "got: {}", req.path);
            (
                200,
                r#"{"tracks":{"items":[{"id":"track1","name":"Comfortably Numb",
                    "duration_ms":382000,"track_number":6,
                    "artists":[{"name":"Pink Floyd"}],
                    "album":{"id":"alb1","name":"The Wall",
                             "artists":[{"name":"Pink Floyd"}],
                             "images":[{"url":"https://i.scdn.co/image/abc"}],
                             "release_date":"1979-11-30","total_tracks":26}}],
                    "next":null}}"#
                    .to_string(),
            )
        });

        let api = test_api(&server.base_url);
        let tracks = api.search_tracks("comfortably numb", 10).unwrap();
        assert_eq!(tracks.len(), 1);
        let t = &tracks[0];
        assert_eq!(t.id, "track1");
        assert_eq!(t.title, "Comfortably Numb");
        assert_eq!(t.artist, "Pink Floyd");
        assert_eq!(t.album, "The Wall");
        assert!((t.duration_secs - 382.0).abs() < 1e-9);
        assert_eq!(t.track_number, Some(6));
        assert_eq!(
            t.album_art_url.as_deref(),
            Some("https://i.scdn.co/image/abc")
        );
        assert!(t.available_qualities.contains(&AudioQuality::High));
    }

    #[test]
    fn search_albums_maps_year_and_track_count() {
        let server = spawn_mock_server(|req| {
            assert!(req.path.contains("type=album"), "got: {}", req.path);
            (
                200,
                r#"{"albums":{"items":[{"id":"alb9","name":"Animals",
                    "artists":[{"name":"Pink Floyd"}],
                    "images":[{"url":"https://i.scdn.co/image/xyz"}],
                    "release_date":"1977-01-21","total_tracks":5}],
                    "next":null}}"#
                    .to_string(),
            )
        });

        let api = test_api(&server.base_url);
        let albums = api.search_albums("animals", 5).unwrap();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].id, "alb9");
        assert_eq!(albums[0].title, "Animals");
        assert_eq!(albums[0].artist, "Pink Floyd");
        assert_eq!(albums[0].year, Some(1977));
        assert_eq!(albums[0].track_count, 5);
    }

    #[test]
    fn album_tracks_follows_pagination_and_fills_album_fields() {
        let server = spawn_mock_server(|req| {
            assert_bearer(req);
            if req.path == "/albums/alb1" {
                (
                    200,
                    r#"{"id":"alb1","name":"The Wall",
                        "artists":[{"name":"Pink Floyd"}],
                        "images":[{"url":"https://i.scdn.co/image/abc"}],
                        "release_date":"1979-11-30","total_tracks":2}"#
                        .to_string(),
                )
            } else if req.path.starts_with("/albums/alb1/tracks") && req.path.contains("offset=0") {
                let next = format!("http://{}/albums/alb1/tracks?limit=50&offset=50", req.host);
                (
                    200,
                    format!(
                        r#"{{"items":[{{"id":"t1","name":"In the Flesh?",
                            "duration_ms":200000,"track_number":1,
                            "artists":[{{"name":"Pink Floyd"}}]}}],
                            "next":"{next}"}}"#
                    ),
                )
            } else if req.path.starts_with("/albums/alb1/tracks") && req.path.contains("offset=50")
            {
                (
                    200,
                    r#"{"items":[{"id":"t2","name":"The Thin Ice",
                        "duration_ms":149000,"track_number":2,
                        "artists":[{"name":"Pink Floyd"}]}],
                        "next":null}"#
                        .to_string(),
                )
            } else {
                (404, r#"{"error":"unexpected path"}"#.to_string())
            }
        });

        let api = test_api(&server.base_url);
        let tracks = api.album_tracks("alb1").unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].id, "t1");
        assert_eq!(tracks[1].id, "t2");
        // Simplified track objects lack album info; it must come from the
        // album fetch.
        for t in &tracks {
            assert_eq!(t.album, "The Wall");
            assert_eq!(
                t.album_art_url.as_deref(),
                Some("https://i.scdn.co/image/abc")
            );
        }
        assert_eq!(tracks[1].track_number, Some(2));
    }

    #[test]
    fn saved_endpoints_unwrap_nested_items_and_skip_nulls() {
        let server = spawn_mock_server(|req| {
            if req.path.starts_with("/me/albums") {
                (
                    200,
                    r#"{"items":[{"added_at":"2024-01-01T00:00:00Z",
                        "album":{"id":"alb1","name":"The Wall",
                                 "artists":[{"name":"Pink Floyd"}],
                                 "images":[],"release_date":"1979-11-30",
                                 "total_tracks":26}}, null],
                        "next":null}"#
                        .to_string(),
                )
            } else if req.path.starts_with("/me/tracks") {
                (
                    200,
                    r#"{"items":[{"added_at":"2024-01-01T00:00:00Z",
                        "track":{"id":"t1","name":"Hey You","duration_ms":272000,
                                 "track_number":1,
                                 "artists":[{"name":"Pink Floyd"}],
                                 "album":null}}, null],
                        "next":null}"#
                        .to_string(),
                )
            } else {
                (404, r#"{"error":"unexpected path"}"#.to_string())
            }
        });

        let api = test_api(&server.base_url);
        let albums = api.saved_albums().unwrap();
        assert_eq!(albums.len(), 1, "null entries must be skipped");
        assert_eq!(albums[0].id, "alb1");

        let tracks = api.saved_tracks().unwrap();
        assert_eq!(tracks.len(), 1, "null entries must be skipped");
        assert_eq!(tracks[0].id, "t1");
        assert!((tracks[0].duration_secs - 272.0).abs() < 1e-9);
    }

    #[test]
    fn api_error_truncates_body_and_maps_401_to_auth_error() {
        let long_body = "x".repeat(5000);
        let server = spawn_mock_server(move |_req| (401, long_body.clone()));

        let api = test_api(&server.base_url);
        let result = api.search_tracks("anything", 5);
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("401"), "got: {msg}");
                assert!(msg.len() < 1000, "error body not truncated: {}", msg.len());
            }
            other => panic!("expected AuthError, got: {other:?}"),
        }
    }

    #[test]
    fn api_error_500_maps_to_network_error() {
        let server = spawn_mock_server(|_req| (500, r#"{"error":"boom"}"#.to_string()));
        let api = test_api(&server.base_url);
        match api.search_albums("anything", 5) {
            Err(ServiceError::NetworkError(msg)) => assert!(msg.contains("500"), "got: {msg}"),
            other => panic!("expected NetworkError, got: {other:?}"),
        }
    }

    #[test]
    fn pagination_url_outside_api_base_is_rejected() {
        let server = spawn_mock_server(|_req| {
            (
                200,
                r#"{"items":[],"next":"http://evil.example.com/steal"}"#.to_string(),
            )
        });
        let api = test_api(&server.base_url);
        match api.saved_tracks() {
            Err(ServiceError::NetworkError(msg)) => {
                assert!(msg.contains("outside the API base"), "got: {msg}")
            }
            other => panic!("expected NetworkError, got: {other:?}"),
        }
    }

    #[test]
    fn same_origin_check_has_host_boundary() {
        let base = "https://api.spotify.com/v1";
        assert!(is_same_origin(
            base,
            "https://api.spotify.com/v1/me/tracks?offset=50"
        ));
        assert!(is_same_origin(
            base,
            "https://api.spotify.com:443/v1/me/tracks"
        ));
        // The review's example: a bare prefix check lets these through.
        assert!(!is_same_origin(
            base,
            "https://api.spotify.com.evil.example/v1/me"
        ));
        assert!(!is_same_origin(
            base,
            "https://api.spotify.com.v1.evil.example/x"
        ));
        assert!(!is_same_origin(base, "http://api.spotify.com/v1/me"));
        assert!(!is_same_origin(base, "https://api.spotify.com:444/v1/me"));
        assert!(!is_same_origin(base, "not a url"));
    }

    #[test]
    fn pagination_url_sharing_string_prefix_is_rejected() {
        // `{api_base}.evil…` satisfies the old `starts_with` check but must be
        // rejected by the origin comparison.
        let server = spawn_mock_server(|req| {
            (
                200,
                format!(
                    r#"{{"items":[],"next":"http://{}.evil.example/steal"}}"#,
                    req.host
                ),
            )
        });
        let api = test_api(&server.base_url);
        match api.saved_tracks() {
            Err(ServiceError::NetworkError(msg)) => {
                assert!(msg.contains("outside the API base"), "got: {msg}")
            }
            other => panic!("expected NetworkError, got: {other:?}"),
        }
    }

    #[test]
    fn get_json_refreshes_once_on_401_and_retries() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let get_count = Arc::new(AtomicUsize::new(0));
        let refresh_count = Arc::new(AtomicUsize::new(0));
        let get_count2 = Arc::clone(&get_count);
        let refresh_count2 = Arc::clone(&refresh_count);
        let server = spawn_mock_server(move |req| {
            if req.method == "POST" && req.path == "/api/token" {
                refresh_count2.fetch_add(1, Ordering::SeqCst);
                assert!(
                    req.body.contains("grant_type=refresh_token"),
                    "{}",
                    req.body
                );
                assert!(req.body.contains("refresh_token=refresh-1"), "{}", req.body);
                return (
                    200,
                    r#"{"access_token":"new-access-token","token_type":"Bearer",
                        "expires_in":3600,"refresh_token":"refresh-2"}"#
                        .to_string(),
                );
            }
            // GET: reject the stale token, accept the refreshed one.
            get_count2.fetch_add(1, Ordering::SeqCst);
            if req
                .headers
                .to_lowercase()
                .contains("authorization: bearer new-access-token")
            {
                (200, r#"{"items":[],"next":null}"#.to_string())
            } else {
                (401, r#"{"error":"expired"}"#.to_string())
            }
        });

        let api = test_api(&server.base_url)
            .with_auth_base(&server.base_url)
            .with_refresh(None, "refresh-1".to_string());
        let tracks = api.saved_tracks().unwrap();
        assert!(tracks.is_empty());
        assert_eq!(get_count.load(Ordering::SeqCst), 2, "exactly one retry");
        assert_eq!(
            refresh_count.load(Ordering::SeqCst),
            1,
            "exactly one refresh"
        );
    }

    #[test]
    fn get_json_without_refresh_token_keeps_auth_error() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let count = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::clone(&count);
        let server = spawn_mock_server(move |_req| {
            count2.fetch_add(1, Ordering::SeqCst);
            (401, r#"{"error":"expired"}"#.to_string())
        });

        let api = test_api(&server.base_url);
        match api.saved_tracks() {
            Err(ServiceError::AuthError(msg)) => assert!(msg.contains("401"), "got: {msg}"),
            other => panic!("expected AuthError, got: {other:?}"),
        }
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "no retry without refresh token"
        );
    }

    #[test]
    fn get_json_surfaces_auth_error_when_refresh_fails() {
        let server = spawn_mock_server(|req| {
            if req.method == "POST" {
                return (400, r#"{"error":"invalid_grant"}"#.to_string());
            }
            (401, r#"{"error":"expired"}"#.to_string())
        });

        let api = test_api(&server.base_url)
            .with_auth_base(&server.base_url)
            .with_refresh(None, "refresh-1".to_string());
        match api.saved_tracks() {
            Err(ServiceError::AuthError(msg)) => assert!(msg.contains("401"), "got: {msg}"),
            other => panic!("expected AuthError, got: {other:?}"),
        }
    }

    #[test]
    fn restore_from_token_file_after_restart() {
        // Simulate a restart: a token file with an expired access token and a
        // refresh token is all that survives. Loading it, wiring the refresh
        // state, and hitting a 401 must refresh once, retry, and persist the
        // rotated pair back to the file.
        use crate::token_store::WebApiToken;
        let dir = std::env::temp_dir().join(format!(
            "sotf-spotify-restart-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        WebApiToken {
            access_token: "stale-access".to_string(),
            refresh_token: "refresh-1".to_string(),
            expires_at: 1, // long expired
        }
        .save(&dir)
        .unwrap();

        let loaded = WebApiToken::load(&dir).unwrap();
        assert!(loaded.is_expired());

        let server = spawn_mock_server(|req| {
            if req.method == "POST" && req.path == "/api/token" {
                return (
                    200,
                    r#"{"access_token":"fresh-access","token_type":"Bearer",
                        "expires_in":3600,"refresh_token":"refresh-2"}"#
                        .to_string(),
                );
            }
            if req
                .headers
                .to_lowercase()
                .contains("authorization: bearer fresh-access")
            {
                (
                    200,
                    r#"{"items":[{"added_at":"2024-01-01T00:00:00Z",
                        "track":{"id":"t1","name":"Hey You","duration_ms":272000,
                                 "track_number":1,"artists":[{"name":"Pink Floyd"}],
                                 "album":null}}],
                        "next":null}"#
                        .to_string(),
                )
            } else {
                (401, r#"{"error":"expired"}"#.to_string())
            }
        });

        let rt = Arc::new(AsyncRuntime::new().unwrap());
        let api = SpotifyWebApi::new(loaded.access_token.clone(), rt)
            .with_api_base(&server.base_url)
            .with_auth_base(&server.base_url)
            .with_refresh(Some(dir.clone()), loaded.refresh_token.clone());
        let tracks = api.saved_tracks().unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "t1");

        let persisted = WebApiToken::load(&dir).unwrap();
        assert_eq!(persisted.access_token, "fresh-access");
        assert_eq!(persisted.refresh_token, "refresh-2");
        assert!(!persisted.is_expired());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_release_year_handles_edge_cases() {
        assert_eq!(parse_release_year("1979-11-30"), Some(1979));
        assert_eq!(parse_release_year("1979"), Some(1979));
        assert_eq!(parse_release_year(""), None);
        assert_eq!(parse_release_year("19"), None);
        assert_eq!(parse_release_year("abcd-01-01"), None);
    }
}
