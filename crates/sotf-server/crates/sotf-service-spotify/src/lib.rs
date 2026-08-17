// ============================================================================
// Spotify Integration via librespot
// ============================================================================
//
// Uses librespot to connect to Spotify and decode audio.
// librespot decodes Vorbis/AAC internally and provides raw PCM samples.
// We capture these via a custom Sink and feed them to the engine's decoder thread.
//
// Authentication is OAuth2 authorization-code + PKCE (see `oauth.rs`):
// Spotify disabled username/password login server-side. Search and library
// browsing go through the Spotify Web API with the OAuth access token
// (see `web_api.rs`).

use sotf_services::*;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;

mod async_runtime;
mod consts;
mod misc;
mod oauth;
#[cfg(test)]
mod test_util;
mod token_store;
mod web_api;

/// Per-packet channel capacity between librespot's `Sink` and the decoder
/// thread. Librespot delivers packets of a few thousand samples each, so 16
/// in-flight packets gives roughly one second of buffering at 44.1 kHz stereo
/// without bloating the decoder thread's working set.
const PCM_CHANNEL_CAPACITY: usize = 16;

/// f32 sample clamp range. Librespot's internal pipeline produces normalised
/// f64 samples in [-1.0, 1.0], but the dithering / normalisation passes can
/// momentarily push values slightly past the limits. We clamp before
/// downstream processing to keep the engine's contract that PCM samples are
/// inside [-1, 1].
const F32_SAMPLE_MIN: f32 = -1.0;
const F32_SAMPLE_MAX: f32 = 1.0;

pub struct SpotifyService {
    session: Option<librespot_core::Session>,
    quality: AudioQuality,
    /// Active librespot player handle. Kept alive across `start_stream` so the
    /// decoder thread keeps running, and shut down explicitly in
    /// `stop_stream` (or on drop) to release CPU and network resources.
    player: Option<std::sync::Arc<librespot_playback::player::Player>>,
    /// Drives async HTTP from the sync trait surface (ambient runtime when
    /// available, embedded fallback otherwise).
    rt: Arc<async_runtime::AsyncRuntime>,
    /// OAuth access token; backs the Web API client. Never logged unredacted.
    access_token: Option<String>,
    web_api: Option<web_api::SpotifyWebApi>,
}

impl std::fmt::Debug for SpotifyService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpotifyService")
            .field("session", &self.session.is_some())
            .field("quality", &self.quality)
            .field("player", &self.player.is_some())
            .field(
                "access_token",
                &self.access_token.as_deref().map(redact_secret),
            )
            .field("web_api", &self.web_api.is_some())
            .finish()
    }
}

impl Default for SpotifyService {
    fn default() -> Self {
        Self::new()
    }
}

impl SpotifyService {
    pub fn new() -> Self {
        let rt = async_runtime::AsyncRuntime::new()
            .expect("Failed to create async runtime for SpotifyService");
        Self {
            session: None,
            quality: AudioQuality::High,
            player: None,
            rt: Arc::new(rt),
            access_token: None,
            web_api: None,
        }
    }

    pub fn with_quality(mut self, quality: AudioQuality) -> Self {
        self.quality = quality;
        self
    }

    fn quality_to_bitrate(&self) -> librespot_playback::config::Bitrate {
        match self.quality {
            AudioQuality::Low => librespot_playback::config::Bitrate::Bitrate96,
            AudioQuality::Normal => librespot_playback::config::Bitrate::Bitrate160,
            AudioQuality::High | AudioQuality::Lossless | AudioQuality::HiRes => {
                librespot_playback::config::Bitrate::Bitrate320
            }
        }
    }

