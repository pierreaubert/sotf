//! Client-side business logic for the native SOTF LAN control API.

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::lan_discovery::DiscoveredSotfApiServer;

#[derive(Clone, Debug)]
pub struct SotfApiClient {
    client: reqwest::Client,
    base_url: String,
    auth_token: String,
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
pub struct SotfApiState {
    pub playback: SotfApiPlayback,
    pub current_song: Option<SotfApiSong>,
    pub library: SotfApiLibrarySummary,
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

#[derive(Clone, Debug, Deserialize)]
struct SotfApiErrorResponse {
    error: Option<String>,
}

#[derive(Serialize)]
struct SeekRequest {
    position_secs: f64,
}

#[derive(Serialize)]
struct VolumeRequest {
    volume: u8,
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
        Ok(Self {
            client,
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

    pub async fn state(&self) -> SotfApiResult<SotfApiState> {
        self.get_auth("state").await
    }

    pub async fn queue(&self) -> SotfApiResult<SotfApiQueue> {
        self.get_auth("queue").await
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
    fn parses_error_response_shape() {
        let error: SotfApiErrorResponse =
            serde_json::from_str(r#"{ "ok": false, "error": "missing token" }"#).unwrap();
        assert_eq!(error.error.as_deref(), Some("missing token"));
    }
}
