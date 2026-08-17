//! Process-global streaming-service session manager.
//!
//! The engine knows nothing about specific streaming services: when its
//! decoder thread hits an `AudioSource::ServiceStream` it calls the resolver
//! installed by [`install_service_stream_resolver`]. That resolver delegates
//! to the [`ServiceManager`], which owns one lazily-authenticated
//! `TidalService` / `SpotifyService` per process and keeps their credentials
//! in sync with the federation sources persisted in the music database.
//!
//! ## Credential precedence
//!
//! - Tidal: the first *enabled* federation source holding tokens wins; when no
//!   suitable source exists, the `TIDAL_ACCESS_TOKEN` environment variable is
//!   the fallback.
//! - Spotify: credentials always come from the librespot credential cache
//!   under `<config dir>/spotify` (written by the OAuth login flow); the first
//!   enabled Spotify federation source only contributes the quality setting.
//!
//! ## Locking
//!
//! The manager lives behind a process-global `parking_lot::Mutex` — the
//! crate's convention, and unlike `std::sync::Mutex` there is no lock
//! poisoning, so `lock()` cannot panic on the decoder thread. A plain `Mutex`
//! (not `RwLock`) because every interesting operation needs `&mut`
//! (`start_stream`, token refresh). The lock IS held across provider network
//! calls: that serializes concurrent stream starts, which is harmless because
//! the provider crates carry their own 30s HTTP timeouts, and the engine
//! clones the resolver `Arc` out of its own lock before invoking us, so no
//! engine lock is ever held here. Nothing invoked while the lock is held
//! (source loader, token persister, provider calls) re-enters the manager, so
//! the mutex cannot deadlock.

use parking_lot::Mutex;
use sotf_audio::decoder::{ResolvedServiceStream, ServiceId, ServiceStreamResolver};
use std::sync::{Arc, OnceLock};

#[cfg(any(feature = "tidal", feature = "spotify"))]
use crate::federation_config::{FederationSourceEntry, SourceConnectionConfig};
#[cfg(feature = "spotify")]
use sotf_service_spotify::SpotifyService;
#[cfg(feature = "tidal")]
use sotf_service_tidal::TidalService;
#[cfg(any(feature = "tidal", feature = "spotify"))]
use sotf_services::{AudioQuality, ServiceStreamResult, StreamingService};
#[cfg(feature = "tidal")]
use sotf_services::{ServiceCredentials, ServiceError, redact_secret};

/// Errors from streaming-service session management and stream resolution.
#[derive(Debug, thiserror::Error)]
pub enum ServiceManagerError {
    /// The binary was compiled without support for the requested service.
    #[error("{0} streaming support is not compiled in")]
    Unsupported(ServiceId),
    /// No usable credentials were found (DB source + environment fallback).
    #[error("{0}")]
    MissingCredentials(String),
    /// Authentication against the service failed.
    #[error("{0}")]
    Auth(String),
    /// Any other service failure (network, API, ...).
    #[error("{0}")]
    Service(String),
}

/// Tidal credentials and settings selected from a federation source or the
/// environment fallback.
#[cfg(feature = "tidal")]
#[derive(Clone, PartialEq)]
pub struct TidalCredentials {
    /// Federation source the credentials came from (`None` = environment
    /// fallback; there is nothing to persist rotated tokens back to then).
    pub source_id: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    /// Tidal application client ID (empty = built-in default).
    pub client_id: String,
    pub country_code: String,
    /// Quality label as stored in the source config ("LOSSLESS", ...).
    pub quality: String,
}

#[cfg(feature = "tidal")]
impl std::fmt::Debug for TidalCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TidalCredentials")
            .field("source_id", &self.source_id)
            .field("access_token", &redact_secret(&self.access_token))
            .field("refresh_token", &redact_secret(&self.refresh_token))
            .field("client_id", &redact_secret(&self.client_id))
            .field("country_code", &self.country_code)
            .field("quality", &self.quality)
            .finish()
    }
}

/// Pick Tidal credentials: the first enabled federation source holding an
/// access or refresh token beats the `TIDAL_ACCESS_TOKEN` environment
/// fallback. Disabled sources and token-less sources are ignored.
///
/// Pure function — takes the already-loaded sources and the env value so it
/// stays unit-testable without touching the DB or the process environment.
#[cfg(feature = "tidal")]
#[must_use]
pub fn select_tidal_credentials(
    sources: &[FederationSourceEntry],
    env_access_token: Option<&str>,
) -> Option<TidalCredentials> {
    for source in sources {
        if !source.is_enabled {
            continue;
        }
        if let SourceConnectionConfig::Tidal {
            access_token,
            refresh_token,
            client_id,
            country_code,
            quality,
        } = &source.connection
        {
            if access_token.trim().is_empty() && refresh_token.trim().is_empty() {
                continue;
            }
            return Some(TidalCredentials {
                source_id: Some(source.source_id.clone()),
                access_token: access_token.clone(),
                refresh_token: refresh_token.clone(),
                client_id: client_id.clone(),
                country_code: country_code.clone(),
                quality: quality.clone(),
            });
        }
    }

    let token = env_access_token?.trim();
    if token.is_empty() {
        return None;
    }
    Some(TidalCredentials {
        source_id: None,
        access_token: token.to_string(),
        refresh_token: String::new(),
        client_id: String::new(), // TidalService falls back to its default
        country_code: "US".to_string(),
        quality: "LOSSLESS".to_string(),
    })
}

/// Quality from the first enabled Spotify federation source, defaulting to
/// `High` (~320 kbps Vorbis, the best librespot offers).
#[cfg(feature = "spotify")]
#[must_use]
pub fn select_spotify_quality(sources: &[FederationSourceEntry]) -> AudioQuality {
    for source in sources {
        if !source.is_enabled {
            continue;
        }
        if let SourceConnectionConfig::Spotify { quality, .. } = &source.connection {
            return spotify_audio_quality(quality);
        }
    }
    AudioQuality::High
}

