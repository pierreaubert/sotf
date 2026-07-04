//! Client-side business logic for the native SOTF LAN control API.

use crate::lan_discovery::DiscoveredSotfApiServer;
use serde::{Serialize, de::DeserializeOwned};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex};

mod decode;
mod error;
mod misc;
mod sotf_api_client_error;
#[cfg(test)]
mod tests;
mod types;

pub use error::*;
pub use types::*;

use decode::decode_bytes_response;
use decode::decode_response;
use misc::endpoint_url;
use sotf_api_client_error::normalize_base_url;
use sotf_api_client_error::validate_api_path_segment;
use types::AddAlbumRequest;
use types::CompletePairingRequest;
use types::IndexRequest;
use types::SeekRequest;
use types::VolumeRequest;
use types::parse_sse_frame;

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

impl SotfApiClient {
    pub fn new(base_url: impl Into<String>, auth_token: impl Into<String>) -> SotfApiResult<Self> {
        let base_url = normalize_base_url(base_url.into())?;
        Self::new_normalized(base_url, auth_token.into(), None, None)
    }

    pub(crate) fn new_with_pinned_fingerprint(
        base_url: impl Into<String>,
        auth_token: impl Into<String>,
        accepted_fingerprint: Option<&str>,
    ) -> SotfApiResult<Self> {
        let base_url = normalize_base_url(base_url.into())?;
        Self::new_normalized(base_url, auth_token.into(), None, accepted_fingerprint)
    }

    #[cfg(test)]
    pub(crate) fn new_with_tofu_dir(
        base_url: impl Into<String>,
        auth_token: impl Into<String>,
        tofu_dir: &Path,
    ) -> SotfApiResult<Self> {
        let base_url = normalize_base_url(base_url.into())?;
        Self::new_normalized(base_url, auth_token.into(), Some(tofu_dir), None)
    }