    /// Interactive OAuth login (authorization code + PKCE).
    ///
    /// Builds the Spotify authorize URL, hands it to `open_url` (the UI
    /// opens a browser), then waits on a loopback listener
    /// (`http://127.0.0.1:8898/login`) for the callback, exchanges the code
    /// for an access token, and connects a librespot `Session`. The resulting
    /// credentials are persisted under `cache_dir` so the next run can use
    /// [`Self::login_with_cached_credentials`] instead, along with the Web
    /// API token pair (`web_api_token.json`) used for search/library calls.
    ///
    /// Works from any thread: the librespot session work runs inside the
    /// service's own runtime (see `async_runtime.rs`).
    pub fn login_with_oauth(
        &mut self,
        cache_dir: &Path,
        open_url: impl FnOnce(&str),
    ) -> Result<(), ServiceError> {
        let client = oauth::build_client(
            consts::SPOTIFY_CLIENT_ID,
            consts::SPOTIFY_REDIRECT_URI,
            consts::SPOTIFY_AUTH_URL,
            consts::SPOTIFY_TOKEN_URL,
        )?;
        let request = oauth::build_authorize_request(&client, &consts::SPOTIFY_SCOPES);

        // The redirect URI (and thus the port) is registered against the
        // client ID, so it must match exactly — bind the fixed port.
        let listener = std::net::TcpListener::bind("127.0.0.1:8898").map_err(|e| {
            ServiceError::Other(format!(
                "Cannot bind OAuth loopback listener on 127.0.0.1:8898 ({e}); \
                 is another login already in progress?"
            ))
        })?;

        open_url(&request.url);

        let code = oauth::await_auth_code(
            &listener,
            &request.csrf_state,
            consts::OAUTH_CALLBACK_TIMEOUT,
        )?;
        let token = oauth::exchange_code(&client, code, request.verifier, &consts::SPOTIFY_SCOPES)?;
        log::info!(
            "[Spotify] OAuth login returned access token {}",
            redact_secret(&token.access_token)
        );

        // Persist the Web API token pair so the next restart can restore (and
        // refresh) search/library access without a new browser login — the
        // credentials librespot caches do not carry a usable Web API token.
        let web_token = token_store::WebApiToken::new(
            token.access_token.clone(),
            token.refresh_token.clone(),
            token
                .expires_at
                .saturating_duration_since(std::time::Instant::now()),
        );
        if let Err(e) = web_token.save(cache_dir) {
            log::warn!("[Spotify] Failed to persist Web API token: {e}");
        }

        self.connect_with_access_token(&token.access_token, Some(cache_dir), Some(web_token))
    }

    /// Restore a session from credentials cached by a previous OAuth login.
    ///
    /// Returns `Ok(false)` when the cache holds no credentials (the caller
    /// should fall back to [`Self::login_with_oauth`]).
    ///
    /// Also restores Web API access: after a restart the librespot cache
    /// holds *stored* credentials (not the OAuth token), so the token pair
    /// persisted in `web_api_token.json` is used instead, refreshing it
    /// against the token endpoint when expired.
    ///
    /// Works from any thread: the librespot session work runs inside the
    /// service's own runtime (see `async_runtime.rs`).
    pub fn login_with_cached_credentials(
        &mut self,
        cache_dir: &Path,
    ) -> Result<bool, ServiceError> {
        let cache = librespot_core::cache::Cache::new(Some(cache_dir), None, None, None)
            .map_err(|e| ServiceError::Other(format!("Failed to open credential cache: {e}")))?;
        let Some(credentials) = cache.credentials() else {
            return Ok(false);
        };

        // Token-based cached credentials double as the Web API token
        // (same-process re-login). After a restart the cache holds stored
        // credentials instead, so fall back to the persisted token file.
        let mut web_token = if credentials.auth_type
            == librespot_protocol::authentication::AuthenticationType::AUTHENTICATION_SPOTIFY_TOKEN
        {
            String::from_utf8(credentials.auth_data.clone())
                .ok()
                .map(|access_token| token_store::WebApiToken {
                    access_token,
                    refresh_token: String::new(),
                    // Validity is unknown; a stale token is recovered by the
                    // 401 refresh-retry in the Web API client.
                    expires_at: u64::MAX,
                })
        } else {
            token_store::WebApiToken::load(cache_dir)
        };

        // Refresh an expired Web API token before connecting, so search and
        // library calls work right after the session is restored.
        if let Some(token) = &web_token
            && token.is_expired()
            && !token.refresh_token.is_empty()
        {
            match self.rt.block_on(oauth::refresh_access_token(
                consts::SPOTIFY_TOKEN_URL,
                consts::SPOTIFY_CLIENT_ID,
                &token.refresh_token,
            )) {
                Ok(fresh) => {
                    if let Err(e) = fresh.save(cache_dir) {
                        log::warn!("[Spotify] Failed to persist refreshed Web API token: {e}");
                    }
                    web_token = Some(fresh);
                }
                Err(e) => {
                    // Keep the stale token: the Web API client's 401
                    // refresh-retry gets another shot on the next call.
                    log::warn!("[Spotify] Web API token refresh failed: {e}");
                }
            }
        }

        let session = self
            .rt
            .block_on(async {
                let session = librespot_core::Session::new(
                    librespot_core::SessionConfig::default(),
                    Some(cache),
                );
                session.connect(credentials, false).await.map(|_| session)
            })
            .map_err(|e| ServiceError::AuthError(format!("Spotify cached login failed: {e}")))?;

        log::info!("[Spotify] Authenticated from cached credentials");
        self.session = Some(session);
        if let Some(token) = web_token {
            self.access_token = Some(token.access_token.clone());
            let mut api = web_api::SpotifyWebApi::new(token.access_token, Arc::clone(&self.rt));
            if !token.refresh_token.is_empty() {
                api = api.with_refresh(Some(cache_dir.to_path_buf()), token.refresh_token);
            }
            self.web_api = Some(api);
        }
        Ok(true)
    }

