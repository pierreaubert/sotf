// ============================================================================
// Spotify Integration via librespot
// ============================================================================
//
// Uses librespot to connect to Spotify and decode audio.
// librespot decodes Vorbis/AAC internally and provides raw PCM samples.
// We capture these via a custom Sink and feed them to the engine's decoder thread.

use crate::service::*;
use std::io::Read;
use std::sync::mpsc;

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
}

impl SpotifyService {
    pub fn new() -> Self {
        Self {
            session: None,
            quality: AudioQuality::High,
            player: None,
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
}

impl StreamingService for SpotifyService {
    fn authenticate(&mut self, credentials: ServiceCredentials) -> Result<(), ServiceError> {
        let (username, password) = match credentials {
            ServiceCredentials::UsernamePassword { username, password } => (username, password),
            _ => {
                return Err(ServiceError::AuthError(
                    "Spotify requires UsernamePassword credentials".to_string(),
                ));
            }
        };

        // Create librespot session
        let session_config = librespot_core::SessionConfig::default();
        let credentials =
            librespot_core::authentication::Credentials::with_password(&username, &password);

        let session = librespot_core::Session::new(session_config, None);

        // We need a tokio runtime for librespot's async connection.
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|e| ServiceError::Other(format!("No tokio runtime: {}", e)))?;

        rt.block_on(session.connect(credentials, false))
            .map_err(|e| ServiceError::AuthError(format!("Spotify login failed: {}", e)))?;

        log::info!("[Spotify] Authenticated successfully");
        self.session = Some(session);
        Ok(())
    }

    fn is_authenticated(&self) -> bool {
        self.session.is_some()
    }

    fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<ServiceTrack>, ServiceError> {
        // Spotify search requires the Mercury API via librespot
        // For now, return empty — full implementation needs librespot-metadata
        log::warn!("[Spotify] search_tracks not yet fully implemented");
        let _ = (query, limit);
        Ok(vec![])
    }

    fn search_albums(&self, query: &str, limit: u32) -> Result<Vec<ServiceAlbum>, ServiceError> {
        log::warn!("[Spotify] search_albums not yet fully implemented");
        let _ = (query, limit);
        Ok(vec![])
    }

    fn album_tracks(&self, album_id: &str) -> Result<Vec<ServiceTrack>, ServiceError> {
        log::warn!("[Spotify] album_tracks not yet fully implemented");
        let _ = album_id;
        Ok(vec![])
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
}