    fn new_normalized(
        base_url: String,
        auth_token: String,
        tofu_dir: Option<&Path>,
        accepted_fingerprint: Option<&str>,
    ) -> SotfApiResult<Self> {
        if auth_token.trim().is_empty() {
            return Err(SotfApiClientError::InvalidConfig(
                "auth token must not be empty".to_string(),
            ));
        }
        let tls = sotf_api_tls_options(&base_url, tofu_dir, accepted_fingerprint)?;
        let client = build_reqwest_client(&tls, Some(std::time::Duration::from_secs(8)))?;
        let event_client = build_reqwest_client(&tls, None)?;
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

    pub async fn library_albums_page(
        &self,
        offset: usize,
        limit: usize,
        query: Option<&str>,
        sort: Option<&str>,
    ) -> SotfApiResult<SotfApiAlbumList> {
        let offset = offset.to_string();
        let limit = limit.to_string();
        let mut query_serializer = url::form_urlencoded::Serializer::new(String::new());
        query_serializer.append_pair("offset", &offset);
        query_serializer.append_pair("limit", &limit);
        if let Some(query) = query.filter(|query| !query.trim().is_empty()) {
            query_serializer.append_pair("q", query.trim());
        }
        if let Some(sort) = sort.filter(|sort| !sort.trim().is_empty()) {
            query_serializer.append_pair("sort", sort.trim());
        }
        let query = query_serializer.finish();
        self.get_auth(&format!("library/albums?{query}")).await
    }

    pub async fn album_tracks(&self, album_id: &str) -> SotfApiResult<SotfApiAlbumTracks> {
        let album_id = validate_api_path_segment(album_id)?;
        self.get_auth(&format!("library/albums/{album_id}/tracks"))
            .await
    }

    pub async fn album_artwork(&self, album_id: &str) -> SotfApiResult<Vec<u8>> {
        let album_id = validate_api_path_segment(album_id)?;
        self.get_auth_bytes(&format!("library/albums/{album_id}/artwork"))
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

    pub fn authenticated_media_url(&self, track_id: &str) -> SotfApiResult<String> {
        let base_url = self.media_url(track_id)?;
        let mut query_serializer = url::form_urlencoded::Serializer::new(String::new());
        query_serializer.append_pair("token", &self.auth_token);
        Ok(format!("{base_url}?{}", query_serializer.finish()))
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

    async fn get_auth_bytes(&self, path: &str) -> SotfApiResult<Vec<u8>> {
        let response = self
            .client
            .get(self.endpoint_url(path))
            .bearer_auth(&self.auth_token)
            .send()
            .await?;
        decode_bytes_response(response).await
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

struct SotfApiTlsOptions {
    store: Arc<Mutex<sotf_tls::TofuStore>>,
    port: u16,
}

fn sotf_api_tls_options(
    base_url: &str,
    tofu_dir: Option<&Path>,
    accepted_fingerprint: Option<&str>,
) -> SotfApiResult<Option<SotfApiTlsOptions>> {
    let url = url::Url::parse(base_url).map_err(|err| {
        SotfApiClientError::InvalidConfig(format!("invalid SOTF API base URL: {err}"))
    })?;
    if url.scheme() != "https" {
        return Ok(None);
    }

    let port = url.port_or_known_default().ok_or_else(|| {
        SotfApiClientError::InvalidConfig("HTTPS SOTF API URL has no port".to_string())
    })?;
    let config_dir = match tofu_dir {
        Some(path) => path.to_path_buf(),
        None => crate::config::get_app_config_dir().ok_or_else(|| {
            SotfApiClientError::InvalidConfig(
                "could not determine config directory for SOTF API TLS pins".to_string(),
            )
        })?,
    };
    let store =
        sotf_tls::TofuStore::load(&config_dir).map_err(SotfApiClientError::InvalidConfig)?;
    let store = Arc::new(Mutex::new(store));

    if let Some(fingerprint) = accepted_fingerprint
        .map(str::trim)
        .filter(|fingerprint| !fingerprint.is_empty())
    {
        let host_key = canonical_url_host_port(&url, port)?;
        let fingerprint = canonical_certificate_fingerprint(fingerprint);
        store
            .lock()
            .map_err(|err| {
                SotfApiClientError::InvalidConfig(format!("TOFU store lock poisoned: {err}"))
            })?
            .accept(&host_key, &fingerprint, &host_key)
            .map_err(SotfApiClientError::InvalidConfig)?;
    }

    Ok(Some(SotfApiTlsOptions { store, port }))
}

fn build_reqwest_client(
    tls: &Option<SotfApiTlsOptions>,
    timeout: Option<std::time::Duration>,
) -> SotfApiResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder();
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    if let Some(tls) = tls {
        let config =
            sotf_tls::build_auto_accept_client_tls_config_for_port(tls.store.clone(), tls.port)
                .map_err(SotfApiClientError::InvalidConfig)?;
        builder = builder.use_preconfigured_tls(config);
    }
    builder.build().map_err(SotfApiClientError::from)
}

fn canonical_url_host_port(url: &url::Url, port: u16) -> SotfApiResult<String> {
    match url.host() {
        Some(url::Host::Domain(host)) => Ok(format!("{}:{port}", host.to_ascii_lowercase())),
        Some(url::Host::Ipv4(addr)) => Ok(format!("{addr}:{port}")),
        Some(url::Host::Ipv6(addr)) => Ok(format!("[{addr}]:{port}")),
        None => Err(SotfApiClientError::InvalidConfig(
            "SOTF API URL has no host".to_string(),
        )),
    }
}

fn canonical_certificate_fingerprint(fingerprint: &str) -> String {
    let hex = fingerprint
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .map(|ch| ch.to_ascii_uppercase())
        .collect::<String>();
    if hex.len() >= 2 && hex.len() % 2 == 0 {
        hex.as_bytes()
            .chunks(2)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(":")
    } else {
        fingerprint.trim().to_string()
    }
}
