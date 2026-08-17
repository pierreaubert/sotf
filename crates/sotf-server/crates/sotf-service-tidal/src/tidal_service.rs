use crate::async_runtime::AsyncRuntime;
use crate::consts::API_BASE;
use crate::consts::AUTH_BASE;
use crate::consts::DEFAULT_CLIENT_ID;
use crate::consts::read_bounded_json;
use crate::misc::parse_release_year;
use crate::misc::tidal_cover_url;
use crate::misc::truncate_for_log;
use crate::types::TidalAlbum;
use crate::types::TidalDeviceAuth;
use crate::types::TidalFavoritesResponse;
use crate::types::TidalSearchResult;
use crate::types::TidalSession;
use crate::types::TidalStreamInfo;
use crate::types::TidalTokenResponse;
use crate::types::TidalTrack;
use sotf_services::{redact_secret, *};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Page size for the paged `/users/{id}/favorites/*` endpoints.
const FAVORITES_PAGE_LIMIT: u32 = 50;

/// Upper bound on favorites pagination, so a pathological server that keeps
/// returning full pages without advancing `total` cannot loop forever
/// (same cap as `sotf-service-spotify`).
const MAX_PAGES: usize = 20;

#[derive(Debug, Clone)]
pub(super) struct PendingTidalDeviceAuth {
    pub(super) device_code: String,
    pub(super) verification_message: String,
    pub(super) expires_at: Instant,
}

/// Information a UI needs to display the device-code login prompt.
#[derive(Debug, Clone)]
pub struct DeviceAuthPrompt {
    /// URL the user should visit (verificationUriComplete when available).
    pub verification_url: String,
    /// Code the user must enter at `verification_url`.
    pub user_code: String,
    /// Seconds until the device code expires (clamped to at least 60).
    pub expires_in_secs: u64,
}

/// Outcome of polling the device-code token endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceAuthPoll {
    /// The user has not completed authorization yet; keep polling.
    Pending,
    /// Authorization completed; tokens are stored on the service.
    Complete,
    /// The device code expired; call `begin_device_auth` again.
    Expired,
}

/// Internal result of a device-code token poll, carrying the raw HTTP status
/// and truncated body so callers can build their own error messages.
pub(super) enum RawDevicePoll {
    Tokens(TidalTokenResponse),
    Pending {
        status: reqwest::StatusCode,
        body: String,
    },
    Failed {
        status: reqwest::StatusCode,
        body: String,
    },
}

pub struct TidalService {
    pub(super) client: reqwest::Client,
    pub(super) rt: Arc<AsyncRuntime>,
    pub(super) client_id: String,
    pub(super) api_base: String,
    pub(super) auth_base: String,
    pub(super) access_token: Option<String>,
    pub(super) refresh_token: Option<String>,
    pub(super) user_id: Option<u64>,
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
            .field("user_id", &self.user_id)
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
            user_id: None,
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