    /// Connect a librespot session with an OAuth access token and set up the
    /// Web API client. When `cache_dir` is given, librespot persists the
    /// (reusable) credentials it negotiates there. `web_token` carries the
    /// refresh token when the access token came from the OAuth flow.
    fn connect_with_access_token(
        &mut self,
        token: &str,
        cache_dir: Option<&Path>,
        web_token: Option<token_store::WebApiToken>,
    ) -> Result<(), ServiceError> {
        // `Session::new` calls `Handle::current()`; running it inside
        // `self.rt.block_on` guarantees an entered runtime (the service's own
        // fallback when the calling thread has none), so this works from any
        // thread.
        let cache = match cache_dir {
            Some(dir) => Some(
                librespot_core::cache::Cache::new(Some(dir), None, None, None).map_err(|e| {
                    ServiceError::Other(format!("Failed to open credential cache: {e}"))
                })?,
            ),
            None => None,
        };
        let store_credentials = cache.is_some();

        let credentials = librespot_core::authentication::Credentials::with_access_token(token);
        let session = self
            .rt
            .block_on(async {
                let session =
                    librespot_core::Session::new(librespot_core::SessionConfig::default(), cache);
                session
                    .connect(credentials, store_credentials)
                    .await
                    .map(|_| session)
            })
            .map_err(|e| ServiceError::AuthError(format!("Spotify login failed: {e}")))?;

        log::info!(
            "[Spotify] Authenticated successfully (token {})",
            redact_secret(token)
        );
        self.session = Some(session);
        self.access_token = Some(token.to_string());
        let mut api = web_api::SpotifyWebApi::new(token.to_string(), Arc::clone(&self.rt));
        if let Some(web_token) = web_token
            && !web_token.refresh_token.is_empty()
        {
            api = api.with_refresh(cache_dir.map(Path::to_path_buf), web_token.refresh_token);
        }
        self.web_api = Some(api);
        Ok(())
    }

    fn web_api(&self) -> Result<&web_api::SpotifyWebApi, ServiceError> {
        self.web_api.as_ref().ok_or_else(|| {
            ServiceError::AuthError(
                "Not authenticated with the Spotify Web API (no access token); \
                 use login_with_oauth() or login_with_cached_credentials()"
                    .to_string(),
            )
        })
    }

    /// Albums saved to the user's library (requires the `user-library-read`
    /// scope, requested by `login_with_oauth`).
    pub fn saved_albums(&self) -> Result<Vec<ServiceAlbum>, ServiceError> {
        self.web_api()?.saved_albums()
    }

    /// Tracks saved to the user's library (requires the `user-library-read`
    /// scope, requested by `login_with_oauth`).
    pub fn saved_tracks(&self) -> Result<Vec<ServiceTrack>, ServiceError> {
        self.web_api()?.saved_tracks()
    }

    /// Test seam for downstream integration tests: inject a Web API client
    /// pointed at `api_base` with the given access token, bypassing login.
    /// Library/search calls work against the mock; `start_stream` and
    /// `is_authenticated` still require a real librespot session.
    #[doc(hidden)]
    pub fn with_test_web_api(mut self, api_base: &str, access_token: &str) -> Self {
        self.access_token = Some(access_token.to_string());
        self.web_api = Some(
            web_api::SpotifyWebApi::new(access_token.to_string(), Arc::clone(&self.rt))
                .with_api_base(api_base),
        );
        self
    }
}