/// Same mapping as `TidalProviderConfig::audio_quality` (sotf-federation).
#[cfg(feature = "tidal")]
fn tidal_audio_quality(quality: &str) -> AudioQuality {
    match quality.to_ascii_uppercase().as_str() {
        "LOW" => AudioQuality::Low,
        "NORMAL" => AudioQuality::Normal,
        "HIGH" | "VERYHIGH" | "VERY_HIGH" => AudioQuality::High,
        "HI_RES" | "HI_RES_LOSSLESS" | "HIRES" => AudioQuality::HiRes,
        // "LOSSLESS" and anything unrecognized fall back to lossless.
        _ => AudioQuality::Lossless,
    }
}

#[cfg(feature = "spotify")]
fn spotify_audio_quality(quality: &str) -> AudioQuality {
    match quality.to_ascii_uppercase().as_str() {
        "LOW" => AudioQuality::Low,
        "NORMAL" => AudioQuality::Normal,
        // "HIGH", "VERYHIGH" and anything unrecognized fall back to High —
        // the service maps High/Lossless/HiRes to its best (320 kbps) bitrate.
        _ => AudioQuality::High,
    }
}

/// Map a provider `start_stream` result onto the engine's resolved-stream
/// type. Tidal URLs are seekable (range requests); PCM fields map 1:1.
#[cfg(any(feature = "tidal", feature = "spotify"))]
fn map_service_stream_result(result: ServiceStreamResult) -> ResolvedServiceStream {
    match result {
        ServiceStreamResult::Url { url, format_hint } => ResolvedServiceStream::Url {
            url,
            format_hint,
            seekable: true,
        },
        ServiceStreamResult::Pcm(pcm) => ResolvedServiceStream::Pcm {
            sample_rate: pcm.sample_rate,
            channels: pcm.channels,
            bits_per_sample: pcm.bits_per_sample,
            total_frames: pcm.total_frames,
            reader: pcm.reader,
        },
    }
}

/// Loads the federation sources (default: from the music database).
#[cfg(any(feature = "tidal", feature = "spotify"))]
type SourceLoader = Arc<dyn Fn() -> Result<Vec<FederationSourceEntry>, String> + Send + Sync>;

/// Persists a federation source with rotated Tidal tokens (default: back into
/// the music database via the usual save path).
#[cfg(feature = "tidal")]
type TidalTokenPersister = Arc<dyn Fn(&FederationSourceEntry) -> Result<(), String> + Send + Sync>;

#[cfg(any(feature = "tidal", feature = "spotify"))]
fn default_source_loader() -> SourceLoader {
    Arc::new(|| {
        let path = crate::database::MusicDatabase::default_path()
            .ok_or_else(|| "could not determine the music database path".to_string())?;
        let db = crate::database::MusicDatabase::open(&path)
            .map_err(|e| format!("failed to open the music database: {e}"))?;
        db.load_federation_sources()
    })
}

#[cfg(feature = "tidal")]
fn default_tidal_token_persister() -> TidalTokenPersister {
    Arc::new(crate::service_login::persist_federation_source)
}

#[cfg(feature = "tidal")]
struct TidalState {
    service: TidalService,
    quality: AudioQuality,
    /// Federation source the credentials came from (`None` = env fallback);
    /// needed to persist tokens rotated by a stream-time refresh.
    source_id: Option<String>,
}

#[cfg(feature = "spotify")]
struct SpotifyState {
    service: SpotifyService,
    quality: AudioQuality,
}

