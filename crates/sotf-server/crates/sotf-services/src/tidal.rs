// ============================================================================
// Tidal Integration
// ============================================================================
//
// Uses Tidal's HTTP API for authentication, search, and stream URL resolution.
// Audio is delivered as direct FLAC/AAC URLs that the engine's HTTP decoder handles.
//
// HTTP is performed with `reqwest`'s async client. The `StreamingService` trait
// is synchronous, so each call is driven on a tokio runtime via a small helper
// that handles both "already inside a runtime" (using `block_in_place`) and
// "no runtime present" (using an embedded current-thread runtime) cases.

use crate::service::{redact_secret, *};
use serde::Deserialize;
use std::sync::Arc;

/// Tidal API base URL.
const API_BASE: &str = "https://api.tidal.com/v1";

/// Tidal auth base URL.
const AUTH_BASE: &str = "https://auth.tidal.com/v1/oauth2";

/// Tidal client ID for device code flow.
/// In production this should be configurable, not hardcoded.
const DEFAULT_CLIENT_ID: &str = "";

/// Maximum JSON response body size we are willing to parse (16 MiB).
/// Anything larger is treated as a network error to prevent a malicious or
/// misbehaving peer from exhausting memory.
const MAX_JSON_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Holds the runtime used to drive async HTTP calls when no ambient tokio
/// runtime is available.
struct AsyncRuntime {
    /// Fallback runtime used when the caller is not already inside one (or is
    /// inside a current-thread runtime where `block_in_place` would panic).
    fallback: tokio::runtime::Runtime,
}

impl AsyncRuntime {
    fn new() -> Result<Self, ServiceError> {
        let fallback = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| ServiceError::Other(format!("Failed to build tokio runtime: {}", e)))?;
        Ok(Self { fallback })
    }

    /// Drive `fut` to completion. Uses the ambient tokio runtime when running
    /// inside a multi-thread runtime (yielding the worker via
    /// `block_in_place`), otherwise falls back to the embedded current-thread
    /// runtime. This satisfies the "Tidal blocking inside async runtime"
    /// concern from the review without requiring the public trait to become
    /// async.
    fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => match handle.runtime_flavor() {
                tokio::runtime::RuntimeFlavor::MultiThread => {
                    tokio::task::block_in_place(|| handle.block_on(fut))
                }
                _ => self.fallback.block_on(fut),
            },
            Err(_) => self.fallback.block_on(fut),
        }
    }
}

pub struct TidalService {
    client: reqwest::Client,
    rt: Arc<AsyncRuntime>,
    client_id: String,
    access_token: Option<String>,
    #[allow(dead_code)]
    refresh_token: Option<String>,
    country_code: String,
    quality: AudioQuality,
}

impl std::fmt::Debug for TidalService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TidalService")
            .field("client_id", &redact_secret(&self.client_id))
            .field(
                "access_token",
                &self.access_token.as_deref().map(redact_secret),
            )
            .field(
                "refresh_token",
                &self.refresh_token.as_deref().map(redact_secret),
            )
            .field("country_code", &self.country_code)
            .field("quality", &self.quality)
            .finish()
    }
}

impl Default for TidalService {
    fn default() -> Self {
        Self::new()
    }
}

impl TidalService {
    pub fn new() -> Self {
        let rt = AsyncRuntime::new().expect("Failed to create async runtime for TidalService");
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            rt: Arc::new(rt),
            client_id: DEFAULT_CLIENT_ID.to_string(),
            access_token: None,
            refresh_token: None,
            country_code: "US".to_string(),
            quality: AudioQuality::Lossless,
        }
    }

    pub fn with_client_id(mut self, client_id: &str) -> Self {
        self.client_id = client_id.to_string();
        self
    }

    pub fn with_country_code(mut self, code: &str) -> Self {
        self.country_code = code.to_string();
        self
    }

    pub fn with_quality(mut self, quality: AudioQuality) -> Self {
        self.quality = quality;
        self
    }

    /// Issue a GET to `path` (relative to `API_BASE`) with bearer auth and the
    /// configured country code, returning the response asynchronously.
    async fn api_get_async(&self, path: &str) -> Result<reqwest::Response, ServiceError> {
        let token = self
            .access_token
            .as_ref()
            .ok_or_else(|| ServiceError::AuthError("Not authenticated".to_string()))?;

        let url = format!("{}{}", API_BASE, path);
        self.client
            .get(&url)
            .bearer_auth(token)
            .query(&[("countryCode", &self.country_code)])
            .send()
            .await
            .map_err(|e| ServiceError::NetworkError(e.to_string()))
    }

    fn quality_to_tidal_quality(&self) -> &str {
        match self.quality {
            AudioQuality::Low => "LOW",
            AudioQuality::Normal => "LOW",
            AudioQuality::High => "HIGH",
            AudioQuality::Lossless => "LOSSLESS",
            AudioQuality::HiRes => "HI_RES_LOSSLESS",
        }
    }
}