impl StreamingService for SpotifyService {
    fn authenticate(&mut self, credentials: ServiceCredentials) -> Result<(), ServiceError> {
        match credentials {
            ServiceCredentials::UsernamePassword { .. } => Err(ServiceError::AuthError(
                "Spotify disabled username/password login server-side; use \
                 SpotifyService::login_with_oauth() to sign in via browser OAuth"
                    .to_string(),
            )),
            ServiceCredentials::AccessToken(token) => {
                // No refresh token is available for a bare access token, so
                // the Web API client is built without refresh support.
                self.connect_with_access_token(&token, None, None)
            }
            ServiceCredentials::CachedSession(_) => Err(ServiceError::AuthError(
                "Raw CachedSession bytes are not supported for Spotify; use \
                 SpotifyService::login_with_cached_credentials(cache_dir)"
                    .to_string(),
            )),
            ServiceCredentials::DeviceCode => Err(ServiceError::AuthError(
                "Spotify uses the authorization-code flow, not device codes; \
                 use SpotifyService::login_with_oauth()"
                    .to_string(),
            )),
        }
    }

    fn is_authenticated(&self) -> bool {
        self.session.is_some()
    }

    fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<ServiceTrack>, ServiceError> {
        self.web_api()?.search_tracks(query, limit)
    }

    fn search_albums(&self, query: &str, limit: u32) -> Result<Vec<ServiceAlbum>, ServiceError> {
        self.web_api()?.search_albums(query, limit)
    }

    fn album_tracks(&self, album_id: &str) -> Result<Vec<ServiceTrack>, ServiceError> {
        self.web_api()?.album_tracks(album_id)
    }

    fn start_stream(
        &mut self,
        track_id: &str,
        quality: AudioQuality,
    ) -> Result<ServiceStreamResult, ServiceError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| ServiceError::AuthError("Not authenticated".to_string()))?
            .clone();

        // Stop any previously running player so we don't leak background
        // decoder threads when the caller starts a new track without first
        // calling `stop_stream`.
        if let Some(prev) = self.player.take() {
            prev.stop();
        }

        self.quality = quality;
        // Spotify (via librespot) tops out at Vorbis ~320 kbps; if the caller
        // asked for lossless / hi-res, log the downgrade so the choice is
        // visible rather than silent.
        if matches!(quality, AudioQuality::Lossless | AudioQuality::HiRes) {
            log::warn!(
                "[Spotify] Requested {:?} quality is not available via librespot; \
                 falling back to Vorbis ~320 kbps",
                quality
            );
        }

        // Create the PCM channel. `tx` is moved into the sink builder below
        // (which is `FnOnce`, so it is consumed by `Player::new`) and `rx` is
        // moved into `ChannelReader`. We deliberately do NOT keep a copy of
        // `tx` in `self` — that would prevent the channel from ever closing
        // when librespot drops its sink at end-of-track, leaving the
        // `ChannelReader` blocked on `recv()` forever.
        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(PCM_CHANNEL_CAPACITY);

        // Parse Spotify track URI
        let track_uri = if track_id.starts_with("spotify:track:") {
            track_id.to_string()
        } else {
            format!("spotify:track:{}", track_id)
        };

        let spotify_id = librespot_core::SpotifyId::from_uri(&track_uri)
            .map_err(|e| ServiceError::NotFound(format!("Invalid track URI: {:?}", e)))?;

        let player_config = librespot_playback::config::PlayerConfig {
            bitrate: self.quality_to_bitrate(),
            ..Default::default()
        };

        // librespot 0.6's `Player::new` expects a `FnOnce() -> Box<dyn Sink>`
        // (no arguments). Capture `tx` by move so the sink owns the only
        // remaining sender; once librespot drops the sink at EOF, the channel
        // closes and the reader returns `Ok(0)`.
        let sink_builder = move || -> Box<dyn librespot_playback::audio_backend::Sink> {
            Box::new(ChannelSink::new(tx))
        };

        let player = librespot_playback::player::Player::new(
            player_config,
            session.clone(),
            Box::new(librespot_playback::mixer::NoOpVolume),
            sink_builder,
        );

        // Start playing the track.
        player.load(spotify_id, true, 0);

        log::info!(
            "[Spotify] Streaming track {} at {:?} quality",
            track_id,
            quality
        );

        // Retain the player so we can shut it down in `stop_stream` /
        // `Drop`. Without this the player was previously dropped at the end
        // of `start_stream`, terminating playback (and leaking the background
        // decoder thread it spawned).
        self.player = Some(player);

        let reader = ChannelReader::new(rx);

        Ok(ServiceStreamResult::Pcm(PcmStream {
            sample_rate: 44100, // Spotify always outputs 44.1kHz
            channels: 2,
            // `bits_per_sample` is metadata-only per the trait; samples on the
            // wire are f32. Reflect that.
            bits_per_sample: 32,
            total_frames: None, // Unknown until track metadata is fetched.
            reader: Box::new(reader),
        }))
    }

    fn stop_stream(&mut self) {
        if let Some(player) = self.player.take() {
            player.stop();
            // Dropping the Arc here closes the player's command channel,
            // signalling its background thread to exit.
        }
    }

    fn service_name(&self) -> &str {
        "Spotify"
    }
}