    /// Current access token, if any. Redact before logging.
    pub fn access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }

    /// Current refresh token, if any. Redact before logging.
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    /// Tidal user id captured during authentication, if known.
    pub fn user_id(&self) -> Option<u64> {
        self.user_id
    }

    /// Restore a persisted session without re-authenticating. The tokens are
    /// not validated here; the first API call will fail if they are stale
    /// (use `authenticate_refresh` to renew them first).
    pub fn set_tokens(&mut self, access_token: String, refresh_token: Option<String>) {
        self.access_token = Some(access_token);
        self.refresh_token = refresh_token;
    }

    /// Begin the OAuth2 device-code flow: request a device code and store it
    /// as pending. Returns the prompt a UI should display to the user.
    pub fn begin_device_auth(&mut self) -> Result<DeviceAuthPrompt, ServiceError> {
        if self.client_id.is_empty() {
            return Err(ServiceError::AuthError(
                "Tidal client_id not configured. Use with_client_id() or set TIDAL_CLIENT_ID env var.".to_string(),
            ));
        }

        let rt = self.rt.clone();
        let client = self.client.clone();
        let client_id = self.client_id.clone();
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
        let expires_in_secs = device_auth.expires_in.max(60);
        let verification_message = format!(
            "Visit {} and enter code: {}",
            verification_url, device_auth.user_code
        );
        self.pending_device_auth = Some(PendingTidalDeviceAuth {
            device_code: device_auth.device_code,
            verification_message,
            expires_at: Instant::now() + Duration::from_secs(expires_in_secs),
        });
        Ok(DeviceAuthPrompt {
            verification_url,
            user_code: device_auth.user_code,
            expires_in_secs,
        })
    }

    /// POST /token with the pending device code. Does not touch service
    /// state — callers decide what to do with the outcome.
    pub(super) fn poll_device_auth_raw(
        &self,
        pending: &PendingTidalDeviceAuth,
    ) -> Result<RawDevicePoll, ServiceError> {
        let rt = self.rt.clone();
        let client = self.client.clone();
        let client_id = self.client_id.clone();
        let auth_base = self.auth_base.clone();
        let device_code = pending.device_code.clone();
        rt.block_on(async move {
            let resp = client
                .post(format!("{}/token", auth_base))
                .form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("device_code", device_code.as_str()),
                    ("client_id", client_id.as_str()),
                ])
                .send()
                .await
                .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

            if resp.status().is_success() {
                let tokens: TidalTokenResponse = read_bounded_json(resp).await?;
                return Ok(RawDevicePoll::Tokens(tokens));
            }

            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            // Sniff the RFC 8628 error code on the full body — truncating
            // first could cut the code out of a long error JSON.
            // `authorization_pending` and `slow_down` both mean "keep
            // waiting" (`slow_down` additionally asks to poll less often;
            // the caller's poll interval is fixed, so Pending is enough).
            let is_pending = status == reqwest::StatusCode::BAD_REQUEST
                && (body.contains("authorization_pending") || body.contains("slow_down"));
            let body = truncate_for_log(&body, 512);
            if is_pending {
                Ok(RawDevicePoll::Pending { status, body })
            } else {
                Ok(RawDevicePoll::Failed { status, body })
            }
        })
    }

    /// Poll the device-code flow once. On success the access/refresh tokens
    /// are stored and the pending state is cleared.
    pub fn poll_device_auth(&mut self) -> Result<DeviceAuthPoll, ServiceError> {
        let pending = self.pending_device_auth.clone().ok_or_else(|| {
            ServiceError::AuthError(
                "No pending device authorization; call begin_device_auth() first".to_string(),
            )
        })?;

        if Instant::now() >= pending.expires_at {
            self.pending_device_auth = None;
            return Ok(DeviceAuthPoll::Expired);
        }

        match self.poll_device_auth_raw(&pending)? {
            RawDevicePoll::Tokens(tokens) => {
                log::info!("[Tidal] Device authorization completed");
                self.access_token = Some(tokens.access_token);
                self.refresh_token = tokens.refresh_token;
                self.pending_device_auth = None;
                Ok(DeviceAuthPoll::Complete)
            }
            RawDevicePoll::Pending { .. } => Ok(DeviceAuthPoll::Pending),
            RawDevicePoll::Failed { status, body } => {
                // Terminal failure (access_denied, expired_token, …): clear
                // the pending state so the caller restarts the flow instead
                // of polling until the local expiry.
                self.pending_device_auth = None;
                Err(ServiceError::AuthError(format!(
                    "Tidal device authorization failed: HTTP {} ({})",
                    status, body
                )))
            }
        }
    }

    /// Exchange the stored refresh token for a new access token. Tidal rotates
    /// refresh tokens; when the response carries a new one it replaces the
    /// stored token.
    pub fn authenticate_refresh(&mut self) -> Result<(), ServiceError> {
        let refresh_token = self.refresh_token.clone().ok_or_else(|| {
            ServiceError::AuthError("No refresh token available; authenticate first".to_string())
        })?;

        let rt = self.rt.clone();
        let client = self.client.clone();
        let client_id = self.client_id.clone();
        let auth_base = self.auth_base.clone();

        let tokens: TidalTokenResponse = rt.block_on(async move {
            let resp = client
                .post(format!("{}/token", auth_base))
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", refresh_token.as_str()),
                    ("client_id", client_id.as_str()),
                ])
                .send()
                .await
                .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let body = truncate_for_log(&body, 512);
                return Err(ServiceError::AuthError(format!(
                    "Token refresh failed: HTTP {} ({})",
                    status, body
                )));
            }

            read_bounded_json(resp).await
        })?;

        let rotated = tokens.refresh_token.is_some();
        self.access_token = Some(tokens.access_token);
        if let Some(new_refresh) = tokens.refresh_token {
            self.refresh_token = Some(new_refresh);
        }
        log::info!(
            "[Tidal] Refreshed access token (refresh token rotated: {})",
            rotated
        );
        Ok(())
    }

    /// Fetches one page of a paged favorites endpoint and appends the mapped
    /// items to `out`. Returns `(items_on_page, total_number_of_items)`.
    pub(super) fn favorites_page<T, O, F>(
        &self,
        path: &str,
        offset: u64,
        map: F,
        out: &mut Vec<O>,
    ) -> Result<(usize, u64), ServiceError>
    where
        T: serde::de::DeserializeOwned,
        F: Fn(T) -> O,
    {
        let token = self
            .access_token
            .as_ref()
            .ok_or_else(|| ServiceError::AuthError("Not authenticated".to_string()))?
            .clone();
        let rt = self.rt.clone();
        let client = self.client.clone();
        let country_code = self.country_code.clone();
        let api_base = self.api_base.clone();
        let path = path.to_string();
        let offset_str = offset.to_string();
        let limit_str = FAVORITES_PAGE_LIMIT.to_string();
        rt.block_on(async move {
            let resp = client
                .get(format!("{}{}", api_base, path))
                .bearer_auth(&token)
                .query(&[
                    ("limit", limit_str.as_str()),
                    ("offset", offset_str.as_str()),
                    ("countryCode", country_code.as_str()),
                ])
                .send()
                .await
                .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let body = truncate_for_log(&body, 512);
                return Err(ServiceError::NetworkError(format!(
                    "Favorites request failed: HTTP {} ({})",
                    status, body
                )));
            }

            let page: TidalFavoritesResponse<T> = read_bounded_json(resp).await?;
            let count = page.items.len();
            let total = page.total_number_of_items;
            out.extend(page.items.into_iter().map(|entry| map(entry.item)));
            Ok((count, total))
        })
    }

    /// All albums in the user's favorites, paged through the Tidal API.
    pub fn favorites_albums(&self) -> Result<Vec<ServiceAlbum>, ServiceError> {
        let user_id = self.user_id.ok_or_else(|| {
            ServiceError::AuthError(
                "Tidal user id unknown; authenticate with an access token first".to_string(),
            )
        })?;

        let mut albums = Vec::new();
        let mut offset = 0u64;
        let mut pages = 0usize;
        loop {
            let (count, total) = self.favorites_page(
                &format!("/users/{}/favorites/albums", user_id),
                offset,
                tidal_album_to_service,
                &mut albums,
            )?;
            pages += 1;
            offset += count as u64;
            if count == 0 || offset >= total {
                break;
            }
            if pages >= MAX_PAGES {
                log::warn!(
                    "[Tidal] favorites_albums hit the {MAX_PAGES}-page cap \
                     (offset {offset} of {total} items); returning partial results"
                );
                break;
            }
        }
        Ok(albums)
    }

    /// All tracks in the user's favorites, paged through the Tidal API.
    pub fn favorites_tracks(&self) -> Result<Vec<ServiceTrack>, ServiceError> {
        let user_id = self.user_id.ok_or_else(|| {
            ServiceError::AuthError(
                "Tidal user id unknown; authenticate with an access token first".to_string(),
            )
        })?;

        let mut tracks = Vec::new();
        let mut offset = 0u64;
        let mut pages = 0usize;
        loop {
            let (count, total) = self.favorites_page(
                &format!("/users/{}/favorites/tracks", user_id),
                offset,
                tidal_track_to_service,
                &mut tracks,
            )?;
            pages += 1;
            offset += count as u64;
            if count == 0 || offset >= total {
                break;
            }
            if pages >= MAX_PAGES {
                log::warn!(
                    "[Tidal] favorites_tracks hit the {MAX_PAGES}-page cap \
                     (offset {offset} of {total} items); returning partial results"
                );
                break;
            }
        }
        Ok(tracks)
    }
}