/// Read a response body and decode it as JSON, refusing to parse anything
/// larger than `MAX_JSON_BODY_BYTES`.
///
/// We bail out early when the server advertises a `Content-Length` greater
/// than the limit, and again after buffering in case the server omitted
/// `Content-Length` and over-sent.
async fn read_bounded_json<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T, ServiceError> {
    if let Some(len) = resp.content_length()
        && (len as usize) > MAX_JSON_BODY_BYTES
    {
        return Err(ServiceError::NetworkError(format!(
            "Response body too large: {} bytes (max {})",
            len, MAX_JSON_BODY_BYTES
        )));
    }

    let body = resp
        .bytes()
        .await
        .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

    if body.len() > MAX_JSON_BODY_BYTES {
        return Err(ServiceError::NetworkError(format!(
            "Response body exceeded {} bytes",
            MAX_JSON_BODY_BYTES
        )));
    }

    serde_json::from_slice(&body).map_err(|e| ServiceError::Other(e.to_string()))
}

impl StreamingService for TidalService {
    fn authenticate(&mut self, credentials: ServiceCredentials) -> Result<(), ServiceError> {
        match credentials {
            ServiceCredentials::AccessToken(token) => {
                // Log only the redacted prefix so the token doesn't end up in
                // log files.
                log::debug!("[Tidal] Validating access token {}", redact_secret(&token));
                self.access_token = Some(token);
                let rt = self.rt.clone();
                let result: Result<(), ServiceError> = rt.block_on(async {
                    let resp = self.api_get_async("/sessions").await?;
                    if resp.status().is_success() {
                        let session: TidalSession = read_bounded_json(resp).await?;
                        self.country_code = session.country_code.clone();
                        log::info!(
                            "[Tidal] Authenticated as user {} (country: {})",
                            session.user_id,
                            self.country_code
                        );
                        Ok(())
                    } else {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        let body = truncate_for_log(&body, 512);
                        Err(ServiceError::AuthError(format!(
                            "Token validation failed: HTTP {} ({})",
                            status, body
                        )))
                    }
                });
                if result.is_err() {
                    // Wipe the bad token so subsequent calls cannot
                    // accidentally forward it.
                    self.access_token = None;
                }
                result
            }
            ServiceCredentials::DeviceCode => {
                if self.client_id.is_empty() {
                    return Err(ServiceError::AuthError(
                        "Tidal client_id not configured. Use with_client_id() or set TIDAL_CLIENT_ID env var.".to_string(),
                    ));
                }

                let rt = self.rt.clone();
                let client = self.client.clone();
                let client_id = self.client_id.clone();
                rt.block_on(async move {
                    let resp = client
                        .post(format!("{}/device_authorization", AUTH_BASE))
                        .form(&[
                            ("client_id", client_id.as_str()),
                            ("scope", "r_usr+w_usr+w_sub"),
                        ])
                        .send()
                        .await
                        .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

                    if !resp.status().is_success() {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        let body = truncate_for_log(&body, 512);
                        return Err(ServiceError::AuthError(format!(
                            "Device code request failed: HTTP {} ({})",
                            status, body
                        )));
                    }

                    let device_auth: TidalDeviceAuth = read_bounded_json(resp).await?;

                    // Return the verification URL for the user to visit.
                    Err(ServiceError::AuthError(format!(
                        "Visit {} and enter code: {}",
                        device_auth
                            .verification_uri_complete
                            .as_deref()
                            .unwrap_or(&device_auth.verification_uri),
                        device_auth.user_code
                    )))
                })
            }
            _ => Err(ServiceError::AuthError(
                "Tidal supports AccessToken or DeviceCode credentials".to_string(),
            )),
        }
    }

