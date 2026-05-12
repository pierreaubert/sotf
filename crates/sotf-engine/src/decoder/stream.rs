use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::decoder::core::{AudioSpec, DecodedAudio, create_decoder};
use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};

const STREAM_STOP_JOIN_TIMEOUT: Duration = Duration::from_millis(500);

/// Configuration for audio streaming
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Buffer size in frames per chunk
    pub buffer_frames: usize,
    /// Number of buffers to keep in the queue
    pub buffer_count: usize,
    /// Enable seeking support
    pub enable_seeking: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_frames: 4096, // ~93ms at 44.1kHz
            buffer_count: 8,     // ~744ms total buffering
            enable_seeking: true,
        }
    }
}

/// Current position in the audio stream
#[derive(Debug, Clone)]
pub struct StreamPosition {
    /// Current frame position
    pub frame: u64,
    /// Total frames (if known)
    pub total_frames: Option<u64>,
    /// Current time position
    pub time: Duration,
    /// Total duration (if known)
    pub total_duration: Option<Duration>,
}

impl StreamPosition {
    /// Get playback progress as a ratio (0.0 to 1.0)
    pub fn progress_ratio(&self) -> Option<f32> {
        self.total_frames.map(|total| {
            if total == 0 {
                0.0
            } else {
                (self.frame as f32) / (total as f32)
            }
        })
    }

    /// Check if stream has ended
    pub fn is_complete(&self) -> bool {
        if let Some(total) = self.total_frames {
            self.frame >= total
        } else {
            false
        }
    }
}

/// Commands that can be sent to the audio stream
#[derive(Debug, Clone)]
pub enum StreamCommand {
    /// Start playback
    Play,
    /// Pause playback (buffering continues)
    Pause,
    /// Stop playback and reset to beginning
    Stop,
    /// Seek to specific frame position
    Seek(u64),
    /// Get current position
    GetPosition,
}

/// Events emitted by the audio stream
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Stream has started
    Started,
    /// Stream is playing
    Playing,
    /// Stream is paused
    Paused,
    /// Stream has stopped
    Stopped,
    /// Stream position update
    Position(StreamPosition),
    /// Decoded audio chunk ready for consumers
    Audio(DecodedAudio),
    /// End of stream reached
    EndOfStream,
    /// An error occurred
    Error(AudioDecoderError),
    /// Buffering progress (0.0 to 1.0)
    Buffering(f32),
}

/// Audio streaming state
#[derive(Debug, Clone, PartialEq)]
pub enum StreamState {
    Idle,
    Buffering,
    Playing,
    Paused,
    Stopped,
    Error,
}

fn lock_stream_state(state: &Mutex<StreamState>) -> MutexGuard<'_, StreamState> {
    state.lock().unwrap_or_else(|poisoned| {
        log::warn!("[AudioStream] Recovering from poisoned stream state lock");
        poisoned.into_inner()
    })
}

/// High-level audio streaming manager
pub struct AudioStream {
    /// Audio specification
    spec: AudioSpec,
    /// Current streaming state
    state: Arc<Mutex<StreamState>>,
    /// Channel for sending commands to the decoder thread
    command_tx: Option<Sender<StreamCommand>>,
    /// Channel for receiving events from the decoder thread
    event_rx: Option<Receiver<StreamEvent>>,
    /// Channel for returning consumed decoded buffers to the decoder thread
    recycle_tx: Option<Sender<DecodedAudio>>,
    /// Handle for the decoder thread
    decoder_thread: Option<JoinHandle<()>>,
    /// Completion signal from the decoder thread, used to avoid blocking
    /// forever if the decoder is stuck in I/O during shutdown.
    decoder_done_rx: Option<Receiver<()>>,
    /// Stream configuration
    config: StreamConfig,
}