/// Owns the per-process streaming-service sessions. Services are created and
/// authenticated lazily on first use, then reused for every track.
///
/// Use the process-global instance via [`resolve_service_stream`] /
/// [`install_service_stream_resolver`] rather than constructing your own —
/// a second manager would double-authenticate against the providers.
pub struct ServiceManager {
    #[cfg(feature = "tidal")]
    tidal: Option<TidalState>,
    #[cfg(feature = "spotify")]
    spotify: Option<SpotifyState>,
    #[cfg(any(feature = "tidal", feature = "spotify"))]
    source_loader: SourceLoader,
    #[cfg(feature = "tidal")]
    tidal_token_persister: TidalTokenPersister,
    /// Test seam: mock API base URL (overrides the real Tidal API).
    #[cfg(feature = "tidal")]
    tidal_api_base: Option<String>,
    /// Test seam: mock auth base URL (overrides the real Tidal auth server).
    #[cfg(feature = "tidal")]
    tidal_auth_base: Option<String>,
    /// Test seam: `Some(value)` overrides the `TIDAL_ACCESS_TOKEN` process
    /// env lookup (empty string = force "missing"), so tests never call the
    /// unsafe-in-edition-2024 `std::env::set_var`.
    #[cfg(feature = "tidal")]
    tidal_env_access_token: Option<String>,
    /// Test seam: overrides the Spotify credential cache directory.
    #[cfg(feature = "spotify")]
    spotify_cache_dir: Option<std::path::PathBuf>,
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceManager {
    /// A manager with the production credential loader/persister. Services
    /// are not contacted until the first [`Self::resolve`] call.
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "tidal")]
            tidal: None,
            #[cfg(feature = "spotify")]
            spotify: None,
            #[cfg(any(feature = "tidal", feature = "spotify"))]
            source_loader: default_source_loader(),
            #[cfg(feature = "tidal")]
            tidal_token_persister: default_tidal_token_persister(),
            #[cfg(feature = "tidal")]
            tidal_api_base: None,
            #[cfg(feature = "tidal")]
            tidal_auth_base: None,
            #[cfg(feature = "tidal")]
            tidal_env_access_token: None,
            #[cfg(feature = "spotify")]
            spotify_cache_dir: None,
        }
    }

    /// Resolve a service stream into something the engine can decode
    /// directly (a URL or a PCM stream).
    pub fn resolve(
        &mut self,
        service: ServiceId,
        track_id: &str,
    ) -> Result<ResolvedServiceStream, ServiceManagerError> {
        match service {
            ServiceId::Tidal => self.resolve_tidal(track_id),
            ServiceId::Spotify => self.resolve_spotify(track_id),
        }
    }

    #[cfg(feature = "tidal")]
    fn resolve_tidal(
        &mut self,
        track_id: &str,
    ) -> Result<ResolvedServiceStream, ServiceManagerError> {
        let state = self.tidal_state()?;
        match state.service.start_stream(track_id, state.quality) {
            Ok(result) => Ok(map_service_stream_result(result)),
            Err(auth @ ServiceError::AuthError(_)) => {
                self.refresh_tidal_once_and_retry(track_id, auth)
            }
            Err(e) => Err(ServiceManagerError::Service(format!(
                "Tidal stream request failed: {e}"
            ))),
        }
    }

    /// Stream-time token recovery: the provider rejected the access token
    /// (HTTP 401/403 surfaced as [`ServiceError::AuthError`]). Refresh once,
    /// persist the rotated tokens back to the source, and retry the stream
    /// request exactly once — never loop. When the refresh itself fails, the
    /// original stream error is the one that surfaces.
    #[cfg(feature = "tidal")]
    fn refresh_tidal_once_and_retry(
        &mut self,
        track_id: &str,
        first_error: ServiceError,
    ) -> Result<ResolvedServiceStream, ServiceManagerError> {
        let Some(state) = self.tidal.as_mut() else {
            return Err(ServiceManagerError::Service(
                "Tidal service initialization failed".to_string(),
            ));
        };
        if let Err(e) = state.service.authenticate_refresh() {
            log::warn!("[Tidal] token refresh after stream auth failure failed: {e}");
            return Err(ServiceManagerError::Auth(format!(
                "Tidal stream request failed: {first_error}"
            )));
        }
        log::info!("[Tidal] access token refreshed after stream auth failure; retrying once");
        // `authenticate_refresh` stores the rotated tokens inside the
        // service, so the retry below picks up the new access token.
        let source_id = state.source_id.clone();
        if let (Some(source_id), Some(state)) = (source_id, self.tidal.as_ref()) {
            self.persist_rotated_tidal_tokens(&source_id, &state.service);
        }
        let Some(state) = self.tidal.as_mut() else {
            return Err(ServiceManagerError::Service(
                "Tidal service initialization failed".to_string(),
            ));
        };
        match state.service.start_stream(track_id, state.quality) {
            Ok(result) => Ok(map_service_stream_result(result)),
            Err(e @ ServiceError::AuthError(_)) => Err(ServiceManagerError::Auth(format!(
                "Tidal stream request failed after token refresh: {e}"
            ))),
            Err(e) => Err(ServiceManagerError::Service(format!(
                "Tidal stream request failed: {e}"
            ))),
        }
    }

    #[cfg(not(feature = "tidal"))]
    fn resolve_tidal(
        &mut self,
        _track_id: &str,
    ) -> Result<ResolvedServiceStream, ServiceManagerError> {
        Err(ServiceManagerError::Unsupported(ServiceId::Tidal))
    }

    #[cfg(feature = "spotify")]
    fn resolve_spotify(
        &mut self,
        track_id: &str,
    ) -> Result<ResolvedServiceStream, ServiceManagerError> {
        let state = self.spotify_state()?;
        let result = state
            .service
            .start_stream(track_id, state.quality)
            .map_err(|e| {
                ServiceManagerError::Service(format!("Spotify stream request failed: {e}"))
            })?;
        Ok(map_service_stream_result(result))
    }

    #[cfg(not(feature = "spotify"))]
    fn resolve_spotify(
        &mut self,
        _track_id: &str,
    ) -> Result<ResolvedServiceStream, ServiceManagerError> {
        Err(ServiceManagerError::Unsupported(ServiceId::Spotify))
    }

    #[cfg(feature = "tidal")]
    fn tidal_state(&mut self) -> Result<&mut TidalState, ServiceManagerError> {
        if self.tidal.is_none() {
            self.tidal = Some(self.connect_tidal()?);
        }
        self.tidal.as_mut().ok_or_else(|| {
            ServiceManagerError::Service("Tidal service initialization failed".to_string())
        })
    }

    /// Build and authenticate the Tidal service. Auth sequence: restore the
    /// stored tokens, exchange the refresh token when present, validate the
    /// access token against `/sessions` (which also captures the user id),
    /// and only then persist the rotated tokens back to the source — so a
    /// failed validation never leaves dead tokens in the database.
    #[cfg(feature = "tidal")]
    fn connect_tidal(&self) -> Result<TidalState, ServiceManagerError> {
        let sources = (self.source_loader)().unwrap_or_else(|e| {
            log::warn!("[Tidal] failed to load federation sources, using env fallback: {e}");
            Vec::new()
        });
        let env_token = match &self.tidal_env_access_token {
            Some(overridden) => Some(overridden.clone()),
            None => std::env::var("TIDAL_ACCESS_TOKEN").ok(),
        };
        let credentials = select_tidal_credentials(&sources, env_token.as_deref()).ok_or_else(|| {
            ServiceManagerError::MissingCredentials(
                "Tidal playback requires TIDAL_ACCESS_TOKEN or a Tidal source configured in settings"
                    .to_string(),
            )
        })?;
        let quality = tidal_audio_quality(&credentials.quality);

        let mut service = TidalService::new()
            .with_country_code(&credentials.country_code)
            .with_quality(quality);
        if !credentials.client_id.trim().is_empty() {
            service = service.with_client_id(&credentials.client_id);
        }
        if let Some(api_base) = &self.tidal_api_base {
            service = service.with_api_base(api_base);
        }
        if let Some(auth_base) = &self.tidal_auth_base {
            service = service.with_auth_base(auth_base);
        }

        let refresh = (!credentials.refresh_token.trim().is_empty())
            .then(|| credentials.refresh_token.clone());
        service.set_tokens(credentials.access_token.clone(), refresh);

        let mut refreshed = false;
        if service.refresh_token().is_some() {
            match service.authenticate_refresh() {
                Ok(()) => {
                    log::info!("[Tidal] access token refreshed via refresh token");
                    refreshed = true;
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
            return Err(ServiceManagerError::MissingCredentials(
                "no Tidal access token configured".to_string(),
            ));
        }
        service
            .authenticate(ServiceCredentials::AccessToken(access_token))
            .map_err(|e| ServiceManagerError::Auth(format!("Tidal authentication failed: {e}")))?;

        // Persist only after the rotated access token validated successfully.
        if refreshed {
            if let Some(source_id) = &credentials.source_id {
                self.persist_rotated_tidal_tokens(source_id, &service);
            }
        }

        Ok(TidalState {
            service,
            quality,
            source_id: credentials.source_id.clone(),
        })
    }

    /// Persist rotated tokens back to the originating federation source.
    /// The entry is re-loaded at persist time and only its token fields are
    /// updated, so user edits made since connect time (quality, country, ...)
    /// are not clobbered by a stale snapshot. Never logs token values;
    /// persistence failures are non-fatal (the in-memory session still works
    /// for this run).
    #[cfg(feature = "tidal")]
    fn persist_rotated_tidal_tokens(&self, source_id: &str, service: &TidalService) {
        let new_access = service.access_token().unwrap_or_default();
        let new_refresh = service.refresh_token().unwrap_or_default();
        let sources = match (self.source_loader)() {
            Ok(sources) => sources,
            Err(e) => {
                log::warn!("[Tidal] failed to re-load sources; cannot persist rotated tokens: {e}");
                return;
            }
        };
        let Some(mut entry) = sources
            .iter()
            .find(|entry| entry.source_id == source_id)
            .cloned()
        else {
            log::warn!("[Tidal] source {source_id} vanished; cannot persist rotated tokens");
            return;
        };
        let SourceConnectionConfig::Tidal {
            access_token,
            refresh_token,
            ..
        } = &mut entry.connection
        else {
            log::warn!(
                "[Tidal] source {source_id} is no longer a Tidal source; cannot persist rotated tokens"
            );
            return;
        };
        if *access_token == new_access && *refresh_token == new_refresh {
            return; // nothing rotated relative to what is stored
        }
        *access_token = new_access.to_string();
        *refresh_token = new_refresh.to_string();
        log::info!("[Tidal] persisting rotated tokens for source {source_id}");
        if let Err(e) = (self.tidal_token_persister)(&entry) {
            log::warn!("[Tidal] failed to persist rotated tokens for source {source_id}: {e}");
        }
    }

    #[cfg(feature = "spotify")]
    fn spotify_state(&mut self) -> Result<&mut SpotifyState, ServiceManagerError> {
        if self.spotify.is_none() {
            self.spotify = Some(self.connect_spotify()?);
        }
        self.spotify.as_mut().ok_or_else(|| {
            ServiceManagerError::Service("Spotify service initialization failed".to_string())
        })
    }

    /// Restore the Spotify session from the librespot credential cache
    /// (written by the OAuth login flow in the settings UI).
    #[cfg(feature = "spotify")]
    fn connect_spotify(&self) -> Result<SpotifyState, ServiceManagerError> {
        let sources = (self.source_loader)().unwrap_or_else(|e| {
            log::warn!("[Spotify] failed to load federation sources for quality setting: {e}");
            Vec::new()
        });
        let quality = select_spotify_quality(&sources);

        let cache_dir = match &self.spotify_cache_dir {
            Some(dir) => dir.clone(),
            None => crate::service_login::spotify_cache_dir().ok_or_else(|| {
                ServiceManagerError::MissingCredentials(
                    "could not determine config directory for the Spotify credential cache"
                        .to_string(),
                )
            })?,
        };

        let mut service = SpotifyService::new().with_quality(quality);
        match service.login_with_cached_credentials(&cache_dir) {
            Ok(true) => Ok(SpotifyState { service, quality }),
            Ok(false) => Err(ServiceManagerError::MissingCredentials(
                "Spotify playback requires cached credentials; sign in via the Spotify settings first"
                    .to_string(),
            )),
            Err(e) => Err(ServiceManagerError::Auth(format!("Spotify login failed: {e}"))),
        }
    }
}

/// Test seams — only compiled into test builds so production code paths
/// cannot accidentally use them.
#[cfg(test)]
impl ServiceManager {
    #[cfg(any(feature = "tidal", feature = "spotify"))]
    pub(crate) fn with_test_source_loader(mut self, loader: SourceLoader) -> Self {
        self.source_loader = loader;
        self
    }

    #[cfg(feature = "tidal")]
    pub(crate) fn with_test_tidal_token_persister(
        mut self,
        persister: TidalTokenPersister,
    ) -> Self {
        self.tidal_token_persister = persister;
        self
    }

    #[cfg(feature = "tidal")]
    pub(crate) fn with_test_tidal_bases(mut self, api_base: &str, auth_base: &str) -> Self {
        self.tidal_api_base = Some(api_base.to_string());
        self.tidal_auth_base = Some(auth_base.to_string());
        self
    }

    /// `Some("")` forces "env token missing" deterministically.
    #[cfg(feature = "tidal")]
    pub(crate) fn with_test_tidal_env_token(mut self, token: &str) -> Self {
        self.tidal_env_access_token = Some(token.to_string());
        self
    }

    #[cfg(feature = "spotify")]
    pub(crate) fn with_test_spotify_cache_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.spotify_cache_dir = Some(dir);
        self
    }
}