    fn is_authenticated(&self) -> bool {
        self.access_token.is_some()
    }

    fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<ServiceTrack>, ServiceError> {
        let token = self
            .access_token
            .as_ref()
            .ok_or_else(|| ServiceError::AuthError("Not authenticated".to_string()))?
            .clone();

        let rt = self.rt.clone();
        rt.block_on(async {
            let resp = self
                .client
                .get(format!("{}/search/tracks", API_BASE))
                .bearer_auth(&token)
                .query(&[
                    ("query", query),
                    ("limit", &limit.to_string()),
                    ("countryCode", &self.country_code),
                ])
                .send()
                .await
                .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let body = truncate_for_log(&body, 512);
                return Err(ServiceError::NetworkError(format!(
                    "Search failed: HTTP {} ({})",
                    status, body
                )));
            }

            let result: TidalSearchResult<TidalTrack> = read_bounded_json(resp).await?;

            Ok(result
                .items
                .into_iter()
                .map(|t| ServiceTrack {
                    id: t.id.to_string(),
                    title: t.title,
                    artist: t.artist.name,
                    album: t.album.title,
                    duration_secs: t.duration as f64,
                    track_number: Some(t.track_number),
                    album_art_url: t.album.cover.as_deref().and_then(tidal_cover_url),
                    available_qualities: vec![AudioQuality::High, AudioQuality::Lossless],
                })
                .collect())
        })
    }

    fn search_albums(&self, query: &str, limit: u32) -> Result<Vec<ServiceAlbum>, ServiceError> {
        let token = self
            .access_token
            .as_ref()
            .ok_or_else(|| ServiceError::AuthError("Not authenticated".to_string()))?
            .clone();

        let rt = self.rt.clone();
        rt.block_on(async {
            let resp = self
                .client
                .get(format!("{}/search/albums", API_BASE))
                .bearer_auth(&token)
                .query(&[
                    ("query", query),
                    ("limit", &limit.to_string()),
                    ("countryCode", &self.country_code),
                ])
                .send()
                .await
                .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let body = truncate_for_log(&body, 512);
                return Err(ServiceError::NetworkError(format!(
                    "Album search failed: HTTP {} ({})",
                    status, body
                )));
            }

            let result: TidalSearchResult<TidalAlbum> = read_bounded_json(resp).await?;

            Ok(result
                .items
                .into_iter()
                .map(|a| ServiceAlbum {
                    id: a.id.to_string(),
                    title: a.title,
                    artist: a.artist.name,
                    year: a.release_date.as_deref().and_then(parse_release_year),
                    track_count: a.number_of_tracks,
                    album_art_url: a.cover.as_deref().and_then(tidal_cover_url),
                })
                .collect())
        })
    }

    fn album_tracks(&self, album_id: &str) -> Result<Vec<ServiceTrack>, ServiceError> {
        let rt = self.rt.clone();
        rt.block_on(async {
            let resp = self
                .api_get_async(&format!("/albums/{}/tracks", album_id))
                .await?;

            if !resp.status().is_success() {
                return Err(ServiceError::NotFound(format!(
                    "Album {} not found",
                    album_id
                )));
            }

            let result: TidalSearchResult<TidalTrack> = read_bounded_json(resp).await?;

            Ok(result
                .items
                .into_iter()
                .map(|t| ServiceTrack {
                    id: t.id.to_string(),
                    title: t.title,
                    artist: t.artist.name,
                    album: t.album.title,
                    duration_secs: t.duration as f64,
                    track_number: Some(t.track_number),
                    album_art_url: None,
                    available_qualities: vec![AudioQuality::High, AudioQuality::Lossless],
                })
                .collect())
        })
    }

    fn start_stream(
        &mut self,
        track_id: &str,
        quality: AudioQuality,
    ) -> Result<ServiceStreamResult, ServiceError> {
        self.quality = quality;
        let quality_str = self.quality_to_tidal_quality().to_string();

        let token = self
            .access_token
            .as_ref()
            .ok_or_else(|| ServiceError::AuthError("Not authenticated".to_string()))?
            .clone();

        let rt = self.rt.clone();
        let client = self.client.clone();
        let country_code = self.country_code.clone();
        let track_id_owned = track_id.to_string();
        rt.block_on(async move {
            let resp = client
                .get(format!(
                    "{}/tracks/{}/urlpostpaywall",
                    API_BASE, track_id_owned
                ))
                .bearer_auth(&token)
                .query(&[
                    ("audioquality", quality_str.as_str()),
                    ("urlusagemode", "STREAM"),
                    ("assetpresentation", "FULL"),
                    ("countryCode", country_code.as_str()),
                ])
                .send()
                .await
                .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let body = truncate_for_log(&body, 512);
                return Err(ServiceError::NotFound(format!(
                    "Track {} not available (HTTP {}): {}",
                    track_id_owned, status, body
                )));
            }

            let stream_info: TidalStreamInfo = read_bounded_json(resp).await?;

            // Tidal provides a direct URL — let the engine's HTTP decoder handle it.
            let format_hint = match stream_info.codec.as_deref() {
                Some("FLAC") => Some("flac".to_string()),
                Some("AAC") => Some("aac".to_string()),
                Some("MQA") => Some("flac".to_string()), // MQA is FLAC-encapsulated
                _ => None,
            };

            log::info!(
                "[Tidal] Streaming track {} at {} quality, codec: {:?}",
                track_id_owned,
                quality_str,
                stream_info.codec,
            );

            Ok(ServiceStreamResult::Url {
                url: stream_info.url,
                format_hint,
            })
        })
    }

    fn stop_stream(&mut self) {
        // Tidal streams are HTTP URLs — nothing to clean up.
    }

    fn service_name(&self) -> &str {
        "Tidal"
    }
}

