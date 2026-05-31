//! Client-side business logic for the native SOTF LAN control API.

use std::fmt;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::lan_discovery::DiscoveredSotfApiServer;
use crate::sotf_server_event::SotfServerEvent;

#[derive(Clone)]
pub struct SotfApiClient {
    client: reqwest::Client,
    event_client: reqwest::Client,
    base_url: String,
    auth_token: String,
}

impl fmt::Debug for SotfApiClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SotfApiClient")
            .field("base_url", &self.base_url)
            .field("auth_token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SotfApiClientError {
    #[error("invalid SOTF API client configuration: {0}")]
    InvalidConfig(String),
    #[error("SOTF API request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("SOTF API returned HTTP {status}: {message}")]
    Api { status: u16, message: String },
    #[error("SOTF API returned invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub type SotfApiResult<T> = Result<T, SotfApiClientError>;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiHealth {
    pub ok: bool,
    pub service: String,
    pub version: String,
    pub auth_required: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiDiscoveryInfo {
    pub service: String,
    pub version: String,
    pub friendly_name: String,
    pub api_version: u32,
    pub base_path: String,
    pub auth: String,
    pub auth_required: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiCapabilities {
    pub api_version: u32,
    pub features: SotfApiFeatureFlags,
    pub endpoints: SotfApiEndpoints,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiFeatureFlags {
    pub playback_control: bool,
    pub queue_editing: bool,
    pub library_browse: bool,
    pub library_search: bool,
    pub media_range: bool,
    pub events: bool,
    pub outputs: bool,
    pub plugin_presets: bool,
    pub room_eq: bool,
    pub headphone_eq: bool,
    pub pairing: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiEndpoints {
    pub health: String,
    pub discovery: String,
    pub capabilities: String,
    pub state: String,
    pub events: String,
    pub queue: String,
    pub library_albums: String,
    pub media: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiPairingStatus {
    pub pairing_enabled: bool,
    pub nonce: Option<String>,
    pub server_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiTrustedClient {
    pub fingerprint: String,
    pub name: String,
    pub paired_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiTrustedClientList {
    pub clients: Vec<SotfApiTrustedClient>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiPairingModeResponse {
    pub ok: bool,
    pub pairing_enabled: bool,
    #[serde(default)]
    pub nonce: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiOkResponse {
    pub ok: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiState {
    pub playback: SotfApiPlayback,
    pub current_song: Option<SotfApiSong>,
    pub library: SotfApiLibrarySummary,
}

#[allow(
    clippy::large_enum_variant,
    reason = "stream events mirror API payloads directly; boxing would churn public callers"
)]
#[derive(Clone, Debug, PartialEq)]
pub enum SotfApiStreamEvent {
    State(SotfApiState),
    Server(SotfServerEvent),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiPlayback {
    pub state: String,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,
    pub current_index: Option<u32>,
    pub playlist_length: u32,
    pub playlist_version: u32,
    pub audio: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiLibrarySummary {
    pub albums: usize,
    pub tracks: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiQueue {
    pub items: Vec<SotfApiSong>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiAlbumList {
    pub albums: Vec<SotfApiAlbum>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiAlbum {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub track_count: usize,
    pub edition: Option<String>,
    pub dynamic_range: Option<f64>,
    pub is_favorite: bool,
    pub play_count: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiAlbumTracks {
    pub album: SotfApiAlbum,
    pub tracks: Vec<SotfApiTrack>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiTrack {
    pub id: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: String,
    pub track: Option<u32>,
    pub duration_secs: Option<u64>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub disc_number: Option<u32>,
    pub conductor: Option<String>,
    pub performer: Option<String>,
    pub ensemble: Option<String>,
    pub channels: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub is_favorite: bool,
    pub play_count: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiSong {
    pub file: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track: Option<u32>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub duration_secs: Option<f64>,
    pub pos: u32,
    pub id: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiCommandResponse {
    pub ok: bool,
    pub command: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SotfApiQueueEditResponse {
    pub ok: bool,
    pub command: String,
    pub index: Option<usize>,
    pub was_current: Option<bool>,
    pub playlist_version: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
struct SotfApiErrorResponse {
    error: Option<String>,
}

#[derive(Serialize)]
struct AddAlbumRequest {
    album_id: String,
    play_now: bool,
}

#[derive(Serialize)]
struct IndexRequest {
    index: usize,
}

#[derive(Serialize)]
struct SeekRequest {
    position_secs: f64,
}

#[derive(Serialize)]
struct VolumeRequest {
    volume: u8,
}

#[derive(Serialize)]
struct CompletePairingRequest {
    nonce: String,
    fingerprint: String,
    name: String,
}

impl SotfApiClient {
    pub fn new(base_url: impl Into<String>, auth_token: impl Into<String>) -> SotfApiResult<Self> {
        let base_url = normalize_base_url(base_url.into())?;
        let auth_token = auth_token.into();
        if auth_token.trim().is_empty() {
            return Err(SotfApiClientError::InvalidConfig(
                "auth token must not be empty".to_string(),
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()?;
        let event_client = reqwest::Client::builder().build()?;
        Ok(Self {
            client,
            event_client,
            base_url,
            auth_token: auth_token.trim().to_string(),
        })
    }

    pub fn from_discovered(
        server: &DiscoveredSotfApiServer,
        auth_token: impl Into<String>,
    ) -> SotfApiResult<Self> {
        Self::new(server.api_base_url.clone(), auth_token)
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    #[must_use]
    pub fn endpoint_url(&self, path: &str) -> String {
        endpoint_url(&self.base_url, path)
    }

    pub async fn health(&self) -> SotfApiResult<SotfApiHealth> {
        self.get_public("health").await
    }

    pub async fn discovery(&self) -> SotfApiResult<SotfApiDiscoveryInfo> {
        self.get_public("discovery").await
    }

    pub async fn capabilities(&self) -> SotfApiResult<SotfApiCapabilities> {
        self.get_public("capabilities").await
    }

    pub async fn state(&self) -> SotfApiResult<SotfApiState> {
        self.get_auth("state").await
    }

    pub async fn queue(&self) -> SotfApiResult<SotfApiQueue> {
        self.get_auth("queue").await
    }

    pub async fn library_albums(&self) -> SotfApiResult<SotfApiAlbumList> {
        self.get_auth("library/albums").await
    }

    pub async fn album_tracks(&self, album_id: &str) -> SotfApiResult<SotfApiAlbumTracks> {
        let album_id = validate_api_path_segment(album_id)?;
        self.get_auth(&format!("library/albums/{album_id}/tracks"))
            .await
    }

    pub async fn play(&self) -> SotfApiResult<SotfApiCommandResponse> {
        self.post_empty("play").await
    }

    pub async fn pause(&self) -> SotfApiResult<SotfApiCommandResponse> {
        self.post_empty("pause").await
    }

    pub async fn resume(&self) -> SotfApiResult<SotfApiCommandResponse> {
        self.post_empty("resume").await
    }

    pub async fn stop(&self) -> SotfApiResult<SotfApiCommandResponse> {
        self.post_empty("stop").await
    }

    pub async fn next(&self) -> SotfApiResult<SotfApiCommandResponse> {
        self.post_empty("next").await
    }

    pub async fn previous(&self) -> SotfApiResult<SotfApiCommandResponse> {
        self.post_empty("previous").await
    }

    pub async fn seek(&self, position_secs: f64) -> SotfApiResult<SotfApiCommandResponse> {
        if !position_secs.is_finite() || position_secs < 0.0 {
            return Err(SotfApiClientError::InvalidConfig(
                "position_secs must be a non-negative finite number".to_string(),
            ));
        }
        self.post_json("seek", &SeekRequest { position_secs }).await
    }

    pub async fn set_volume(&self, volume: u8) -> SotfApiResult<SotfApiCommandResponse> {
        if volume > 100 {
            return Err(SotfApiClientError::InvalidConfig(
                "volume must be between 0 and 100".to_string(),
            ));
        }
        self.post_json("volume", &VolumeRequest { volume }).await
    }

    pub async fn queue_add_album(
        &self,
        album_id: impl Into<String>,
        play_now: bool,
    ) -> SotfApiResult<SotfApiQueueEditResponse> {
        let album_id = album_id.into();
        validate_api_path_segment(&album_id)?;
        self.post_json("queue/add-album", &AddAlbumRequest { album_id, play_now })
            .await
    }

    pub async fn queue_clear(&self) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.post_empty("queue/clear").await
    }

    pub async fn queue_delete(&self, index: usize) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.post_json("queue/delete", &IndexRequest { index })
            .await
    }

    pub async fn queue_jump(&self, index: usize) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.post_json("queue/jump", &IndexRequest { index }).await
    }

    pub fn media_url(&self, track_id: &str) -> SotfApiResult<String> {
        let track_id = validate_api_path_segment(track_id)?;
        Ok(self.endpoint_url(&format!("media/{track_id}")))
    }

    #[must_use]
    pub fn events_url(&self) -> String {
        self.endpoint_url("events")
    }

    /// Query the server's pairing status.
    ///
    /// This is a public endpoint — no auth token required.
    pub async fn pairing_status(&self) -> SotfApiResult<SotfApiPairingStatus> {
        self.get_public("pairing/status").await
    }

    /// Enable pairing mode on the server and receive the one-time nonce.
    pub async fn enable_pairing(&self) -> SotfApiResult<SotfApiPairingModeResponse> {
        self.post_empty("pairing/enable").await
    }

    /// Disable pairing mode on the server.
    pub async fn disable_pairing(&self) -> SotfApiResult<SotfApiPairingModeResponse> {
        self.post_empty("pairing/disable").await
    }

    /// List trusted client certificates registered with the server.
    pub async fn trusted_clients(&self) -> SotfApiResult<SotfApiTrustedClientList> {
        self.get_auth("pairing/clients").await
    }

    /// Revoke a trusted client certificate fingerprint.
    pub async fn revoke_trusted_client(
        &self,
        fingerprint: &str,
    ) -> SotfApiResult<SotfApiOkResponse> {
        let response = self
            .client
            .delete(self.endpoint_url(&format!("pairing/clients/{fingerprint}")))
            .bearer_auth(&self.auth_token)
            .send()
            .await?;
        decode_response(response).await
    }

    /// Complete the pairing ceremony by submitting this client's fingerprint.
    ///
    /// This is a public endpoint — no auth token required.
    pub async fn complete_pairing(
        &self,
        nonce: &str,
        fingerprint: &str,
        name: &str,
    ) -> SotfApiResult<SotfApiCommandResponse> {
        self.post_public_json(
            "pairing/complete",
            &CompletePairingRequest {
                nonce: nonce.to_string(),
                fingerprint: fingerprint.to_string(),
                name: name.to_string(),
            },
        )
        .await
    }

    /// Open a long-lived SSE event stream from the server.
    ///
    /// Returns a receiver that yields parsed stream frames. The background task
    /// automatically reconnects with exponential backoff on disconnect.
    pub async fn events_stream(
        &self,
    ) -> SotfApiResult<tokio::sync::mpsc::Receiver<SotfApiResult<SotfApiStreamEvent>>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<SotfApiResult<SotfApiStreamEvent>>(64);
        let url = self.events_url();
        let token = self.auth_token.clone();
        let client = self.event_client.clone();

        tokio::spawn(async move {
            let mut backoff = std::time::Duration::from_secs(1);
            const MAX_BACKOFF: std::time::Duration = std::time::Duration::from_secs(30);

            loop {
                match client
                    .get(&url)
                    .bearer_auth(&token)
                    .header("Accept", "text/event-stream")
                    .send()
                    .await
                {
                    Ok(mut response) => {
                        if !response.status().is_success() {
                            let status = response.status();
                            let _ = tx
                                .send(Err(SotfApiClientError::Api {
                                    status: status.as_u16(),
                                    message: "event stream request failed".to_string(),
                                }))
                                .await;
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(MAX_BACKOFF);
                            continue;
                        }

                        backoff = std::time::Duration::from_secs(1);
                        let mut buf = String::new();

                        loop {
                            match response.chunk().await {
                                Ok(Some(bytes)) => {
                                    buf.push_str(&String::from_utf8_lossy(&bytes));
                                    while let Some(pos) = buf.find("\n\n") {
                                        let frame = buf[..pos].to_string();
                                        buf = buf[pos + 2..].to_string();
                                        if let Some(event) = parse_sse_frame(&frame) {
                                            if tx.send(Ok(event)).await.is_err() {
                                                return;
                                            }
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(err) => {
                                    let _ = tx.send(Err(SotfApiClientError::Request(err))).await;
                                    break;
                                }
                            }
                        }
                    }
                    Err(err) => {
                        let _ = tx.send(Err(SotfApiClientError::Request(err))).await;
                    }
                }

                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        });

        Ok(rx)
    }

    async fn get_public<T: DeserializeOwned>(&self, path: &str) -> SotfApiResult<T> {
        let response = self.client.get(self.endpoint_url(path)).send().await?;
        decode_response(response).await
    }

    async fn get_auth<T: DeserializeOwned>(&self, path: &str) -> SotfApiResult<T> {
        let response = self
            .client
            .get(self.endpoint_url(path))
            .bearer_auth(&self.auth_token)
            .send()
            .await?;
        decode_response(response).await
    }

    async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> SotfApiResult<T> {
        let response = self
            .client
            .post(self.endpoint_url(path))
            .bearer_auth(&self.auth_token)
            .send()
            .await?;
        decode_response(response).await
    }

    async fn post_json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> SotfApiResult<T> {
        let response = self
            .client
            .post(self.endpoint_url(path))
            .bearer_auth(&self.auth_token)
            .json(body)
            .send()
            .await?;
        decode_response(response).await
    }

    async fn post_public_json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> SotfApiResult<T> {
        let response = self
            .client
            .post(self.endpoint_url(path))
            .json(body)
            .send()
            .await?;
        decode_response(response).await
    }
}

pub fn normalized_api_base_url(base_url: impl Into<String>) -> SotfApiResult<String> {
    normalize_base_url(base_url.into())
}

/// Parse a single SSE frame (lines separated by `\n`, not `\n\n`).
/// Looks for `event:` and `data:` lines and returns the parsed frame.
fn parse_sse_frame(frame: &str) -> Option<SotfApiStreamEvent> {
    let mut event_type: Option<String> = None;
    let mut data: Option<String> = None;

    for line in frame.lines() {
        if let Some(val) = line.strip_prefix("event:") {
            event_type = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("data:") {
            data = Some(val.trim().to_string());
        }
    }

    let data = data?;
    match event_type.as_deref() {
        Some("ping") => None,
        Some("state") => serde_json::from_str(&data)
            .ok()
            .map(SotfApiStreamEvent::State),
        _ => serde_json::from_str(&data)
            .ok()
            .map(SotfApiStreamEvent::Server),
    }
}

async fn decode_response<T: DeserializeOwned>(response: reqwest::Response) -> SotfApiResult<T> {
    let status = response.status();
    let body = response.bytes().await?;
    if !status.is_success() {
        let message = serde_json::from_slice::<SotfApiErrorResponse>(&body)
            .ok()
            .and_then(|error| error.error)
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        return Err(SotfApiClientError::Api {
            status: status.as_u16(),
            message,
        });
    }
    Ok(serde_json::from_slice(&body)?)
}

fn normalize_base_url(base_url: String) -> SotfApiResult<String> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err(SotfApiClientError::InvalidConfig(
            "base URL must start with http:// or https://".to_string(),
        ));
    }
    if base_url.ends_with("/api/v1") {
        Ok(base_url)
    } else {
        Ok(format!("{base_url}/api/v1"))
    }
}

fn endpoint_url(base_url: &str, path: &str) -> String {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        base_url.to_string()
    } else {
        format!("{base_url}/{path}")
    }
}

fn validate_api_path_segment(segment: &str) -> SotfApiResult<&str> {
    if !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_'))
    {
        Ok(segment)
    } else {
        Err(SotfApiClientError::InvalidConfig(
            "API path segment contains unsupported characters".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::net::Ipv4Addr;

    #[test]
    fn client_normalizes_base_url() {
        let client = SotfApiClient::new("http://192.168.1.10:8732/", "secret").unwrap();
        assert_eq!(client.base_url(), "http://192.168.1.10:8732/api/v1");
        assert_eq!(
            client.endpoint_url("/state"),
            "http://192.168.1.10:8732/api/v1/state"
        );

        let client = SotfApiClient::new("http://host:8732/api/v1/", "secret").unwrap();
        assert_eq!(client.base_url(), "http://host:8732/api/v1");
    }

    #[test]
    fn client_rejects_invalid_config() {
        assert!(SotfApiClient::new("ftp://host", "secret").is_err());
        assert!(SotfApiClient::new("http://host:8732", " ").is_err());
        assert!(validate_api_path_segment("id:42").is_ok());
        assert!(validate_api_path_segment("hash:abc-123_def").is_ok());
        assert!(validate_api_path_segment("../music").is_err());
        assert!(validate_api_path_segment("id/42").is_err());
    }

    #[test]
    fn client_debug_redacts_token() {
        let client = SotfApiClient::new("http://host:8732", "very-secret-token").unwrap();
        let debug = format!("{client:?}");
        assert!(debug.contains("SotfApiClient"));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("very-secret-token"));
    }

    #[test]
    fn client_can_be_created_from_discovered_server() {
        let server = DiscoveredSotfApiServer {
            instance_name: "Kitchen._sotf._tcp.local".to_string(),
            friendly_name: "Kitchen".to_string(),
            host_name: "Kitchen.local".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 42),
            port: 8732,
            protocol: "http".to_string(),
            api_path: "/api/v1".to_string(),
            auth: "bearer".to_string(),
            origin_url: "http://192.168.1.42:8732".to_string(),
            api_base_url: "http://192.168.1.42:8732/api/v1".to_string(),
            txt_records: BTreeMap::new(),
        };
        let client = SotfApiClient::from_discovered(&server, "secret").unwrap();
        assert_eq!(client.base_url(), "http://192.168.1.42:8732/api/v1");
    }

    #[test]
    fn parses_state_response_shape() {
        let state: SotfApiState = serde_json::from_str(
            r#"{
              "playback": {
                "state": "play",
                "position_secs": 12.5,
                "duration_secs": 240.0,
                "volume": 42,
                "current_index": 0,
                "playlist_length": 1,
                "playlist_version": 7,
                "audio": "44100:16:2"
              },
              "current_song": {
                "file": "/music/a.flac",
                "title": "A",
                "artist": "Artist",
                "album": "Album",
                "track": 1,
                "date": null,
                "genre": null,
                "duration_secs": 240.0,
                "pos": 0,
                "id": 0
              },
              "library": { "albums": 10, "tracks": 100 }
            }"#,
        )
        .unwrap();
        assert_eq!(state.playback.state, "play");
        assert_eq!(state.playback.volume, 42);
        assert_eq!(state.current_song.unwrap().title.as_deref(), Some("A"));
        assert_eq!(state.library.albums, 10);
    }

    #[test]
    fn parses_capabilities_response_shape() {
        let capabilities: SotfApiCapabilities = serde_json::from_str(
            r#"{
              "api_version": 1,
              "features": {
                "playback_control": true,
                "queue_editing": true,
                "library_browse": true,
                "library_search": false,
                "media_range": true,
                "events": true,
                "outputs": false,
                "plugin_presets": false,
                "room_eq": false,
                "headphone_eq": false,
                "pairing": false
              },
              "endpoints": {
                "health": "/api/v1/health",
                "discovery": "/api/v1/discovery",
                "capabilities": "/api/v1/capabilities",
                "state": "/api/v1/state",
                "events": "/api/v1/events",
                "queue": "/api/v1/queue",
                "library_albums": "/api/v1/library/albums",
                "media": "/api/v1/media/{track_id}"
              }
            }"#,
        )
        .unwrap();
        assert!(capabilities.features.media_range);
        assert!(capabilities.features.events);
        assert!(!capabilities.features.pairing);
        assert_eq!(capabilities.endpoints.media, "/api/v1/media/{track_id}");
    }

    #[test]
    fn builds_media_and_event_urls() {
        let client = SotfApiClient::new("http://host:8732", "secret").unwrap();
        assert_eq!(
            client.media_url("track-abc_123").unwrap(),
            "http://host:8732/api/v1/media/track-abc_123"
        );
        assert_eq!(client.events_url(), "http://host:8732/api/v1/events");
        assert!(client.media_url("../secret").is_err());
    }

    #[test]
    fn parses_error_response_shape() {
        let error: SotfApiErrorResponse =
            serde_json::from_str(r#"{ "ok": false, "error": "missing token" }"#).unwrap();
        assert_eq!(error.error.as_deref(), Some("missing token"));
    }

    #[test]
    fn parses_library_album_response_shapes() {
        let albums: SotfApiAlbumList = serde_json::from_str(
            r#"{
              "albums": [
                {
                  "id": "id:42",
                  "title": "Quartets",
                  "artist": "Example Ensemble",
                  "year": 2024,
                  "track_count": 2,
                  "edition": "Blu-ray",
                  "dynamic_range": 14.5,
                  "is_favorite": true,
                  "play_count": 3
                }
              ]
            }"#,
        )
        .unwrap();
        assert_eq!(albums.albums[0].id, "id:42");
        assert_eq!(albums.albums[0].track_count, 2);

        let tracks: SotfApiAlbumTracks = serde_json::from_str(
            r#"{
              "album": {
                "id": "id:42",
                "title": "Quartets",
                "artist": "Example Ensemble",
                "year": 2024,
                "track_count": 1,
                "edition": null,
                "dynamic_range": null,
                "is_favorite": false,
                "play_count": 0
              },
              "tracks": [
                {
                  "id": "uuid:abc",
                  "title": "I. Allegro",
                  "artist": "Example Ensemble",
                  "album": "Quartets",
                  "track": 1,
                  "duration_secs": 615,
                  "genre": "Classical",
                  "composer": "Composer",
                  "disc_number": 1,
                  "conductor": null,
                  "performer": null,
                  "ensemble": "Example Ensemble",
                  "channels": 2,
                  "sample_rate": 96000,
                  "bit_depth": 24,
                  "is_favorite": false,
                  "play_count": 1
                }
              ]
            }"#,
        )
        .unwrap();
        assert_eq!(tracks.album.title, "Quartets");
        assert_eq!(tracks.tracks[0].duration_secs, Some(615));
        assert_eq!(tracks.tracks[0].sample_rate, Some(96000));
    }

    #[test]
    fn parses_queue_edit_response_shape() {
        let response: SotfApiQueueEditResponse = serde_json::from_str(
            r#"{
              "ok": true,
              "command": "queue.delete",
              "index": 1,
              "was_current": false,
              "playlist_version": 8
            }"#,
        )
        .unwrap();
        assert_eq!(response.index, Some(1));
        assert_eq!(response.was_current, Some(false));
        assert_eq!(response.playlist_version, Some(8));
    }

    #[test]
    fn parse_sse_frame_extracts_event_and_data() {
        let frame = "event: volume_changed\ndata: {\"event\":\"volume_changed\",\"volume\":42}";
        let event = parse_sse_frame(frame).expect("should parse");
        assert_eq!(
            event,
            SotfApiStreamEvent::Server(SotfServerEvent::VolumeChanged { volume: 42 })
        );
    }

    #[test]
    fn parse_sse_frame_surfaces_state_snapshot() {
        let frame = r#"event: state
data: {"playback":{"state":"play","position_secs":1.0,"duration_secs":2.0,"volume":42,"current_index":0,"playlist_length":1,"playlist_version":7,"audio":null},"current_song":null,"library":{"albums":10,"tracks":100}}"#;
        let event = parse_sse_frame(frame).expect("should parse");
        let SotfApiStreamEvent::State(state) = event else {
            panic!("expected state frame");
        };
        assert_eq!(state.playback.volume, 42);
        assert_eq!(state.library.tracks, 100);
    }

    #[test]
    fn parse_sse_frame_ignores_ping() {
        let frame = "event: ping\ndata: {}";
        assert!(parse_sse_frame(frame).is_none());
    }

    #[test]
    fn parse_sse_frame_returns_none_for_missing_data() {
        let frame = "event: playback_changed";
        assert!(parse_sse_frame(frame).is_none());
    }

    #[test]
    fn parses_pairing_status_response() {
        let status: SotfApiPairingStatus = serde_json::from_str(
            r#"{
                "pairing_enabled": true,
                "nonce": null,
                "server_fingerprint": "AA:BB:CC"
            }"#,
        )
        .unwrap();
        assert!(status.pairing_enabled);
        assert_eq!(status.nonce, None);
        assert_eq!(status.server_fingerprint, "AA:BB:CC");
    }

    #[test]
    fn parses_pairing_status_when_disabled() {
        let status: SotfApiPairingStatus = serde_json::from_str(
            r#"{
                "pairing_enabled": false,
                "nonce": null,
                "server_fingerprint": "DD:EE:FF"
            }"#,
        )
        .unwrap();
        assert!(!status.pairing_enabled);
        assert_eq!(status.nonce, None);
    }

    #[test]
    fn parses_trusted_client_list() {
        let list: SotfApiTrustedClientList = serde_json::from_str(
            r#"{
                "clients": [
                    {
                        "fingerprint": "AA:BB",
                        "name": "iPhone",
                        "paired_at": "2026-05-30"
                    }
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(list.clients.len(), 1);
        assert_eq!(list.clients[0].name, "iPhone");
    }
}
