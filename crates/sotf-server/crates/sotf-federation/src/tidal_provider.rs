//! Tidal federation provider — exposes the user's Tidal favorites (albums)
//! as federation albums. Tracks carry `AudioSource::ServiceStream` sources;
//! the engine's service-stream resolver mints a fresh stream URL at decode
//! time via the installed `TidalService`.

use crate::provider::{
    LibraryEvent, LibraryProvider, ProviderAlbum, ProviderCapabilities, ProviderError,
    ProviderFuture, SourceId, SourceType,
};
use crate::service_common::{fetch_image, map_service_error, service_track_to_provider};
use sotf_audio::decoder::{AudioSource, ServiceId};
use sotf_service_tidal::TidalService;
use sotf_services::{AudioQuality, ServiceCredentials, StreamingService, redact_secret};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Connection configuration for a Tidal federation source.
#[derive(Clone)]
pub struct TidalProviderConfig {
    /// OAuth2 access token.
    pub access_token: String,
    /// OAuth2 refresh token (Tidal rotates these; the service keeps the new one).
    pub refresh_token: String,
    /// Tidal application client ID (empty = built-in default / TIDAL_CLIENT_ID env).
    pub client_id: String,
    /// ISO country code used as fallback until the session reports its own.
    pub country_code: String,
    /// Quality label ("LOW", "HIGH", "LOSSLESS", "HI_RES_LOSSLESS", ...).
    pub quality: String,
}

impl std::fmt::Debug for TidalProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TidalProviderConfig")
            .field("access_token", &redact_secret(&self.access_token))
            .field("refresh_token", &redact_secret(&self.refresh_token))
            .field("client_id", &redact_secret(&self.client_id))
            .field("country_code", &self.country_code)
            .field("quality", &self.quality)
            .finish()
    }
}

impl TidalProviderConfig {
    fn audio_quality(&self) -> AudioQuality {
        match self.quality.to_ascii_uppercase().as_str() {
            "LOW" => AudioQuality::Low,
            "NORMAL" => AudioQuality::Normal,
            "HIGH" | "VERYHIGH" | "VERY_HIGH" => AudioQuality::High,
            "HI_RES" | "HI_RES_LOSSLESS" | "HIRES" => AudioQuality::HiRes,
            // "LOSSLESS" and anything unrecognized fall back to lossless.
            _ => AudioQuality::Lossless,
        }
    }
}

/// Reports rotated Tidal tokens after a successful refresh-token exchange:
/// the new access token and the new refresh token (when the server returned
/// one). Tidal refresh tokens are single-use, so callers must persist the
/// rotated pair back to wherever the configuration came from — otherwise the
/// stored refresh token is burned and the user is silently logged out.
///
/// Invoked on the blocking thread pool during connect; the callback returns
/// no `Result`, so persistence failures must be handled (logged) inside it.
pub type TidalTokenPersister = Arc<dyn Fn(&str, Option<&str>) + Send + Sync>;

/// A Tidal federation provider backed by `TidalService`.
pub struct TidalProvider {
    source_id: SourceId,
    service: Arc<TidalService>,
    /// Album external id → cover art URL, populated by `fetch_all_albums`.
    art_urls: Mutex<HashMap<String, String>>,
    http: reqwest::Client,
}

impl TidalProvider {
    /// Build a provider and authenticate against Tidal. The blocking API
    /// calls run on the blocking thread pool.
    ///
    /// When a refresh token is present it is exchanged first (getting a fresh
    /// access token); otherwise the stored access token is validated directly.
    /// Both failing is a construction error — surfaced by the caller as a
    /// scan error.
    pub async fn new(
        source_id: SourceId,
        config: TidalProviderConfig,
    ) -> Result<Self, ProviderError> {
        Self::connect(source_id, config, None).await
    }

    /// Like [`Self::new`], but invokes `token_persister` with the rotated
    /// access/refresh tokens after a successful refresh-token exchange (see
    /// [`TidalTokenPersister`]).
    pub async fn new_with_token_persister(
        source_id: SourceId,
        config: TidalProviderConfig,
        token_persister: TidalTokenPersister,
    ) -> Result<Self, ProviderError> {
        Self::connect(source_id, config, Some(token_persister)).await
    }