/// Parse the leading four ASCII digits of an ISO-8601 release date into a
/// year. Returns `None` for malformed input (too short, non-ASCII, or not
/// numeric) instead of panicking — the previous `d[..4]` slice would crash
/// on any input shorter than 4 bytes or on a non-UTF-8-boundary 4th byte.
fn parse_release_year(date: &str) -> Option<u32> {
    let prefix = date.get(..4)?;
    if !prefix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    prefix.parse().ok()
}

/// Build a Tidal CDN cover-art URL from a cover ID. The cover ID is expected
/// to be a UUID-like string of hex chars and dashes; the dashes are replaced
/// with `/` to form the CDN path segments. Invalid characters short-circuit
/// to `None` so a malicious payload cannot inject path traversal sequences
/// or query strings into the URL.
fn tidal_cover_url(cover: &str) -> Option<String> {
    if cover.is_empty() || !cover.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return None;
    }
    Some(format!(
        "https://resources.tidal.com/images/{}/640x640.jpg",
        cover.replace('-', "/")
    ))
}

/// Truncate a string for safe inclusion in log/error messages, replacing the
/// trailing portion with `…` when it would otherwise exceed `max` bytes.
fn truncate_for_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

// ============================================================================
// Tidal API response types
// ============================================================================

#[derive(Deserialize)]
struct TidalSession {
    #[serde(rename = "userId")]
    user_id: u64,
    #[serde(rename = "countryCode")]
    country_code: String,
}

#[derive(Deserialize)]
struct TidalDeviceAuth {
    #[serde(rename = "userCode")]
    user_code: String,
    #[serde(rename = "verificationUri")]
    verification_uri: String,
    #[serde(rename = "verificationUriComplete")]
    verification_uri_complete: Option<String>,
}

#[derive(Deserialize)]
struct TidalSearchResult<T> {
    items: Vec<T>,
}

#[derive(Deserialize)]
struct TidalTrack {
    id: u64,
    title: String,
    duration: u32,
    #[serde(rename = "trackNumber")]
    track_number: u32,
    artist: TidalArtist,
    album: TidalAlbumRef,
}

#[derive(Deserialize)]
struct TidalAlbum {
    id: u64,
    title: String,
    artist: TidalArtist,
    cover: Option<String>,
    #[serde(rename = "numberOfTracks")]
    number_of_tracks: u32,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
}

