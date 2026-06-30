use super::async_runtime::AsyncRuntime;
use super::consts::API_BASE;
use super::consts::AUTH_BASE;
use super::consts::DEFAULT_CLIENT_ID;
use super::consts::read_bounded_json;
use super::misc::parse_release_year;
use super::misc::tidal_cover_url;
use super::misc::truncate_for_log;
use super::types::TidalAlbum;
use super::types::TidalDeviceAuth;
use super::types::TidalSearchResult;
use super::types::TidalSession;
use super::types::TidalStreamInfo;
use super::types::TidalTokenResponse;
use super::types::TidalTrack;
use crate::service::{redact_secret, *};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub(super) struct PendingTidalDeviceAuth {
    pub(super) device_code: String,
    pub(super) verification_message: String,
    pub(super) expires_at: Instant,
}

pub struct TidalService {
    pub(super) client: reqwest::Client,
    pub(super) rt: Arc<AsyncRuntime>,
    pub(super) client_id: String,
    pub(super) api_base: String,
    pub(super) auth_base: String,
    pub(super) access_token: Option<String>,
    #[allow(dead_code)]
    pub(super) refresh_token: Option<String>,
    pub(super) country_code: String,
    pub(super) quality: AudioQuality,
    pub(super) pending_device_auth: Option<PendingTidalDeviceAuth>,
}

impl std::fmt::Debug for TidalService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TidalService")
            .field("client_id", &redact_secret(&self.client_id))
            .field("api_base", &self.api_base)
            .field("auth_base", &self.auth_base)
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
            .field("pending_device_auth", &self.pending_device_auth.is_some())
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
            client_id: std::env::var("TIDAL_CLIENT_ID")
                .ok()
                .filter(|id| !id.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string()),
            api_base: API_BASE.to_string(),
            auth_base: AUTH_BASE.to_string(),
            access_token: None,
            refresh_token: None,
            country_code: "US".to_string(),
            quality: AudioQuality::Lossless,
            pending_device_auth: None,
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

    pub fn with_api_base(mut self, api_base: &str) -> Self {
        self.api_base = api_base.trim_end_matches('/').to_string();
        self
    }

    pub fn with_auth_base(mut self, auth_base: &str) -> Self {
        self.auth_base = auth_base.trim_end_matches('/').to_string();
        self
    }

    /// Issue a GET to `path` (relative to `API_BASE`) with bearer auth and the
    /// configured country code, returning the response asynchronously.
    pub(super) async fn api_get_async(
        &self,
        path: &str,
    ) -> Result<reqwest::Response, ServiceError> {
        let token = self
            .access_token
            .as_ref()
            .ok_or_else(|| ServiceError::AuthError("Not authenticated".to_string()))?;

        let url = format!("{}{}", self.api_base, path);
        self.client
            .get(&url)
            .bearer_auth(token)
            .query(&[("countryCode", &self.country_code)])
            .send()
            .await
            .map_err(|e| ServiceError::NetworkError(e.to_string()))
    }

    pub(super) fn quality_to_tidal_quality(&self) -> &str {
        match self.quality {
            AudioQuality::Low => "LOW",
            AudioQuality::Normal => "LOW",
            AudioQuality::High => "HIGH",
            AudioQuality::Lossless => "LOSSLESS",
            AudioQuality::HiRes => "HI_RES_LOSSLESS",
        }
    }
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
                let auth_base = self.auth_base.clone();

                if let Some(pending) = self.pending_device_auth.clone() {
                    if Instant::now() >= pending.expires_at {
                        self.pending_device_auth = None;
                        return Err(ServiceError::AuthError(
                            "Tidal device authorization expired; request a new device code"
                                .to_string(),
                        ));
                    }

                    let token_result: Result<TidalTokenResponse, ServiceError> =
                        rt.block_on(async move {
                            let resp = client
                                .post(format!("{}/token", auth_base))
                                .form(&[
                                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                                    ("device_code", pending.device_code.as_str()),
                                    ("client_id", client_id.as_str()),
                                ])
                                .send()
                                .await
                                .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

                            if resp.status().is_success() {
                                return read_bounded_json(resp).await;
                            }

                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            let body = truncate_for_log(&body, 512);
                            Err(ServiceError::AuthError(format!(
                                "{}. Waiting for Tidal device authorization: HTTP {} ({})",
                                pending.verification_message, status, body
                            )))
                        });

                    match token_result {
                        Ok(tokens) => {
                            self.access_token = Some(tokens.access_token);
                            self.refresh_token = tokens.refresh_token;
                            self.pending_device_auth = None;
                            Ok(())
                        }
                        Err(err) => Err(err),
                    }
                } else {
                    let auth_base = self.auth_base.clone();
                    let device_auth = rt.block_on(async move {
                        let resp = client
                            .post(format!("{}/device_authorization", auth_base))
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
                        Ok(device_auth)
                    })?;

                    let verification_url = device_auth
                        .verification_uri_complete
                        .clone()
                        .unwrap_or_else(|| device_auth.verification_uri.clone());
                    let verification_message = format!(
                        "Visit {} and enter code: {}",
                        verification_url, device_auth.user_code
                    );
                    self.pending_device_auth = Some(PendingTidalDeviceAuth {
                        device_code: device_auth.device_code,
                        verification_message: verification_message.clone(),
                        expires_at: Instant::now()
                            + Duration::from_secs(device_auth.expires_in.max(60)),
                    });
                    Err(ServiceError::AuthError(verification_message))
                }
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
        let api_base = self.api_base.clone();
        rt.block_on(async {
            let resp = self
                .client
                .get(format!("{}/search/tracks", api_base))
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
        let api_base = self.api_base.clone();
        rt.block_on(async {
            let resp = self
                .client
                .get(format!("{}/search/albums", api_base))
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
        let api_base = self.api_base.clone();
        let track_id_owned = track_id.to_string();
        rt.block_on(async move {
            let resp = client
                .get(format!(
                    "{}/tracks/{}/urlpostpaywall",
                    api_base, track_id_owned
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
