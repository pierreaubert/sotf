//! Spotify federation provider — exposes the user's saved albums as
//! federation albums. Tracks carry `AudioSource::ServiceStream` sources;
//! playback goes through librespot's PCM path via the engine's
//! service-stream resolver.
//!
//! Authentication is OAuth2 (PKCE) only — Spotify disabled password login
//! server-side. The provider restores a session from the librespot
//! credential cache; an empty cache is a construction error directing the
//! caller to the OAuth login flow.

use crate::provider::{
    LibraryEvent, LibraryProvider, ProviderAlbum, ProviderCapabilities, ProviderError,
    ProviderFuture, SourceId, SourceType,
};
use crate::service_common::{fetch_image, map_service_error, service_track_to_provider};
use sotf_audio::decoder::{AudioSource, ServiceId};
use sotf_service_spotify::SpotifyService;
use sotf_services::StreamingService;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Connection configuration for a Spotify federation source.
#[derive(Debug, Clone)]
pub struct SpotifyProviderConfig {
    /// Directory holding the librespot credential cache (written by the
    /// OAuth login flow).
    pub cache_dir: PathBuf,
}

/// A Spotify federation provider backed by `SpotifyService`.
pub struct SpotifyProvider {
    source_id: SourceId,
    service: Arc<SpotifyService>,
    /// Album external id → cover art URL, populated by `fetch_all_albums`.
    art_urls: Mutex<HashMap<String, String>>,
    http: reqwest::Client,
}

impl SpotifyProvider {
    /// Build a provider and restore the Spotify session from the credential
    /// cache. The blocking login runs on the blocking thread pool (librespot
    /// requires an ambient tokio runtime, which blocking threads provide).
    pub async fn new(
        source_id: SourceId,
        config: SpotifyProviderConfig,
    ) -> Result<Self, ProviderError> {
        let service = tokio::task::spawn_blocking(move || {
            let mut service = SpotifyService::new();
            match service.login_with_cached_credentials(&config.cache_dir) {
                Ok(true) => Ok(service),
                Ok(false) => Err(ProviderError::Auth(
                    "no cached Spotify credentials; sign in via OAuth first".to_string(),
                )),
                Err(e) => Err(ProviderError::Auth(format!("Spotify login failed: {e}"))),
            }
        })
        .await
        .map_err(|e| ProviderError::Other(format!("Spotify login task failed: {e}")))??;
        Ok(Self::with_service(source_id, service))
    }

    /// Test seam: inject a pre-authenticated service (e.g. with a Web API
    /// pointed at a mock server).
    pub fn with_service(source_id: SourceId, service: SpotifyService) -> Self {
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

impl LibraryProvider for SpotifyProvider {
    fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    fn display_name(&self) -> &str {
        "Spotify"
    }

    fn source_type(&self) -> SourceType {
        SourceType::Spotify
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            writable: false,
            seekable: false, // librespot PCM stream does not support seeking
            offline_available: false,
            supports_events: false,
            has_album_art: true,
        }
    }

    fn fetch_all_albums(&self) -> ProviderFuture<'_, Result<Vec<ProviderAlbum>, ProviderError>> {
        let service = Arc::clone(&self.service);
        Box::pin(async move {
            let (albums, art) = tokio::task::spawn_blocking(move || {
                let albums = service.saved_albums().map_err(map_service_error)?;
                let mut out = Vec::with_capacity(albums.len());
                let mut art = HashMap::new();
                for album in albums {
                    // One bad album (404/500 on its track listing) must not
                    // abort the whole scan: skip it and keep the rest.
                    let tracks = match service.album_tracks(&album.id) {
                        Ok(tracks) => tracks,
                        Err(e) => {
                            log::warn!(
                                "[Spotify] skipping album {} ({}) — track listing failed: {e}",
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
                            .map(|t| service_track_to_provider(ServiceId::Spotify, t))
                            .collect(),
                    });
                }
                Ok::<_, ProviderError>((out, art))
            })
            .await
            .map_err(|e| ProviderError::Other(format!("Spotify library task failed: {e}")))??;
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
            // The engine's service-stream resolver starts the librespot PCM
            // stream at decode time.
            Ok(AudioSource::ServiceStream {
                service: ServiceId::Spotify,
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