    async fn connect(
        source_id: SourceId,
        config: TidalProviderConfig,
        token_persister: Option<TidalTokenPersister>,
    ) -> Result<Self, ProviderError> {
        let service = tokio::task::spawn_blocking(move || {
            Self::connect_with(&config, None, None, token_persister.as_ref())
        })
        .await
        .map_err(|e| ProviderError::Other(format!("Tidal auth task failed: {e}")))??;
        Ok(Self::with_service(source_id, service))
    }

    /// Test seam: authenticate like [`Self::new`] but against overridden
    /// API/auth base URLs (a mock server in integration tests).
    #[doc(hidden)]
    pub fn connect_with(
        config: &TidalProviderConfig,
        api_base: Option<&str>,
        auth_base: Option<&str>,
        token_persister: Option<&TidalTokenPersister>,
    ) -> Result<TidalService, ProviderError> {
        let mut service = TidalService::new()
            .with_country_code(&config.country_code)
            .with_quality(config.audio_quality());
        // An empty / whitespace client id means "use the built-in default"
        // (same convention as `ServiceManager::connect_tidal`); passing it
        // through would clobber the default with an empty string.
        let client_id = config.client_id.trim();
        if !client_id.is_empty() {
            service = service.with_client_id(client_id);
        }
        if let Some(api_base) = api_base {
            service = service.with_api_base(api_base);
        }
        if let Some(auth_base) = auth_base {
            service = service.with_auth_base(auth_base);
        }
        let refresh =
            (!config.refresh_token.trim().is_empty()).then(|| config.refresh_token.clone());
        service.set_tokens(config.access_token.clone(), refresh);

        if service.refresh_token().is_some() {
            match service.authenticate_refresh() {
                Ok(()) => {
                    if let Some(persist) = token_persister {
                        let new_access = service.access_token().unwrap_or_default();
                        let new_refresh = service.refresh_token();
                        // Skip the callback when nothing actually rotated.
                        if new_access != config.access_token
                            || new_refresh != Some(config.refresh_token.as_str())
                        {
                            persist(new_access, new_refresh);
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[Tidal] refresh token exchange failed, falling back to stored access token: {e}"
                    );
                }
            }
        }

        let access_token = service
            .access_token()
            .unwrap_or_default()
            .trim()
            .to_string();
        if access_token.is_empty() {
            return Err(ProviderError::Auth(
                "no Tidal access token configured".to_string(),
            ));
        }
        // Validate the (possibly refreshed) access token against /sessions,
        // which also captures the user id required by the favorites endpoints.
        service
            .authenticate(ServiceCredentials::AccessToken(access_token))
            .map_err(|e| ProviderError::Auth(format!("Tidal authentication failed: {e}")))?;
        Ok(service)
    }

    /// Test seam: inject a pre-authenticated service (e.g. pointed at a mock
    /// server via `with_api_base` / `with_auth_base`).
    pub fn with_service(source_id: SourceId, service: TidalService) -> Self {
        Self {
            source_id,
            service: Arc::new(service),
            art_urls: Mutex::new(HashMap::new()),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

impl LibraryProvider for TidalProvider {
    fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    fn display_name(&self) -> &str {
        "Tidal"
    }

    fn source_type(&self) -> SourceType {
        SourceType::Tidal
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            writable: false,
            seekable: true, // direct FLAC/AAC URLs support range requests
            offline_available: false,
            supports_events: false,
            has_album_art: true,
        }
    }

    fn fetch_all_albums(&self) -> ProviderFuture<'_, Result<Vec<ProviderAlbum>, ProviderError>> {
        let service = Arc::clone(&self.service);
        Box::pin(async move {
            let (albums, art) = tokio::task::spawn_blocking(move || {
                let albums = service.favorites_albums().map_err(map_service_error)?;
                let mut out = Vec::with_capacity(albums.len());
                let mut art = HashMap::new();
                for album in albums {
                    // One bad album (404/500 on its track listing) must not
                    // abort the whole scan: skip it and keep the rest.
                    let tracks = match service.album_tracks(&album.id) {
                        Ok(tracks) => tracks,
                        Err(e) => {
                            log::warn!(
                                "[Tidal] skipping album {} ({}) — track listing failed: {e}",
                                album.id,
                                album.title
                            );
                            continue;
                        }
                    };
                    if tracks.is_empty() {
                        continue;
                    }
                    if let Some(url) = &album.album_art_url {
                        art.insert(album.id.clone(), url.clone());
                    }
                    out.push(ProviderAlbum {
                        external_id: album.id,
                        title: album.title,
                        artist: album.artist,
                        year: album.year,
                        album_art_url: album.album_art_url,
                        tracks: tracks
                            .into_iter()
                            .map(|t| service_track_to_provider(ServiceId::Tidal, t))
                            .collect(),
                    });
                }
                Ok::<_, ProviderError>((out, art))
            })
            .await
            .map_err(|e| ProviderError::Other(format!("Tidal favorites task failed: {e}")))??;
            *self.art_urls.lock().unwrap_or_else(|e| e.into_inner()) = art;
            Ok(albums)
        })
    }

    fn fetch_changes_since(
        &self,
        _since: u64,
    ) -> ProviderFuture<'_, Result<Option<Vec<LibraryEvent>>, ProviderError>> {
        Box::pin(async { Ok(None) })
    }

    fn subscribe_events(&self) -> Option<tokio::sync::broadcast::Receiver<LibraryEvent>> {
        None
    }

    fn resolve_source(
        &self,
        track_external_id: &str,
    ) -> ProviderFuture<'_, Result<AudioSource, ProviderError>> {
        let track_id = track_external_id.to_string();
        Box::pin(async move {
            // Do NOT call start_stream here: the engine's service-stream
            // resolver mints a fresh (unexpired) URL at decode time.
            Ok(AudioSource::ServiceStream {
                service: ServiceId::Tidal,
                track_id,
            })
        })
    }