static SERVICE_MANAGER: OnceLock<Mutex<ServiceManager>> = OnceLock::new();

fn global_manager() -> &'static Mutex<ServiceManager> {
    SERVICE_MANAGER.get_or_init(|| Mutex::new(ServiceManager::new()))
}

/// Replace the process-global manager. Tests use this to inject mock-backed
/// credential loaders; production code should never call it.
#[cfg(test)]
pub(crate) fn install_manager_for_tests(manager: ServiceManager) {
    *global_manager().lock() = manager;
}

/// Serializes tests that touch the process-global manager. Defined here so
/// both this module's and `service_streams`' tests share one lock.
#[cfg(test)]
pub(crate) static SERVICE_STREAM_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Install the engine's service-stream resolver backed by the process-global
/// [`ServiceManager`]. Call once at app startup; calling again replaces the
/// resolver (the manager is shared, so no re-authentication happens).
pub fn install_service_stream_resolver() {
    let resolver: ServiceStreamResolver = Arc::new(resolve_service_stream);
    sotf_audio::decoder::set_service_stream_resolver(resolver);
    log::info!(
        "Service stream resolver installed (tidal: {}, spotify: {})",
        cfg!(feature = "tidal"),
        cfg!(feature = "spotify")
    );
}

/// Remove the engine resolver and reset the global manager (mainly for
/// tests and shutdown; the next resolve re-authenticates from scratch).
pub fn clear_service_stream_resolver() {
    sotf_audio::decoder::clear_service_stream_resolver();
    *global_manager().lock() = ServiceManager::new();
}