impl Drop for SpotifyService {
    fn drop(&mut self) {
        // Ensure the librespot Player is shut down even if the user forgot to
        // call `stop_stream` — otherwise its background thread keeps running.
        if let Some(player) = self.player.take() {
            player.stop();
        }
    }
}

// ============================================================================
// librespot Sink that captures PCM to a channel
// ============================================================================

struct ChannelSink {
    tx: mpsc::SyncSender<Vec<f32>>,
}

impl ChannelSink {
    fn new(tx: mpsc::SyncSender<Vec<f32>>) -> Self {
        Self { tx }
    }
}

impl librespot_playback::audio_backend::Sink for ChannelSink {
    fn start(&mut self) -> Result<(), librespot_playback::audio_backend::SinkError> {
        Ok(())
    }

    fn stop(&mut self) -> Result<(), librespot_playback::audio_backend::SinkError> {
        Ok(())
    }

    fn write(
        &mut self,
        packet: librespot_playback::decoder::AudioPacket,
        _converter: &mut librespot_playback::convert::Converter,
    ) -> librespot_playback::audio_backend::SinkResult<()> {
        match packet {
            librespot_playback::decoder::AudioPacket::Samples(samples) => {
                // librespot 0.6 delivers normalised f64 interleaved samples in
                // [-1.0, 1.0]. Convert to f32 with a defensive clamp — see
                // `convert_librespot_samples` for the rationale and tests.
                let f32_samples = convert_librespot_samples(&samples);
                // Blocking send to apply backpressure to librespot's decoder.
                // If the receiver has been dropped (stream stopped) ignore.
                let _ = self.tx.send(f32_samples);
                Ok(())
            }
            librespot_playback::decoder::AudioPacket::Raw(_) => {
                // We don't handle raw encoded data; this shouldn't happen with
                // our config (we only use the Vorbis decoder path).
                Ok(())
            }
        }
    }
}

/// Convert a slice of librespot f64 PCM samples (interleaved, range
/// [-1.0, 1.0]) to f32, clamping out-of-range values that may appear after
/// librespot's normalisation / dithering passes.
///
/// Exposed as a free function so it can be unit-tested without needing the
/// librespot stack.
fn convert_librespot_samples(samples: &[f64]) -> Vec<f32> {
    samples
        .iter()
        .map(|&s| (s as f32).clamp(F32_SAMPLE_MIN, F32_SAMPLE_MAX))
        .collect()
}

// ============================================================================
// Reader that consumes PCM from the channel
// ============================================================================

struct ChannelReader {
    rx: mpsc::Receiver<Vec<f32>>,
    current_buf: Vec<u8>,
    pos: usize,
}

