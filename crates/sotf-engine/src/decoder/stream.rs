use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::decoder::core::{AudioSpec, create_decoder};
use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};

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
    /// Handle for the decoder thread
    decoder_thread: Option<JoinHandle<()>>,
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
            decoder_thread: None,
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

        // Spawn decoder thread
        let thread_handle = thread::spawn(move || {
            if let Err(e) = Self::decoder_thread_main(path, config, state, cmd_rx, event_tx) {
                log::debug!("[AudioStream] Decoder thread error: {:?}", e);
            }
        });

        self.command_tx = Some(cmd_tx);
        self.event_rx = Some(event_rx);
        self.decoder_thread = Some(thread_handle);

        // Send initial start command
        self.send_command(StreamCommand::Play)?;

        Ok(())
    }

    /// Stop the audio stream
    pub fn stop(&mut self) -> AudioDecoderResult<()> {
        if let Some(ref cmd_tx) = self.command_tx {
            let _ = cmd_tx.send(StreamCommand::Stop);
        }

        // Wait for thread to finish
        if let Some(handle) = self.decoder_thread.take() {
            let _ = handle.join();
        }

        self.command_tx = None;
        self.event_rx = None;

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

    /// Get current stream state
    pub fn state(&self) -> StreamState {
        self.state.lock().unwrap().clone()
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
        _config: StreamConfig,
        state: Arc<Mutex<StreamState>>,
        cmd_rx: Receiver<StreamCommand>,
        event_tx: Sender<StreamEvent>,
    ) -> AudioDecoderResult<()> {
        log::info!("[AudioStream] Decoder thread starting for: {:?}", path);

        // Create decoder
        let mut decoder = create_decoder(&path)?;
        let spec = decoder.spec().clone();

        let mut playing = false;
        let mut position = 0u64;

        // Set initial state
        {
            let mut state_lock = state.lock().unwrap();
            *state_lock = StreamState::Buffering;
        }
        let _ = event_tx.send(StreamEvent::Started);

        loop {
            // Check for commands
            if let Ok(command) = cmd_rx.try_recv() {
                match command {
                    StreamCommand::Play => {
                        playing = true;
                        {
                            let mut state_lock = state.lock().unwrap();
                            *state_lock = StreamState::Playing;
                        }
                        let _ = event_tx.send(StreamEvent::Playing);
                    }
                    StreamCommand::Pause => {
                        playing = false;
                        {
                            let mut state_lock = state.lock().unwrap();
                            *state_lock = StreamState::Paused;
                        }
                        let _ = event_tx.send(StreamEvent::Paused);
                    }
                    StreamCommand::Stop => {
                        {
                            let mut state_lock = state.lock().unwrap();
                            *state_lock = StreamState::Stopped;
                        }
                        let _ = event_tx.send(StreamEvent::Stopped);
                        break;
                    }
                    StreamCommand::Seek(frame_pos) => {
                        if let Err(e) = decoder.seek(frame_pos) {
                            let _ = event_tx.send(StreamEvent::Error(e));
                        } else {
                            position = frame_pos;
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
                match decoder.decode_next() {
                    Ok(Some(decoded)) => {
                        position = decoded.frame_position;
                        // Here you would send the decoded audio data to CamillaDSP
                        // For now, we just simulate processing time
                        thread::sleep(Duration::from_millis(10));
                    }
                    Ok(None) => {
                        // End of stream
                        let _ = event_tx.send(StreamEvent::EndOfStream);
                        playing = false;
                        {
                            let mut state_lock = state.lock().unwrap();
                            *state_lock = StreamState::Stopped;
                        }
                    }
                    Err(e) => {
                        let _ = event_tx.send(StreamEvent::Error(e));
                        {
                            let mut state_lock = state.lock().unwrap();
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
}