impl AudioStream {
    /// Create a new audio stream for the given file
    pub fn new<P: AsRef<Path>>(path: P, config: StreamConfig) -> AudioDecoderResult<Self> {
        let path = path.as_ref();

        // Probe the file to get specifications
        let decoder = create_decoder(path)?;
        let spec = decoder.spec().clone();

        log::info!(
            "[AudioStream] Created stream: {}Hz, {}ch, {:?} frames",
            spec.sample_rate,
            spec.channels,
            spec.total_frames
        );

        Ok(Self {
            spec,
            state: Arc::new(Mutex::new(StreamState::Idle)),
            command_tx: None,
            event_rx: None,
            recycle_tx: None,
            decoder_thread: None,
            decoder_done_rx: None,
            config,
        })
    }

    /// Start the audio stream
    pub fn start<P: AsRef<Path>>(&mut self, path: P) -> AudioDecoderResult<()> {
        if self.decoder_thread.is_some() {
            return Err(AudioDecoderError::ConfigError(
                "Stream is already running".to_string(),
            ));
        }

        let path = path.as_ref().to_path_buf();
        let config = self.config.clone();
        let state = Arc::clone(&self.state);

        // Create channels for communication with decoder thread
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let (recycle_tx, recycle_rx) = mpsc::channel();
        let (decoder_done_tx, decoder_done_rx) = mpsc::channel();

        // Spawn decoder thread
        let thread_handle = thread::spawn(move || {
            if let Err(e) =
                Self::decoder_thread_main(path, config, state, cmd_rx, event_tx, recycle_rx)
            {
                log::debug!("[AudioStream] Decoder thread error: {:?}", e);
            }
            let _ = decoder_done_tx.send(());
        });

        self.command_tx = Some(cmd_tx);
        self.event_rx = Some(event_rx);
        self.recycle_tx = Some(recycle_tx);
        self.decoder_thread = Some(thread_handle);
        self.decoder_done_rx = Some(decoder_done_rx);

        // Send initial start command
        self.send_command(StreamCommand::Play)?;

        Ok(())
    }

    /// Stop the audio stream
    pub fn stop(&mut self) -> AudioDecoderResult<()> {
        if let Some(ref cmd_tx) = self.command_tx {
            let _ = cmd_tx.send(StreamCommand::Stop);
        }

        if let Some(handle) = self.decoder_thread.take() {
            let completed = self
                .decoder_done_rx
                .take()
                .and_then(|rx| rx.recv_timeout(STREAM_STOP_JOIN_TIMEOUT).ok())
                .is_some();
            if completed {
                let _ = handle.join();
            } else {
                log::warn!(
                    "[AudioStream] Decoder thread did not stop within {:?}; detaching",
                    STREAM_STOP_JOIN_TIMEOUT
                );
            }
        } else {
            self.decoder_done_rx = None;
        }

        self.command_tx = None;
        self.event_rx = None;
        self.recycle_tx = None;

        Ok(())
    }

    /// Send a command to the decoder thread
    pub fn send_command(&self, command: StreamCommand) -> AudioDecoderResult<()> {
        if let Some(ref cmd_tx) = self.command_tx {
            cmd_tx
                .send(command)
                .map_err(|_| AudioDecoderError::StreamEnded)?;
            Ok(())
        } else {
            Err(AudioDecoderError::ConfigError(
                "Stream not started".to_string(),
            ))
        }
    }

    /// Try to receive the next event (non-blocking)
    pub fn try_recv_event(&self) -> Option<StreamEvent> {
        if let Some(ref event_rx) = self.event_rx {
            event_rx.try_recv().ok()
        } else {
            None
        }
    }

    /// Return a decoded audio buffer after the caller has consumed it.
    ///
    /// This lets the decoder thread reuse the allocation on later decode
    /// iterations while preserving the existing owned `StreamEvent::Audio`
    /// API for callers that do not participate in recycling.
    pub fn recycle_decoded_audio(&self, decoded: DecodedAudio) -> AudioDecoderResult<()> {
        if let Some(ref recycle_tx) = self.recycle_tx {
            recycle_tx
                .send(decoded)
                .map_err(|_| AudioDecoderError::StreamEnded)?;
            Ok(())
        } else {
            Err(AudioDecoderError::ConfigError(
                "Stream not started".to_string(),
            ))
        }
    }