impl ChannelReader {
    fn new(rx: mpsc::Receiver<Vec<f32>>) -> Self {
        Self {
            rx,
            current_buf: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Serve from current buffer first
        if self.pos < self.current_buf.len() {
            let available = self.current_buf.len() - self.pos;
            let to_copy = buf.len().min(available);
            buf[..to_copy].copy_from_slice(&self.current_buf[self.pos..self.pos + to_copy]);
            self.pos += to_copy;
            return Ok(to_copy);
        }

        // Wait for next chunk from librespot
        match self.rx.recv() {
            Ok(samples) => {
                // Convert f32 samples to raw bytes (little-endian)
                self.current_buf.clear();
                self.current_buf.reserve(samples.len() * 4);
                for s in &samples {
                    self.current_buf.extend_from_slice(&s.to_le_bytes());
                }
                self.pos = 0;

                let to_copy = buf.len().min(self.current_buf.len());
                buf[..to_copy].copy_from_slice(&self.current_buf[..to_copy]);
                self.pos = to_copy;
                Ok(to_copy)
            }
            Err(_) => Ok(0), // Channel closed = EOF
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_convert_librespot_samples_basic() {
        // f64 in [-1, 1] should map to f32 with the same value.
        let input: Vec<f64> = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let out = convert_librespot_samples(&input);
        assert_eq!(out.len(), input.len());
        for (a, b) in out.iter().zip(input.iter()) {
            assert!((*a as f64 - *b).abs() < 1e-6, "got {a} expected {b}");
        }
    }

    #[test]
    fn test_convert_librespot_samples_clamps_out_of_range() {
        // Slightly over- and under-range f64 values (which the dithering /
        // normalisation passes can occasionally produce) must be clamped to
        // [-1.0, 1.0] before reaching the engine.
        let input: Vec<f64> = vec![1.5, -1.5, 2.0, -2.0, 1.0000001, -1.0000001];
        let out = convert_librespot_samples(&input);
        for s in &out {
            assert!(*s <= 1.0 && *s >= -1.0, "sample {s} out of range");
        }
        assert_eq!(out[0], 1.0);
        assert_eq!(out[1], -1.0);
        assert_eq!(out[2], 1.0);
        assert_eq!(out[3], -1.0);
    }

    #[test]
    fn test_convert_librespot_samples_does_not_divide_by_32768() {
        // This is the bug the review flagged: the original code treated each
        // element as i16 and divided by 32768, which against the actual f64
        // input would produce essentially silence. A sample of 0.5 must map
        // to roughly 0.5, NOT 0.5 / 32768 ≈ 1.5e-5.
        let out = convert_librespot_samples(&[0.5f64]);
        assert!(
            out[0] > 0.49 && out[0] < 0.51,
            "expected ~0.5, got {} — sample format conversion is wrong",
            out[0]
        );
    }

    #[test]
    fn test_channel_reader_eof_when_sender_dropped() {
        // Regression for the EOF-hang bug: dropping the sender must allow
        // `read` to return Ok(0), not block forever. (The fix is to no longer
        // retain `pcm_tx` in the service; here we just verify the reader's
        // contract by dropping the sender directly.)
        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(4);
        // Send one packet then drop the sender.
        tx.send(vec![0.25f32, -0.25]).unwrap();
        drop(tx);

        let mut reader = ChannelReader::new(rx);
        // Read the buffered packet (8 bytes = 2 * f32).
        let mut buf = [0u8; 32];
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 8, "should read both f32 samples as 8 bytes");

        // Next read must return Ok(0) — EOF — not hang.
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 0, "channel was closed; reader must signal EOF");
    }

    #[test]
    fn test_channel_reader_roundtrip_f32_bytes() {
        // f32 samples sent over the channel come back as little-endian bytes.
        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(4);
        let samples = vec![1.0f32, -1.0, 0.5];
        tx.send(samples.clone()).unwrap();
        drop(tx);

        let mut reader = ChannelReader::new(rx);
        let mut buf = vec![0u8; samples.len() * 4];
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, buf.len());

        for (i, s) in samples.iter().enumerate() {
            let bytes: [u8; 4] = buf[i * 4..i * 4 + 4].try_into().unwrap();
            assert_eq!(f32::from_le_bytes(bytes), *s);
        }
    }

