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

/// Buffer size for PCM samples between librespot and the decoder thread.
const PCM_CHANNEL_CAPACITY: usize = 48000 * 2 * 4; // ~1 second of stereo f32

pub struct SpotifyService {
    session: Option<librespot_core::Session>,
    quality: AudioQuality,
    /// Sender end of the PCM pipe (librespot's Sink writes here)
    pcm_tx: Option<mpsc::SyncSender<Vec<f32>>>,
    /// Receiver end (wrapped in PcmStream for the decoder thread)
    pcm_rx: Option<mpsc::Receiver<Vec<f32>>>,
    player_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SpotifyService {
    pub fn new() -> Self {
        Self {
            session: None,
            quality: AudioQuality::High,
            pcm_tx: None,
            pcm_rx: None,
            player_handle: None,
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

        // We need a tokio runtime for librespot's async session
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|e| ServiceError::Other(format!("No tokio runtime: {}", e)))?;

        let session = rt.block_on(librespot_core::Session::new(session_config, None));

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

        self.quality = quality;

        // Create PCM channel
        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(16);
        self.pcm_tx = Some(tx.clone());

        // Parse Spotify track URI
        let track_uri = if track_id.starts_with("spotify:track:") {
            track_id.to_string()
        } else {
            format!("spotify:track:{}", track_id)
        };

        let spotify_id = librespot_core::SpotifyId::from_uri(&track_uri)
            .map_err(|e| ServiceError::NotFound(format!("Invalid track URI: {:?}", e)))?;

        // Create librespot player with our PCM-capturing sink
        let player_config = librespot_playback::config::PlayerConfig {
            bitrate: self.quality_to_bitrate(),
            ..Default::default()
        };

        let backend = move |_device: Option<String>,
                            _format: librespot_playback::config::AudioFormat|
              -> Box<dyn librespot_playback::audio_backend::Sink> {
            Box::new(ChannelSink::new(tx.clone()))
        };

        let (player, _event_rx) = librespot_playback::player::Player::new(
            player_config,
            session.clone(),
            Box::new(librespot_playback::mixer::NoOpVolume),
            backend,
        );

        // Start playing the track
        player.load(spotify_id, true, 0);

        log::info!(
            "[Spotify] Streaming track {} at {:?} quality",
            track_id,
            quality
        );

        // Create PcmStream that reads from the channel
        let reader = ChannelReader::new(rx);
        self.pcm_rx = None; // Moved into reader

        Ok(ServiceStreamResult::Pcm(PcmStream {
            sample_rate: 44100, // Spotify always outputs 44.1kHz
            channels: 2,
            bits_per_sample: 16,
            total_frames: None, // Unknown until track metadata is fetched
            reader: Box::new(reader),
        }))
    }

    fn stop_stream(&mut self) {
        self.pcm_tx = None;
        self.pcm_rx = None;
        if let Some(handle) = self.player_handle.take() {
            handle.abort();
        }
    }

    fn service_name(&self) -> &str {
        "Spotify"
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
        packet: librespot_playback::audio_backend::SinkResult<
            librespot_playback::decoder::AudioPacket,
        >,
    ) -> Result<(), librespot_playback::audio_backend::SinkError> {
        match packet {
            Ok(librespot_playback::decoder::AudioPacket::Samples(samples)) => {
                // Convert i16 samples to f32
                let f32_samples: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
                // Blocking send to apply backpressure to librespot's decoder.
                // If the receiver is dropped, the stream has stopped — ignore the error.
                let _ = self.tx.send(f32_samples);
                Ok(())
            }
            Ok(librespot_playback::decoder::AudioPacket::OggData(_)) => {
                // We don't handle raw OGG; this shouldn't happen with our config
                Ok(())
            }
            Err(_) => Ok(()),
        }
    }
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