pub(super) fn tidal_track_to_service(t: TidalTrack) -> ServiceTrack {
    ServiceTrack {
        id: t.id.to_string(),
        title: t.title,
        artist: t.artist.name,
        album: t.album.title,
        duration_secs: t.duration as f64,
        track_number: Some(t.track_number),
        album_art_url: t.album.cover.as_deref().and_then(tidal_cover_url),
        available_qualities: vec![AudioQuality::High, AudioQuality::Lossless],
    }
}

pub(super) fn tidal_album_to_service(a: TidalAlbum) -> ServiceAlbum {
    ServiceAlbum {
        id: a.id.to_string(),
        title: a.title,
        artist: a.artist.name,
        year: a.release_date.as_deref().and_then(parse_release_year),
        track_count: a.number_of_tracks,
        album_art_url: a.cover.as_deref().and_then(tidal_cover_url),
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
                        self.user_id = Some(session.user_id);
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
                    self.user_id = None;
                }
                result
            }
            ServiceCredentials::DeviceCode => {
                if self.pending_device_auth.is_none() {
                    // First call: request a device code and surface the
                    // verification prompt as an AuthError (the caller displays
                    // it and retries once the user completes authorization).
                    let prompt = self.begin_device_auth()?;
                    return Err(ServiceError::AuthError(format!(
                        "Visit {} and enter code: {}",
                        prompt.verification_url, prompt.user_code
                    )));
                }

                let pending = self
                    .pending_device_auth
                    .clone()
                    .expect("checked is_none above");
                if Instant::now() >= pending.expires_at {
                    self.pending_device_auth = None;
                    return Err(ServiceError::AuthError(
                        "Tidal device authorization expired; request a new device code".to_string(),
                    ));
                }

                match self.poll_device_auth_raw(&pending)? {
                    RawDevicePoll::Tokens(tokens) => {
                        self.access_token = Some(tokens.access_token);
                        self.refresh_token = tokens.refresh_token;
                        self.pending_device_auth = None;
                        Ok(())
                    }
                    RawDevicePoll::Pending { status, body } => {
                        Err(ServiceError::AuthError(format!(
                            "{}. Waiting for Tidal device authorization: HTTP {} ({})",
                            pending.verification_message, status, body
                        )))
                    }
                    RawDevicePoll::Failed { status, body } => {
                        // Terminal failure (access_denied, expired_token, …):
                        // clear the pending state and surface the error now
                        // instead of waiting for the local expiry.
                        self.pending_device_auth = None;
                        Err(ServiceError::AuthError(format!(
                            "Tidal device authorization failed: HTTP {} ({})",
                            status, body
                        )))
                    }
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
                .map(tidal_track_to_service)
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
                .map(tidal_album_to_service)
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
                if status == reqwest::StatusCode::UNAUTHORIZED
                    || status == reqwest::StatusCode::FORBIDDEN
                {
                    // Stale or rejected token — surface as AuthError so the
                    // caller can refresh the token and retry.
                    return Err(ServiceError::AuthError(format!(
                        "Tidal stream access denied for track {} (HTTP {}): {}",
                        track_id_owned, status, body
                    )));
                }
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