    #[test]
    fn authenticate_rejects_username_password_credentials() {
        // Spotify disabled password auth server-side; the trait method must
        // steer callers to the OAuth login instead of attempting a connect.
        let mut service = SpotifyService::new();
        let result = service.authenticate(ServiceCredentials::UsernamePassword {
            username: "alice".to_string(),
            password: "secret".to_string(),
        });
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("login_with_oauth"), "got: {msg}");
            }
            other => panic!("expected AuthError, got: {other:?}"),
        }
    }

    #[test]
    fn authenticate_rejects_empty_username_password() {
        // Regression: this used to panic inside `Session::new`
        // (`Handle::current()` with no runtime). Username/password auth now
        // fails fast with an AuthError directing the user to OAuth, before
        // any librespot session is constructed — no panic, no credential leak.
        let mut service = SpotifyService::new();
        let result = service.authenticate(ServiceCredentials::UsernamePassword {
            username: "".to_string(),
            password: "".to_string(),
        });
        assert!(result.is_err());
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("login_with_oauth"), "got: {msg}");
            }
            other => panic!("expected AuthError, got: {other:?}"),
        }
    }

    #[test]
    fn authenticate_access_token_works_without_ambient_runtime() {
        // Regression: this used to fail fast with "No tokio runtime" (and
        // before that, panic inside `Session::new` on `Handle::current()`).
        // The session work now runs inside the service's own runtime, so the
        // call must proceed to the actual connect — which fails here on the
        // bogus token — instead of erroring on the missing ambient runtime.
        let mut service = SpotifyService::new();
        let result =
            service.authenticate(ServiceCredentials::AccessToken("some-token".to_string()));
        match result {
            Err(ServiceError::Other(msg)) => {
                assert!(!msg.contains("No tokio runtime"), "got: {msg}");
            }
            // A failed connect surfaces as AuthError; either way the runtime
            // pre-check is gone and nothing panicked.
            Err(ServiceError::AuthError(_)) => {}
            other => panic!("expected connect failure, got: {other:?}"),
        }
        assert!(!service.is_authenticated());
    }

    #[test]
    fn authenticate_rejects_cached_session_and_device_code() {
        let mut service = SpotifyService::new();
        let result = service.authenticate(ServiceCredentials::CachedSession(vec![1, 2, 3]));
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("login_with_cached_credentials"), "got: {msg}");
            }
            other => panic!("expected AuthError, got: {other:?}"),
        }

        let result = service.authenticate(ServiceCredentials::DeviceCode);
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("login_with_oauth"), "got: {msg}");
            }
            other => panic!("expected AuthError, got: {other:?}"),
        }
    }

    #[test]
    fn login_with_cached_credentials_returns_false_when_nothing_cached() {
        // A fresh cache dir has no credentials.json; must report false (not
        // error, not panic) and must not require a tokio runtime.
        let dir = std::env::temp_dir().join(format!(
            "sotf-spotify-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut service = SpotifyService::new();
        let result = service.login_with_cached_credentials(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Ok(false) => {}
            other => panic!("expected Ok(false), got: {other:?}"),
        }
        assert!(!service.is_authenticated());
    }

    #[test]
    fn login_with_cached_credentials_works_from_runtime_less_thread() {
        // Regression: the engine's decoder thread is a plain `std::thread`
        // with no tokio runtime, and the old `Handle::try_current()` pre-check
        // made cached login fail there with "No tokio runtime" even though
        // the service owns a runtime it could do the work in. Seed a cache
        // with valid-looking stored credentials and call from a bare thread:
        // the connect must be attempted (and fail on the bogus credentials /
        // no network), not rejected up front.
        let dir = std::env::temp_dir().join(format!(
            "sotf-spotify-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cache = librespot_core::cache::Cache::new(Some(&dir), None, None, None).unwrap();
        cache.save_credentials(&librespot_core::authentication::Credentials {
            username: Some("sotf-test".to_string()),
            auth_type: librespot_protocol::authentication::AuthenticationType::AUTHENTICATION_STORED_SPOTIFY_CREDENTIALS,
            auth_data: b"not-a-real-stored-credential".to_vec(),
        });

        let dir_in_thread = dir.clone();
        let result = std::thread::spawn(move || {
            let mut service = SpotifyService::new();
            service.login_with_cached_credentials(&dir_in_thread)
        })
        .join()
        .expect("login panicked on a runtime-less thread");
        let _ = std::fs::remove_dir_all(&dir);

        match result {
            // The expected outcome: librespot rejected the bogus stored
            // credentials (or the network was unavailable).
            Err(ServiceError::AuthError(_)) | Err(ServiceError::NetworkError(_)) => {}
            Err(ServiceError::Other(msg)) => {
                assert!(!msg.contains("No tokio runtime"), "got: {msg}");
            }
            // A connect that somehow succeeded is fine too — the point is the
            // runtime pre-check is gone.
            _ => {}
        }
    }

    #[test]
    fn search_before_login_is_auth_error() {
        let service = SpotifyService::new();
        match service.search_tracks("anything", 5) {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("no access token"), "got: {msg}");
            }
            other => panic!("expected AuthError, got: {other:?}"),
        }
        assert!(service.search_albums("anything", 5).is_err());
        assert!(service.album_tracks("abc").is_err());
        assert!(service.saved_albums().is_err());
        assert!(service.saved_tracks().is_err());
    }
}