    fn fetch_album_art(
        &self,
        album_external_id: &str,
    ) -> ProviderFuture<'_, Result<Option<Vec<u8>>, ProviderError>> {
        let url = self
            .art_urls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(album_external_id)
            .cloned();
        let http = self.http.clone();
        Box::pin(async move {
            match url {
                Some(url) => fetch_image(&http, &url).await,
                None => Ok(None),
            }
        })
    }

    fn is_available(&self) -> ProviderFuture<'_, bool> {
        let service = Arc::clone(&self.service);
        Box::pin(async move { service.is_authenticated() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_debug_redacts_tokens() {
        let config = TidalProviderConfig {
            access_token: "access-token-abcdefghij".to_string(),
            refresh_token: "refresh-token-abcdefghij".to_string(),
            client_id: "client-id-abcdefghij".to_string(),
            country_code: "US".to_string(),
            quality: "LOSSLESS".to_string(),
        };
        let dbg = format!("{config:?}");
        assert!(!dbg.contains("access-token-abcdefghij"), "leaked: {dbg}");
        assert!(!dbg.contains("refresh-token-abcdefghij"), "leaked: {dbg}");
        assert!(!dbg.contains("client-id-abcdefghij"), "leaked: {dbg}");
        assert!(dbg.contains("US"));
        assert!(dbg.contains("LOSSLESS"));
    }

    #[test]
    fn quality_string_mapping() {
        let quality = |q: &str| {
            TidalProviderConfig {
                access_token: String::new(),
                refresh_token: String::new(),
                client_id: String::new(),
                country_code: "US".to_string(),
                quality: q.to_string(),
            }
            .audio_quality()
        };
        assert_eq!(quality("LOW"), AudioQuality::Low);
        assert_eq!(quality("NORMAL"), AudioQuality::Normal);
        assert_eq!(quality("HIGH"), AudioQuality::High);
        assert_eq!(quality("LOSSLESS"), AudioQuality::Lossless);
        assert_eq!(quality("HI_RES_LOSSLESS"), AudioQuality::HiRes);
        assert_eq!(quality("HI_RES"), AudioQuality::HiRes);
        // Unknown / empty values fall back to lossless.
        assert_eq!(quality(""), AudioQuality::Lossless);
        assert_eq!(quality("garbage"), AudioQuality::Lossless);
    }
}