/// Drop the cached authenticated sessions so the next stream resolution
/// re-authenticates from the persisted federation sources. Frontends call
/// this after a login/logout changed the stored credentials; the engine
/// resolver stays installed.
pub fn reset_service_sessions() {
    *global_manager().lock() = ServiceManager::new();
}

/// Resolve through the process-global manager, mapping every failure —
/// including a panicking provider call — to `Err(String)` as the engine's
/// resolver contract requires. The engine additionally wraps resolvers in
/// its own `catch_unwind`; the one here keeps the legacy
/// `resolve_service_stream_from_env` path panic-free too.
pub fn resolve_service_stream(
    service: ServiceId,
    track_id: &str,
) -> Result<ResolvedServiceStream, String> {
    resolve_typed(service, track_id).map_err(|e| e.to_string())
}

/// Typed variant of [`resolve_service_stream`] for in-crate callers (the
/// legacy `service_streams` shim maps the variants onto its own error enum).
pub(crate) fn resolve_typed(
    service: ServiceId,
    track_id: &str,
) -> Result<ResolvedServiceStream, ServiceManagerError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        global_manager().lock().resolve(service, track_id)
    }))
    .unwrap_or_else(|_| {
        Err(ServiceManagerError::Service(format!(
            "{service} stream resolution panicked"
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        SERVICE_STREAM_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[cfg(feature = "tidal")]
    fn tidal_entry(
        source_id: &str,
        enabled: bool,
        access: &str,
        refresh: &str,
    ) -> FederationSourceEntry {
        FederationSourceEntry {
            source_id: source_id.to_string(),
            display_name: "Tidal".to_string(),
            priority: 0,
            is_enabled: enabled,
            connection: SourceConnectionConfig::Tidal {
                access_token: access.to_string(),
                client_id: "test-client-id".to_string(),
                refresh_token: refresh.to_string(),
                quality: "LOSSLESS".to_string(),
                country_code: "US".to_string(),
            },
            is_available: None,
        }
    }

    #[cfg(feature = "spotify")]
    fn spotify_entry(source_id: &str, enabled: bool, quality: &str) -> FederationSourceEntry {
        FederationSourceEntry {
            source_id: source_id.to_string(),
            display_name: "Spotify".to_string(),
            priority: 0,
            is_enabled: enabled,
            connection: SourceConnectionConfig::Spotify {
                username: "user".to_string(),
                password: "pass".to_string(),
                quality: quality.to_string(),
            },
            is_available: None,
        }
    }

    // -- Credential selection (pure) --------------------------------------

    #[cfg(feature = "tidal")]
    #[test]
    fn enabled_db_source_beats_env() {
        let sources = vec![tidal_entry(
            "tidal:db",
            true,
            "db-access-token",
            "db-refresh-token",
        )];
        let creds = select_tidal_credentials(&sources, Some("env-access-token"))
            .expect("db source should win");
        assert_eq!(creds.source_id.as_deref(), Some("tidal:db"));
        assert_eq!(creds.access_token, "db-access-token");
        assert_eq!(creds.refresh_token, "db-refresh-token");
    }

    #[cfg(feature = "tidal")]
    #[test]
    fn disabled_source_is_ignored_in_favor_of_env() {
        let sources = vec![tidal_entry("tidal:db", false, "db-access-token", "")];
        let creds = select_tidal_credentials(&sources, Some("env-access-token"))
            .expect("env fallback should apply");
        assert_eq!(creds.source_id, None);
        assert_eq!(creds.access_token, "env-access-token");
        assert_eq!(creds.quality, "LOSSLESS");
    }

    #[cfg(feature = "tidal")]
    #[test]
    fn token_less_source_is_skipped_and_missing_everything_yields_none() {
        let sources = vec![tidal_entry("tidal:empty", true, "", "")];
        assert!(select_tidal_credentials(&sources, Some("env-token")).is_some());
        assert!(select_tidal_credentials(&sources, None).is_none());
        assert!(select_tidal_credentials(&[], None).is_none());
        assert!(select_tidal_credentials(&[], Some("   ")).is_none());
    }

    #[cfg(feature = "tidal")]
    #[test]
    fn credentials_debug_redacts_tokens() {
        let creds = select_tidal_credentials(
            &[tidal_entry(
                "tidal:db",
                true,
                "access-token-abcdefghij",
                "refresh-token-abcdefghij",
            )],
            None,
        )
        .expect("creds");
        let dbg = format!("{creds:?}");
        assert!(!dbg.contains("access-token-abcdefghij"), "leaked: {dbg}");
        assert!(!dbg.contains("refresh-token-abcdefghij"), "leaked: {dbg}");
        assert!(dbg.contains("acce***"), "expected redaction: {dbg}");
    }

    #[cfg(feature = "tidal")]
    #[test]
    fn tidal_quality_mapping() {
        assert_eq!(tidal_audio_quality("LOW"), AudioQuality::Low);
        assert_eq!(tidal_audio_quality("NORMAL"), AudioQuality::Normal);
        assert_eq!(tidal_audio_quality("HIGH"), AudioQuality::High);
        assert_eq!(tidal_audio_quality("LOSSLESS"), AudioQuality::Lossless);
        assert_eq!(tidal_audio_quality("HI_RES_LOSSLESS"), AudioQuality::HiRes);
        assert_eq!(tidal_audio_quality("garbage"), AudioQuality::Lossless);
    }

    #[cfg(feature = "spotify")]
    #[test]
    fn spotify_quality_comes_from_first_enabled_source() {
        assert_eq!(select_spotify_quality(&[]), AudioQuality::High);
        let disabled = vec![spotify_entry("spotify:off", false, "Low")];
        assert_eq!(select_spotify_quality(&disabled), AudioQuality::High);
        let enabled = vec![
            spotify_entry("spotify:off", false, "Low"),
            spotify_entry("spotify:on", true, "Normal"),
        ];
        assert_eq!(select_spotify_quality(&enabled), AudioQuality::Normal);
        assert_eq!(spotify_audio_quality("VeryHigh"), AudioQuality::High);
        assert_eq!(spotify_audio_quality("unknown"), AudioQuality::High);
    }

    // -- Stream-result mapping (pure) --------------------------------------

    #[cfg(any(feature = "tidal", feature = "spotify"))]
    #[test]
    fn url_result_maps_to_seekable_url() {
        let resolved = map_service_stream_result(ServiceStreamResult::Url {
            url: "https://example.com/x.flac".to_string(),
            format_hint: Some("flac".to_string()),
        });
        match resolved {
            ResolvedServiceStream::Url {
                url,
                format_hint,
                seekable,
            } => {
                assert_eq!(url, "https://example.com/x.flac");
                assert_eq!(format_hint.as_deref(), Some("flac"));
                assert!(seekable);
            }
            ResolvedServiceStream::Pcm { .. } => panic!("expected Url"),
        }
    }

    #[cfg(any(feature = "tidal", feature = "spotify"))]
    #[test]
    fn pcm_result_maps_fields_one_to_one() {
        let reader: Box<dyn std::io::Read + Send> = Box::new(std::io::Cursor::new(vec![0u8; 8]));
        let resolved =
            map_service_stream_result(ServiceStreamResult::Pcm(sotf_services::PcmStream {
                sample_rate: 44_100,
                channels: 2,
                bits_per_sample: 32,
                total_frames: Some(123),
                reader,
            }));
        match resolved {
            ResolvedServiceStream::Pcm {
                sample_rate,
                channels,
                bits_per_sample,
                total_frames,
                mut reader,
            } => {
                assert_eq!(sample_rate, 44_100);
                assert_eq!(channels, 2);
                assert_eq!(bits_per_sample, 32);
                assert_eq!(total_frames, Some(123));
                let mut buf = Vec::new();
                std::io::Read::read_to_end(&mut reader, &mut buf).expect("read");
                assert_eq!(buf.len(), 8);
            }
            ResolvedServiceStream::Url { .. } => panic!("expected Pcm"),
        }
    }

    // -- Tidal end-to-end against a mock server ----------------------------

    /// Minimal loopback mock HTTP server (same pattern as
    /// `sotf-federation/tests/common/mod.rs`, kept std-only).
    #[cfg(feature = "tidal")]
    mod mock {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::Arc;
        use std::time::Duration;

        pub(crate) struct MockServer {
            pub(crate) base_url: String,
        }

        pub(crate) fn spawn<F>(handler: F) -> MockServer
        where
            F: Fn(&str, &str) -> (u16, String) + Send + Sync + 'static,
        {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
            let addr = listener.local_addr().unwrap();
            let handler = Arc::new(handler);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { break };
                    let handler = Arc::clone(&handler);
                    std::thread::spawn(move || {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let mut data = Vec::new();
                        let mut buf = [0u8; 4096];
                        // Read until the header terminator arrives (request
                        // bodies are irrelevant to these mocks).
                        while !data.windows(4).any(|w| w == b"\r\n\r\n") {
                            match stream.read(&mut buf) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => data.extend_from_slice(&buf[..n]),
                            }
                        }
                        let text = String::from_utf8_lossy(&data).into_owned();
                        let mut parts = text.split_whitespace();
                        let method = parts.next().unwrap_or("").to_string();
                        let path = parts.next().unwrap_or("").to_string();
                        let (status, body) = handler(&method, &path);
                        let reason = if status == 200 { "OK" } else { "Error" };
                        let head = format!(
                            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(body.as_bytes());
                        let _ = stream.flush();
                    });
                }
            });
            MockServer {
                base_url: format!("http://{addr}"),
            }
        }
    }

    /// Mock Tidal server: refresh-token exchange rotates both tokens,
    /// /sessions validates, and track 101 resolves to a FLAC URL.
    #[cfg(feature = "tidal")]
    fn spawn_tidal_mock() -> mock::MockServer {
        mock::spawn(|method, path| {
            if method == "POST" && path == "/token" {
                (
                    200,
                    r#"{"access_token": "refreshed-access-token", "refresh_token": "rotated-refresh-token", "token_type": "Bearer"}"#
                        .to_string(),
                )
            } else if method == "GET" && path.starts_with("/sessions") {
                (200, r#"{"userId": 42, "countryCode": "US"}"#.to_string())
            } else if method == "GET" && path.starts_with("/tracks/101/urlpostpaywall") {
                (
                    200,
                    r#"{"url": "https://cdn.example.com/101.flac", "codec": "FLAC"}"#.to_string(),
                )
            } else {
                (404, "{}".to_string())
            }
        })
    }

    /// Covers both the resolver mapping (Tidal -> Url with flac hint) and the
    /// refresh-token persist-back: the mock rotates the refresh token, and the
    /// injected persister must be called with the new tokens.
    #[cfg(feature = "tidal")]
    #[test]
    fn tidal_resolve_refreshes_persists_and_maps_url() {
        let server = spawn_tidal_mock();
        let saved: Arc<std::sync::Mutex<Vec<FederationSourceEntry>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let saved_sink = Arc::clone(&saved);

        let manager = ServiceManager::new()
            .with_test_source_loader(Arc::new(|| {
                Ok(vec![tidal_entry(
                    "tidal:test",
                    true,
                    "stale-access-token",
                    "stale-refresh-token",
                )])
            }))
            .with_test_tidal_bases(&server.base_url, &server.base_url)
            .with_test_tidal_token_persister(Arc::new(move |entry| {
                saved_sink
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(entry.clone());
                Ok(())
            }));

        let mut manager = manager;
        let resolved = manager
            .resolve(ServiceId::Tidal, "101")
            .expect("tidal resolve");
        match resolved {
            ResolvedServiceStream::Url {
                url,
                format_hint,
                seekable,
            } => {
                assert_eq!(url, "https://cdn.example.com/101.flac");
                assert_eq!(format_hint.as_deref(), Some("flac"));
                assert!(seekable);
            }
            ResolvedServiceStream::Pcm { .. } => panic!("Tidal must resolve to a URL"),
        }

        let saved = saved.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(saved.len(), 1, "rotated tokens must be persisted once");
        assert_eq!(saved[0].source_id, "tidal:test");
        match &saved[0].connection {
            SourceConnectionConfig::Tidal {
                access_token,
                refresh_token,
                ..
            } => {
                assert_eq!(access_token, "refreshed-access-token");
                assert_eq!(refresh_token, "rotated-refresh-token");
            }
            other => panic!("unexpected connection: {}", other.type_name()),
        }
    }

    /// Env fallback: no DB source, `TIDAL_ACCESS_TOKEN` injected through the
    /// test seam (edition 2024 makes `std::env::set_var` unsafe). The env
    /// path has no source to persist to, so the persister must stay silent.
    #[cfg(feature = "tidal")]
    #[test]
    fn tidal_env_fallback_still_works() {
        let server = spawn_tidal_mock();
        let saved: Arc<std::sync::Mutex<Vec<FederationSourceEntry>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let saved_sink = Arc::clone(&saved);

        let mut manager = ServiceManager::new()
            .with_test_source_loader(Arc::new(|| Ok(Vec::new())))
            .with_test_tidal_bases(&server.base_url, &server.base_url)
            .with_test_tidal_env_token("env-access-token")
            .with_test_tidal_token_persister(Arc::new(move |entry| {
                saved_sink
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(entry.clone());
                Ok(())
            }));

        let resolved = manager
            .resolve(ServiceId::Tidal, "101")
            .expect("env fallback resolve");
        assert!(matches!(resolved, ResolvedServiceStream::Url { .. }));
        assert!(
            saved.lock().unwrap_or_else(|e| e.into_inner()).is_empty(),
            "env-based credentials have no source to persist to"
        );
    }

    /// Controlled Tidal mock: counts refresh and stream-URL requests and can
    /// fail the next stream request and/or every refresh, for the stream-time
    /// refresh-and-retry tests.
    #[cfg(feature = "tidal")]
    struct ControlledTidalMock {
        server: mock::MockServer,
        token_calls: Arc<std::sync::atomic::AtomicUsize>,
        stream_calls: Arc<std::sync::atomic::AtomicUsize>,
        fail_next_stream: Arc<std::sync::atomic::AtomicBool>,
        fail_token: Arc<std::sync::atomic::AtomicBool>,
    }

    #[cfg(feature = "tidal")]
    fn spawn_controlled_tidal_mock() -> ControlledTidalMock {
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        let token_calls = Arc::new(AtomicUsize::new(0));
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let fail_next_stream = Arc::new(AtomicBool::new(false));
        let fail_token = Arc::new(AtomicBool::new(false));
        let server = {
            let token_calls = Arc::clone(&token_calls);
            let stream_calls = Arc::clone(&stream_calls);
            let fail_next_stream = Arc::clone(&fail_next_stream);
            let fail_token = Arc::clone(&fail_token);
            mock::spawn(move |method, path| {
                use std::sync::atomic::Ordering;
                if method == "POST" && path == "/token" {
                    token_calls.fetch_add(1, Ordering::SeqCst);
                    if fail_token.load(Ordering::SeqCst) {
                        (401, r#"{"error": "invalid_grant"}"#.to_string())
                    } else {
                        (
                            200,
                            r#"{"access_token": "refreshed-access-token", "refresh_token": "rotated-refresh-token", "token_type": "Bearer"}"#
                                .to_string(),
                        )
                    }
                } else if method == "GET" && path.starts_with("/sessions") {
                    (200, r#"{"userId": 42, "countryCode": "US"}"#.to_string())
                } else if method == "GET" && path.starts_with("/tracks/101/urlpostpaywall") {
                    stream_calls.fetch_add(1, Ordering::SeqCst);
                    if fail_next_stream.swap(false, Ordering::SeqCst) {
                        (401, r#"{"error": "unauthorized"}"#.to_string())
                    } else {
                        (
                            200,
                            r#"{"url": "https://cdn.example.com/101.flac", "codec": "FLAC"}"#
                                .to_string(),
                        )
                    }
                } else {
                    (404, "{}".to_string())
                }
            })
        };
        ControlledTidalMock {
            server,
            token_calls,
            stream_calls,
            fail_next_stream,
            fail_token,
        }
    }

    /// Stream-time 401: refresh once, persist the rotated tokens, retry the
    /// stream request exactly once. Phase 1 connects and streams cleanly;
    /// phase 2 fails one stream request and asserts the retry counters.
    #[cfg(feature = "tidal")]
    #[test]
    fn tidal_stream_401_refreshes_persists_and_retries_once() {
        use std::sync::atomic::Ordering;
        let mock = spawn_controlled_tidal_mock();
        let saved: Arc<std::sync::Mutex<Vec<FederationSourceEntry>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let saved_sink = Arc::clone(&saved);

        let mut manager = ServiceManager::new()
            .with_test_source_loader(Arc::new(|| {
                Ok(vec![tidal_entry(
                    "tidal:test",
                    true,
                    "stale-access-token",
                    "stale-refresh-token",
                )])
            }))
            .with_test_tidal_bases(&mock.server.base_url, &mock.server.base_url)
            .with_test_tidal_token_persister(Arc::new(move |entry| {
                saved_sink
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(entry.clone());
                Ok(())
            }));

        // Phase 1: connect (one refresh + persist) and stream successfully.
        manager
            .resolve(ServiceId::Tidal, "101")
            .expect("phase 1 resolve");
        let token_before = mock.token_calls.load(Ordering::SeqCst);
        let stream_before = mock.stream_calls.load(Ordering::SeqCst);
        let saved_before = saved.lock().unwrap_or_else(|e| e.into_inner()).len();

        // Phase 2: the next stream request is rejected with a 401.
        mock.fail_next_stream.store(true, Ordering::SeqCst);
        let resolved = manager
            .resolve(ServiceId::Tidal, "101")
            .expect("retry after refresh must succeed");
        match resolved {
            ResolvedServiceStream::Url { url, .. } => {
                assert_eq!(url, "https://cdn.example.com/101.flac");
            }
            ResolvedServiceStream::Pcm { .. } => panic!("Tidal must resolve to a URL"),
        }

        assert_eq!(
            mock.token_calls.load(Ordering::SeqCst) - token_before,
            1,
            "exactly one refresh for the stream-time 401"
        );
        assert_eq!(
            mock.stream_calls.load(Ordering::SeqCst) - stream_before,
            2,
            "the failed request plus exactly one retry"
        );
        let saved = saved.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            saved.len() - saved_before,
            1,
            "exactly one persist for the stream-time rotation"
        );
        match &saved.last().expect("persisted entry").connection {
            SourceConnectionConfig::Tidal {
                access_token,
                refresh_token,
                ..
            } => {
                assert_eq!(access_token, "refreshed-access-token");
                assert_eq!(refresh_token, "rotated-refresh-token");
            }
            other => panic!("unexpected connection: {}", other.type_name()),
        }
    }

    /// Stream-time 401 where the refresh itself fails: the original stream
    /// auth error surfaces and `start_stream` is NOT retried.
    #[cfg(feature = "tidal")]
    #[test]
    fn tidal_stream_401_with_failed_refresh_surfaces_original_error() {
        use std::sync::atomic::Ordering;
        let mock = spawn_controlled_tidal_mock();
        let saved: Arc<std::sync::Mutex<Vec<FederationSourceEntry>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let saved_sink = Arc::clone(&saved);

        let mut manager = ServiceManager::new()
            .with_test_source_loader(Arc::new(|| {
                Ok(vec![tidal_entry(
                    "tidal:test",
                    true,
                    "stale-access-token",
                    "stale-refresh-token",
                )])
            }))
            .with_test_tidal_bases(&mock.server.base_url, &mock.server.base_url)
            .with_test_tidal_token_persister(Arc::new(move |entry| {
                saved_sink
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(entry.clone());
                Ok(())
            }));

        // Phase 1: connect and stream successfully.
        manager
            .resolve(ServiceId::Tidal, "101")
            .expect("phase 1 resolve");
        let token_before = mock.token_calls.load(Ordering::SeqCst);
        let stream_before = mock.stream_calls.load(Ordering::SeqCst);
        let saved_before = saved.lock().unwrap_or_else(|e| e.into_inner()).len();

        // Phase 2: stream 401, and the refresh token exchange fails too.
        mock.fail_next_stream.store(true, Ordering::SeqCst);
        mock.fail_token.store(true, Ordering::SeqCst);
        let err = manager
            .resolve(ServiceId::Tidal, "101")
            .expect_err("failed refresh must surface the original auth error");
        assert!(
            matches!(err, ServiceManagerError::Auth(_)),
            "expected Auth error, got: {err}"
        );
        assert!(
            err.to_string().contains("stream access denied"),
            "original stream error must surface, got: {err}"
        );

        assert_eq!(
            mock.token_calls.load(Ordering::SeqCst) - token_before,
            1,
            "exactly one refresh attempt"
        );
        assert_eq!(
            mock.stream_calls.load(Ordering::SeqCst) - stream_before,
            1,
            "start_stream must not be retried when the refresh fails"
        );
        assert_eq!(
            saved.lock().unwrap_or_else(|e| e.into_inner()).len() - saved_before,
            0,
            "no tokens rotated, so nothing to persist"
        );
    }

    #[cfg(feature = "tidal")]
    #[test]
    fn missing_credentials_error_mentions_env_var() {
        let mut manager = ServiceManager::new()
            .with_test_source_loader(Arc::new(|| Ok(Vec::new())))
            .with_test_tidal_env_token(""); // force "missing"
        let err = manager
            .resolve(ServiceId::Tidal, "101")
            .expect_err("no credentials anywhere");
        assert!(matches!(err, ServiceManagerError::MissingCredentials(_)));
        assert!(err.to_string().contains("TIDAL_ACCESS_TOKEN"));
    }

    // -- Global resolver install/clear --------------------------------------
    //
    // Everything touching the process-global manager + engine resolver lives
    // in this ONE test so it cannot race with itself or with the
    // `service_streams` tests (which hold the same lock).

    #[test]
    fn install_resolve_and_clear_global_resolver() {
        let _lock = test_lock();

        struct ResolverGuard;
        impl Drop for ResolverGuard {
            fn drop(&mut self) {
                clear_service_stream_resolver();
            }
        }
        let _guard = ResolverGuard;

        install_service_stream_resolver();
        assert!(sotf_audio::decoder::has_service_stream_resolver());

        // Deterministic global manager: no DB sources, forced-missing env
        // token, empty Spotify cache.
        #[allow(unused_mut)]
        let mut manager = ServiceManager::new();
        #[cfg(any(feature = "tidal", feature = "spotify"))]
        {
            manager = manager.with_test_source_loader(Arc::new(|| Ok(Vec::new())));
        }
        #[cfg(feature = "tidal")]
        {
            manager = manager.with_test_tidal_env_token("");
        }
        #[cfg(feature = "spotify")]
        let spotify_cache_dir = tempfile::tempdir().expect("temp cache dir");
        #[cfg(feature = "spotify")]
        {
            manager =
                manager.with_test_spotify_cache_dir(spotify_cache_dir.path().to_path_buf());
        }
        install_manager_for_tests(manager);

        let err = resolve_service_stream(ServiceId::Tidal, "1").expect_err("no tidal creds");
        #[cfg(not(feature = "tidal"))]
        assert!(err.contains("not compiled in"), "got: {err}");
        #[cfg(feature = "tidal")]
        assert!(err.contains("TIDAL_ACCESS_TOKEN"), "got: {err}");

        let err = resolve_service_stream(ServiceId::Spotify, "x").expect_err("no spotify session");
        #[cfg(not(feature = "spotify"))]
        assert!(err.contains("not compiled in"), "got: {err}");
        #[cfg(feature = "spotify")]
        assert!(err.contains("sign in"), "got: {err}");

        clear_service_stream_resolver();
        assert!(!sotf_audio::decoder::has_service_stream_resolver());
    }
}
