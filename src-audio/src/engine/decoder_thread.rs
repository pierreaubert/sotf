// ============================================================================
// Decoder Thread - Audio Decoding + Resampling
// ============================================================================
//
// Decodes audio files using Symphonia and resamples if needed.

use super::{AudioFrame, DecoderCommand, DecoderMessage, ThreadEvent};
use crate::decoder::{AudioDecoder, AudioSpec, create_decoder};
use crate::plugins::{Plugin, ProcessContext, ResamplerPlugin};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, SyncSender};

const SPIN_MS_SLEEP_DECODER : u64 = 10;

/// Decoder thread handle
pub struct DecoderThread {
    command_tx: Sender<DecoderCommand>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl DecoderThread {
    /// Create and start the decoder thread
    pub fn new(
        message_tx: SyncSender<DecoderMessage>,
        event_tx: Sender<ThreadEvent>,
        target_sample_rate: u32,
        frame_size: usize,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();

        let thread_handle = std::thread::Builder::new()
            .name("decoder".to_string())
            .spawn(move || {
                if let Err(e) = run_decoder_thread(
                    message_tx,
                    command_rx,
                    event_tx,
                    target_sample_rate,
                    frame_size,
                ) {
                    log::debug!("[Decoder Thread] Error: {}", e);
                }
            })
            .map_err(|e| format!("Failed to spawn decoder thread: {}", e))?;

        Ok(Self {
            command_tx,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the decoder thread
    pub fn send_command(&self, command: DecoderCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// Shutdown the decoder thread
    pub fn shutdown(&mut self) {
        self.send_command(DecoderCommand::Shutdown).ok();
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}

impl Drop for DecoderThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Decoder state
struct DecoderState {
    decoder: Option<Box<dyn AudioDecoder>>,
    resampler: Option<ResamplerPlugin>,
    resampler_buffer: Vec<f32>,
    paused: bool,
    current_file: Option<PathBuf>,
    spec: Option<AudioSpec>,
    silent_source: bool, // For HAL input plugins (no file source)
}

impl DecoderState {
    fn new() -> Self {
        Self {
            decoder: None,
            resampler: None,
            resampler_buffer: Vec::new(),
            paused: false,
            current_file: None,
            spec: None,
            silent_source: false,
        }
    }

    /// Start playing a new file
    fn play(
        &mut self,
        path: PathBuf,
        target_sample_rate: u32,
        frame_size: usize,
    ) -> Result<(), String> {
        // Create decoder
        let decoder =
            create_decoder(&path).map_err(|e| format!("Failed to create decoder: {:?}", e))?;

        // Get audio spec
        let spec = decoder.spec().clone();
        let source_sample_rate = spec.sample_rate;
        let channels = spec.channels as usize;

        log::info!(
            "[Decoder Thread] Playing: {:?} ({}Hz, {}ch)",
            path,
            source_sample_rate,
            channels
        );

        // Create resampler if needed
        let resampler = if source_sample_rate != target_sample_rate {
            log::info!(
                "[Decoder Thread] Resampling: {}Hz -> {}Hz",
                source_sample_rate,
                target_sample_rate
            );
            let rs =
                ResamplerPlugin::new(channels, source_sample_rate, target_sample_rate, frame_size)
                    .map_err(|e| format!("Failed to create resampler: {}", e))?;
            Some(rs)
        } else {
            None
        };

        self.decoder = Some(decoder);
        self.resampler = resampler;
        self.resampler_buffer.clear();
        self.paused = false;
        self.current_file = Some(path);
        self.spec = Some(spec);

        Ok(())
    }

    /// Decode and send chunks
    fn decode_chunk(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        event_tx: &Sender<ThreadEvent>,
        frame_size: usize,
        target_sample_rate: u32,
    ) -> Result<bool, String> {
        let decoder = self.decoder.as_mut().ok_or("No decoder")?;
        let spec = self.spec.as_ref().ok_or("No spec")?;

        // Decode next chunk
        match decoder.decode_next() {
            Ok(Some(decoded)) => {
                let channels = spec.channels as usize;
                let source_sample_rate = spec.sample_rate;

                // Add to resampler buffer if we're resampling
                if let Some(resampler) = &mut self.resampler {
                    self.resampler_buffer.extend_from_slice(&decoded.samples);

                    // Process resampler buffer in frame_size chunks
                    while self.resampler_buffer.len() >= frame_size * channels {
                        let chunk: Vec<f32> = self
                            .resampler_buffer
                            .drain(..frame_size * channels)
                            .collect();

                        // Resample
                        let max_output_frames = resampler.output_frames_for_input(frame_size);
                        let mut output = vec![0.0; max_output_frames * channels];

                        let context = ProcessContext {
                            sample_rate: source_sample_rate,
                            num_frames: frame_size,
                        };

                        resampler
                            .process(&chunk, &mut output, &context)
                            .map_err(|e| format!("Resampling failed: {}", e))?;

                        // Calculate actual output frames
                        let expected_frames =
                            (frame_size as f64 * resampler.ratio()).ceil() as usize;
                        output.truncate(expected_frames * channels);

                        // Send resampled frame
                        let frame =
                            AudioFrame::new(output, expected_frames, channels, target_sample_rate);
                        message_tx
                            .send(DecoderMessage::Frame(frame))
                            .map_err(|_| "Failed to send frame")?;
                    }
                } else {
                    // No resampling - send decoded samples directly as frames
                    let num_frames = decoded.samples.len() / channels;
                    let frame =
                        AudioFrame::new(decoded.samples, num_frames, channels, source_sample_rate);
                    message_tx
                        .send(DecoderMessage::Frame(frame))
                        .map_err(|_| "Failed to send frame")?;
                }

                // Update position
                let position_sec = decoder.position() as f64 / source_sample_rate as f64;
                event_tx
                    .send(ThreadEvent::PositionUpdate(position_sec))
                    .ok();

                Ok(true)
            }
            Ok(None) => {
                // End of stream
                log::debug!("[Decoder Thread] End of stream");

                // Flush remaining resampler buffer
                if let Some(_resampler) = &mut self.resampler
                    && !self.resampler_buffer.is_empty()
                {
                    // Process remaining samples (pad if needed)
                    log::info!(
                        "[Decoder Thread] Flushing {} remaining samples",
                        self.resampler_buffer.len()
                    );
                    // TODO: Properly flush resampler
                }

                message_tx
                    .send(DecoderMessage::EndOfStream)
                    .map_err(|_| "Failed to send EOS")?;
                event_tx.send(ThreadEvent::DecoderEndOfStream).ok();
                Ok(false)
            }
            Err(e) => {
                let err_msg = format!("Decode error: {:?}", e);
                event_tx
                    .send(ThreadEvent::DecoderError(err_msg.clone()))
                    .ok();
                Err(err_msg)
            }
        }
    }

    /// Seek to position in seconds
    fn seek(&mut self, position: f64) -> Result<(), String> {
        if let (Some(decoder), Some(spec)) = (&mut self.decoder, &self.spec) {
            let frame_position = (position * spec.sample_rate as f64) as u64;
            decoder
                .seek(frame_position)
                .map_err(|e| format!("Seek failed: {:?}", e))?;

            // Clear resampler buffer
            self.resampler_buffer.clear();

            // Reset resampler state
            if let Some(resampler) = &mut self.resampler {
                resampler.reset();
            }

            log::info!(
                "[Decoder Thread] Seeked to {:.2}s (frame {})",
                position,
                frame_position
            );
            Ok(())
        } else {
            Err("No decoder".to_string())
        }
    }

    /// Stop and cleanup
    fn stop(&mut self) {
        self.decoder = None;
        self.resampler = None;
        self.resampler_buffer.clear();
        self.current_file = None;
        self.spec = None;
        self.silent_source = false;
    }

    /// Start silent source mode (for HAL input plugins)
    fn start_silent_source(&mut self) {
        self.stop(); // Clear any existing decoder
        self.silent_source = true;
        log::info!("[Decoder Thread] Started silent source mode for HAL input");
    }

    /// Generate a silent frame (0 channels, no data)
    /// Used for HAL input plugins that act as audio sources
    fn generate_silent_frame(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        frame_size: usize,
        sample_rate: u32,
    ) -> Result<(), String> {
        // Send empty frame (0 samples, 0 channels)
        // The HAL input plugin will generate audio from this
        let frame = AudioFrame::new(vec![], frame_size, 0, sample_rate);

        message_tx
            .send(DecoderMessage::Frame(frame))
            .map_err(|_| "Failed to send silent frame")?;

        Ok(())
    }
}

/// Main decoder thread function
fn run_decoder_thread(
    message_tx: SyncSender<DecoderMessage>,
    command_rx: Receiver<DecoderCommand>,
    event_tx: Sender<ThreadEvent>,
    target_sample_rate: u32,
    frame_size: usize,
) -> Result<(), String> {
    let mut state = DecoderState::new();

    log::info!(
        "[Decoder Thread] Started - target {}Hz, frame size {}",
        target_sample_rate,
        frame_size
    );

    loop {
        // Check for commands (non-blocking when playing/silent, blocking when stopped)
        let is_active = (state.decoder.is_some() && !state.paused) || state.silent_source;
        let command = if is_active {
            command_rx.try_recv().ok()
        } else {
            // Blocking wait when stopped/paused
            command_rx.recv().ok()
        };

        if let Some(cmd) = command {
            match cmd {
                DecoderCommand::Play(path) => {
                    state.stop();
                    if let Err(e) = state.play(path, target_sample_rate, frame_size) {
                        log::debug!("[Decoder Thread] Play failed: {}", e);
                        event_tx.send(ThreadEvent::DecoderError(e)).ok();
                    }
                }
                DecoderCommand::StartSilentSource => {
                    state.start_silent_source();
                }
                DecoderCommand::Pause => {
                    state.paused = true;
                    log::debug!("[Decoder Thread] Paused");
                }
                DecoderCommand::Resume => {
                    state.paused = false;
                    log::debug!("[Decoder Thread] Resumed");
                }
                DecoderCommand::Seek(position) => {
                    message_tx.send(DecoderMessage::Flush).ok();
                    if let Err(e) = state.seek(position) {
                        log::debug!("[Decoder Thread] Seek failed: {}", e);
                    }
                }
                DecoderCommand::Stop => {
                    state.stop();
                    message_tx.send(DecoderMessage::Flush).ok();
                    log::debug!("[Decoder Thread] Stopped");
                }
                DecoderCommand::Shutdown => {
                    log::debug!("[Decoder Thread] Shutting down");
                    break;
                }
            }
        }

        // Generate frames based on mode
        if state.silent_source && !state.paused {
            // Silent source mode: generate empty frames for HAL input plugins
            if let Err(e) = state.generate_silent_frame(&message_tx, frame_size, target_sample_rate) {
                log::debug!("[Decoder Thread] Silent frame error: {}", e);
                state.stop();
            }
            // Sleep to maintain target frame rate (e.g., ~21ms for 1024 samples @ 48kHz)
            let frame_duration_ms = (frame_size as f64 / target_sample_rate as f64 * 1000.0) as u64;
            std::thread::sleep(std::time::Duration::from_millis(frame_duration_ms));
        } else if state.decoder.is_some() && !state.paused {
            // File playback mode: decode from file
            match state.decode_chunk(&message_tx, &event_tx, frame_size, target_sample_rate) {
                Ok(true) => {
                    // Continue
                }
                Ok(false) => {
                    // End of stream - stop
                    state.stop();
                }
                Err(e) => {
                    log::debug!("[Decoder Thread] Error: {}", e);
                    state.stop();
                }
            }
        } else {
            // Idle: small sleep to avoid busy loop when paused/stopped
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_DECODER));
        }
    }

    log::debug!("[Decoder Thread] Stopped");
    Ok(())
}