    /// Get current stream state
    pub fn state(&self) -> StreamState {
        lock_stream_state(&self.state).clone()
    }

    /// Get audio specification
    pub fn spec(&self) -> &AudioSpec {
        &self.spec
    }

    /// Play/resume playback
    pub fn play(&self) -> AudioDecoderResult<()> {
        self.send_command(StreamCommand::Play)
    }

    /// Pause playback
    pub fn pause(&self) -> AudioDecoderResult<()> {
        self.send_command(StreamCommand::Pause)
    }

    /// Seek to frame position
    pub fn seek(&self, frame_position: u64) -> AudioDecoderResult<()> {
        self.send_command(StreamCommand::Seek(frame_position))
    }

    /// Request current position
    pub fn get_position(&self) -> AudioDecoderResult<()> {
        self.send_command(StreamCommand::GetPosition)
    }

    /// Main decoder thread function
    fn decoder_thread_main(
        path: std::path::PathBuf,
        config: StreamConfig,
        state: Arc<Mutex<StreamState>>,
        cmd_rx: Receiver<StreamCommand>,
        event_tx: Sender<StreamEvent>,
        recycle_rx: Receiver<DecodedAudio>,
    ) -> AudioDecoderResult<()> {
        log::info!("[AudioStream] Decoder thread starting for: {:?}", path);

        // Create decoder
        let mut decoder = create_decoder(&path)?;
        let spec = decoder.spec().clone();

        let mut playing = false;
        let mut position = 0u64;
        let mut recycled_audio = Vec::new();

        // Set initial state
        {
            let mut state_lock = lock_stream_state(&state);
            *state_lock = StreamState::Buffering;
        }
        let _ = event_tx.send(StreamEvent::Started);

        loop {
            Self::drain_recycled_audio_buffers(
                &recycle_rx,
                &mut recycled_audio,
                config.buffer_count,
            );

            // Check for commands
            if let Ok(command) = cmd_rx.try_recv() {
                match command {
                    StreamCommand::Play => {
                        playing = true;
                        {
                            let mut state_lock = lock_stream_state(&state);
                            *state_lock = StreamState::Playing;
                        }
                        let _ = event_tx.send(StreamEvent::Playing);
                    }
                    StreamCommand::Pause => {
                        playing = false;
                        {
                            let mut state_lock = lock_stream_state(&state);
                            *state_lock = StreamState::Paused;
                        }
                        let _ = event_tx.send(StreamEvent::Paused);
                    }
                    StreamCommand::Stop => {
                        {
                            let mut state_lock = lock_stream_state(&state);
                            *state_lock = StreamState::Stopped;
                        }
                        let _ = event_tx.send(StreamEvent::Stopped);
                        break;
                    }
                    StreamCommand::Seek(frame_pos) => {
                        if !config.enable_seeking {
                            let _ =
                                event_tx.send(StreamEvent::Error(AudioDecoderError::SeekFailed(
                                    "Seeking is disabled for this stream".to_string(),
                                )));
                        } else if let Err(e) = decoder.seek(frame_pos) {
                            let _ = event_tx.send(StreamEvent::Error(e));
                        } else {
                            position = decoder.position();
                        }
                    }
                    StreamCommand::GetPosition => {
                        let stream_pos = StreamPosition {
                            frame: position,
                            total_frames: spec.total_frames,
                            time: Duration::from_secs_f64(
                                position as f64 / spec.sample_rate as f64,
                            ),
                            total_duration: spec.duration(),
                        };
                        let _ = event_tx.send(StreamEvent::Position(stream_pos));
                    }
                }
            }

            // Decode next chunk if playing
            if playing {
                let mut decoded = Self::take_decode_buffer(&mut recycled_audio, &spec, &config);
                match decoder.decode_into(&mut decoded) {
                    Ok(frames) if frames > 0 => {
                        position = decoded.frame_position + decoded.frame_count() as u64;
                        if event_tx.send(StreamEvent::Audio(decoded)).is_err() {
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Ok(_) => {
                        recycled_audio.push(decoded);
                        // End of stream
                        let _ = event_tx.send(StreamEvent::EndOfStream);
                        playing = false;
                        {
                            let mut state_lock = lock_stream_state(&state);
                            *state_lock = StreamState::Stopped;
                        }
                    }
                    Err(e) => {
                        let _ = event_tx.send(StreamEvent::Error(e));
                        {
                            let mut state_lock = lock_stream_state(&state);
                            *state_lock = StreamState::Error;
                        }
                        break;
                    }
                }
            } else {
                // Sleep when paused to avoid busy waiting
                thread::sleep(Duration::from_millis(50));
            }
        }

        log::info!("[AudioStream] Decoder thread exiting");
        Ok(())
    }

    fn drain_recycled_audio_buffers(
        recycle_rx: &Receiver<DecodedAudio>,
        recycled_audio: &mut Vec<DecodedAudio>,
        max_buffers: usize,
    ) {
        while let Ok(mut decoded) = recycle_rx.try_recv() {
            decoded.clear();
            if recycled_audio.len() < max_buffers {
                recycled_audio.push(decoded);
            }
        }
    }

    fn take_decode_buffer(
        recycled_audio: &mut Vec<DecodedAudio>,
        spec: &AudioSpec,
        config: &StreamConfig,
    ) -> DecodedAudio {
        let target_samples = config.buffer_frames.saturating_mul(spec.channels as usize);
        let mut decoded = if let Some(mut decoded) = recycled_audio.pop() {
            decoded.clear();
            decoded.spec = spec.clone();
            decoded.frame_position = 0;
            decoded
        } else {
            DecodedAudio::new(spec.clone())
        };
        if decoded.samples.capacity() < target_samples {
            decoded
                .samples
                .reserve(target_samples - decoded.samples.capacity());
        }
        decoded
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert!(config.buffer_frames > 0);
        assert!(config.buffer_count > 0);
        assert!(config.enable_seeking);
    }

    #[test]
    fn test_stream_position() {
        let pos = StreamPosition {
            frame: 44100,
            total_frames: Some(441000),
            time: Duration::from_secs(1),
            total_duration: Some(Duration::from_secs(10)),
        };

        assert_eq!(pos.progress_ratio(), Some(0.1));
        assert!(!pos.is_complete());

        let complete_pos = StreamPosition {
            frame: 441000,
            total_frames: Some(441000),
            time: Duration::from_secs(10),
            total_duration: Some(Duration::from_secs(10)),
        };
        assert!(complete_pos.is_complete());
    }

    #[test]
    fn test_stream_creation_with_nonexistent_file() {
        let config = StreamConfig::default();
        let result = AudioStream::new("nonexistent.flac", config);
        assert!(result.is_err());
    }

    fn create_test_wav(path: &std::path::Path) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..128 {
            writer.write_sample(0.25_f32).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn decoder_thread_emits_decoded_audio_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stream.wav");
        create_test_wav(&path);

        let state = Arc::new(Mutex::new(StreamState::Idle));
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let (_recycle_tx, recycle_rx) = mpsc::channel();
        cmd_tx.send(StreamCommand::Play).unwrap();
        cmd_tx.send(StreamCommand::Stop).unwrap();

        AudioStream::decoder_thread_main(
            path,
            StreamConfig::default(),
            state,
            cmd_rx,
            event_tx,
            recycle_rx,
        )
        .unwrap();

        let mut saw_audio = false;
        while let Ok(event) = event_rx.try_recv() {
            if let StreamEvent::Audio(audio) = event {
                saw_audio = true;
                assert_eq!(audio.spec.channels, 1);
                assert!(!audio.samples.is_empty());
            }
        }

        assert!(saw_audio);
    }

    #[test]
    fn decoder_thread_reuses_recycled_audio_buffers() {
        let spec = AudioSpec {
            sample_rate: 48_000,
            channels: 2,
            bits_per_sample: 32,
            total_frames: None,
        };
        let (recycle_tx, recycle_rx) = mpsc::channel();
        let mut decoded = DecodedAudio::new(spec.clone());
        decoded.samples.reserve(256);
        let ptr = decoded.samples.as_ptr();
        recycle_tx.send(decoded).unwrap();

        let mut recycled_audio = Vec::new();
        let config = StreamConfig {
            buffer_frames: 64,
            buffer_count: 1,
            enable_seeking: true,
        };
        AudioStream::drain_recycled_audio_buffers(
            &recycle_rx,
            &mut recycled_audio,
            config.buffer_count,
        );
        let reused = AudioStream::take_decode_buffer(&mut recycled_audio, &spec, &config);

        assert_eq!(reused.samples.as_ptr(), ptr);
        assert!(reused.samples.capacity() >= 256);
    }

    #[test]
    fn decoder_thread_caps_recycled_audio_buffers_from_config() {
        let spec = AudioSpec {
            sample_rate: 48_000,
            channels: 2,
            bits_per_sample: 32,
            total_frames: None,
        };
        let (recycle_tx, recycle_rx) = mpsc::channel();
        for _ in 0..3 {
            recycle_tx.send(DecodedAudio::new(spec.clone())).unwrap();
        }

        let mut recycled_audio = Vec::new();
        AudioStream::drain_recycled_audio_buffers(&recycle_rx, &mut recycled_audio, 2);

        assert_eq!(recycled_audio.len(), 2);
    }

    #[test]
    fn take_decode_buffer_reserves_configured_frame_capacity() {
        let spec = AudioSpec {
            sample_rate: 48_000,
            channels: 2,
            bits_per_sample: 32,
            total_frames: None,
        };
        let config = StreamConfig {
            buffer_frames: 512,
            buffer_count: 1,
            enable_seeking: true,
        };
        let mut recycled_audio = Vec::new();

        let decoded = AudioStream::take_decode_buffer(&mut recycled_audio, &spec, &config);

        assert!(decoded.samples.capacity() >= 1024);
    }

    #[test]
    fn decoder_thread_rejects_seek_when_seeking_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stream.wav");
        create_test_wav(&path);

        let state = Arc::new(Mutex::new(StreamState::Idle));
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let (_recycle_tx, recycle_rx) = mpsc::channel();
        cmd_tx.send(StreamCommand::Play).unwrap();
        cmd_tx.send(StreamCommand::Seek(64)).unwrap();
        cmd_tx.send(StreamCommand::Stop).unwrap();

        AudioStream::decoder_thread_main(
            path,
            StreamConfig {
                buffer_frames: 128,
                buffer_count: 2,
                enable_seeking: false,
            },
            state,
            cmd_rx,
            event_tx,
            recycle_rx,
        )
        .unwrap();

        let mut saw_seek_disabled_error = false;
        while let Ok(event) = event_rx.try_recv() {
            if let StreamEvent::Error(AudioDecoderError::SeekFailed(message)) = event {
                saw_seek_disabled_error = message.contains("disabled");
            }
        }

        assert!(saw_seek_disabled_error);
    }

    #[test]
    fn stream_state_lock_recovers_after_poison() {
        let state = Arc::new(Mutex::new(StreamState::Idle));
        let poisoned = Arc::clone(&state);

        let _ = std::thread::spawn(move || {
            let mut guard = poisoned.lock().unwrap();
            *guard = StreamState::Playing;
            panic!("poison stream state for regression test");
        })
        .join();

        assert_eq!(*lock_stream_state(&state), StreamState::Playing);
    }
}