#[derive(Deserialize)]
struct TidalAlbumRef {
    title: String,
    cover: Option<String>,
}

#[derive(Deserialize)]
struct TidalArtist {
    name: String,
}

#[derive(Deserialize)]
struct TidalStreamInfo {
    url: String,
    codec: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tidal_service_not_authenticated() {
        let service = TidalService::new();
        assert!(!service.is_authenticated());
    }

    #[test]
    fn test_tidal_quality_mapping() {
        let mut service = TidalService::new();
        service.quality = AudioQuality::Lossless;
        assert_eq!(service.quality_to_tidal_quality(), "LOSSLESS");

        service.quality = AudioQuality::High;
        assert_eq!(service.quality_to_tidal_quality(), "HIGH");

        service.quality = AudioQuality::HiRes;
        assert_eq!(service.quality_to_tidal_quality(), "HI_RES_LOSSLESS");
    }

    #[test]
    fn test_tidal_device_code_requires_client_id() {
        let mut service = TidalService::new();
        let result = service.authenticate(ServiceCredentials::DeviceCode);
        assert!(result.is_err());
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("client_id"));
            }
            _ => panic!("Expected AuthError"),
        }
    }

    #[test]
    fn test_parse_release_year_well_formed() {
        assert_eq!(parse_release_year("1991-09-24"), Some(1991));
        assert_eq!(parse_release_year("2026"), Some(2026));
        assert_eq!(parse_release_year("2026-05"), Some(2026));
    }

    #[test]
    fn test_parse_release_year_short_input_no_panic() {
        // Inputs shorter than 4 bytes must not panic — the original code
        // sliced [..4] and would have crashed here.
        assert_eq!(parse_release_year(""), None);
        assert_eq!(parse_release_year("1"), None);
        assert_eq!(parse_release_year("12"), None);
        assert_eq!(parse_release_year("199"), None);
    }

    #[test]
    fn test_parse_release_year_non_ascii_no_panic() {
        // A 4-byte input that is not on a UTF-8 boundary at byte 4 would have
        // panicked under the original `d[..4]` slice.
        assert_eq!(parse_release_year("éé"), None);
        // Non-numeric prefix.
        assert_eq!(parse_release_year("abcd-01-01"), None);
        // Mixed prefix.
        assert_eq!(parse_release_year("19a1-01-01"), None);
    }

    #[test]
    fn test_tidal_cover_url_valid() {
        let url = tidal_cover_url("ab12cd34-5678-90ef-1234-567890abcdef").unwrap();
        assert!(url.starts_with("https://resources.tidal.com/images/"));
        assert!(url.ends_with("/640x640.jpg"));
        // Dashes get turned into path separators.
        assert!(url.contains("/ab12cd34/5678/90ef/1234/567890abcdef/"));
    }

    #[test]
    fn test_tidal_cover_url_rejects_path_traversal() {
        // Inputs containing characters outside of hex+dash are rejected so a
        // hostile or unexpected `cover` value cannot form a `../` sequence.
        assert_eq!(tidal_cover_url("../../etc/passwd"), None);
        assert_eq!(tidal_cover_url("ab12/cd34"), None);
        assert_eq!(tidal_cover_url("ab12?evil=1"), None);
        assert_eq!(tidal_cover_url(""), None);
    }

    #[test]
    fn test_truncate_for_log() {
        assert_eq!(truncate_for_log("hello", 10), "hello");
        assert_eq!(truncate_for_log("hello world", 5), "hello…");
    }

    #[test]
    fn test_tidal_service_debug_redacts_tokens() {
        let mut service = TidalService::new();
        service.access_token = Some("secret-access-token-do-not-log".to_string());
        service.refresh_token = Some("refresh-secret-token".to_string());
        let dbg = format!("{:?}", service);
        assert!(!dbg.contains("secret-access-token-do-not-log"));
        assert!(!dbg.contains("refresh-secret-token"));
        // First 4 chars of each token are visible, the rest redacted.
        assert!(dbg.contains("secr"));
        assert!(dbg.contains("refr"));
        assert!(dbg.contains("***"));
    }
}
