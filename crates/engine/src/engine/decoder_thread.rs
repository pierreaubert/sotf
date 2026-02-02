// ============================================================================
// Decoder Thread - Audio Decoding + Resampling
// ============================================================================
//
// Decodes audio files using Symphonia and resamples if needed.

use super::{AudioFrame, DecoderCommand, DecoderMessage, ThreadEvent};
use crate::decoder::{AudioDecoder, AudioSpec, DecodedAudio, create_decoder};
use sotf_plugins::{Plugin, ProcessContext, ResamplerPlugin};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::time::{Duration, Instant};

#[cfg(all(target_os = "macos", feature = "hal"))]
use driver_hal::HalInputReader;

const SPIN_MS_SLEEP_DECODER: u64 = 10;

/// Action returned by decode loop
enum DecoderLoopAction {
    Continue,
    Stop,
    Interrupted(DecoderCommand),
}

/// Helper to send a message with backpressure handling and interruption support
fn send_or_interrupt<T>(
    tx: &SyncSender<T>,
    rx: &Receiver<DecoderCommand>,
    mut msg: T,
) -> Result<Option<DecoderCommand>, String> {
    loop {
        match tx.try_send(msg) {
            Ok(_) => return Ok(None),
            Err(std::sync::mpsc::TrySendError::Full(returned_msg)) => {
                // Buffer full - check for interruption
                if let Ok(cmd) = rx.try_recv() {
                    return Ok(Some(cmd));
                }
                msg = returned_msg;
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => return Err(format!("Channel disconnected: {}", e)),
        }
    }
}

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
                    log::error!("[Decoder Thread] Error: {}", e);
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
    decode_buffer: Option<DecodedAudio>,
    resample_output_buffer: Vec<f32>,

    #[cfg(all(target_os = "macos", feature = "hal"))]
    hal_input_buffer: Vec<f32>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    hal_reader: Option<HalInputReader>,
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
            decode_buffer: None,
            resample_output_buffer: Vec::new(),
            #[cfg(all(target_os = "macos", feature = "hal"))]
            hal_input_buffer: Vec::new(),
            #[cfg(all(target_os = "macos", feature = "hal"))]
            hal_reader: None,
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

        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            self.hal_reader = None;
        }

        Ok(())
    }

    /// Decode and send chunks
    fn decode_chunk(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        command_rx: &Receiver<DecoderCommand>,
        event_tx: &Sender<ThreadEvent>,
        frame_size: usize,
        target_sample_rate: u32,
    ) -> Result<DecoderLoopAction, String> {
        let decoder = self.decoder.as_mut().ok_or("No decoder")?;
        let spec = self.spec.as_ref().ok_or("No spec")?;

        // Use internal buffer for decoding
        if self.decode_buffer.is_none() {
            self.decode_buffer = Some(DecodedAudio::new(spec.clone()));
        }
        let decode_buffer = self.decode_buffer.as_mut().unwrap();

        let decode_start = Instant::now();
        // Decode next chunk
        match decoder.decode_into(decode_buffer) {
            Ok(frames_decoded) if frames_decoded > 0 => {
                let decode_time = decode_start.elapsed();
                log::trace!("[Decoder Thread] Decode time: {:?}", decode_time);

                let channels = spec.channels as usize;
                let source_sample_rate = spec.sample_rate;

                let mut total_resample_time = Duration::ZERO;
                let mut total_send_time = Duration::ZERO;

                // Add to buffer (reusing resampler_buffer as general sample buffer)
                self.resampler_buffer
                    .extend_from_slice(&decode_buffer.samples);

                // Process buffer in frame_size chunks
                while self.resampler_buffer.len() >= frame_size * channels {
                    // If resampling, we need enough input samples for one output frame
                    // But here we simplify: just process fixed input chunks

                    let s_start = Instant::now();
                    let frame_to_send = if let Some(resampler) = &mut self.resampler {
                        let chunk: Vec<f32> = self
                            .resampler_buffer
                            .drain(..frame_size * channels)
                            .collect();

                        // Resample
                        let max_output_frames = resampler.output_frames_for_input(frame_size);
                        let output_len = max_output_frames * channels;

                        if self.resample_output_buffer.len() != output_len {
                            self.resample_output_buffer.resize(output_len, 0.0);
                        }

                        let context = ProcessContext {
                            sample_rate: source_sample_rate,
                            num_frames: frame_size,
                        };

                        let r_start = Instant::now();
                        resampler
                            .process(&chunk, &mut self.resample_output_buffer, &context)
                            .map_err(|e| format!("Resampling failed: {}", e))?;
                        total_resample_time += r_start.elapsed();

                        // Calculate actual output frames
                        let expected_frames =
                            (frame_size as f64 * resampler.ratio()).ceil() as usize;

                        // Send resampled frame - cloning from reusable buffer
                        let frame_data =
                            self.resample_output_buffer[..expected_frames * channels].to_vec();

                        AudioFrame::new(frame_data, expected_frames, channels, target_sample_rate)
                    } else {
                        // No resampling - just take a chunk
                        let chunk: Vec<f32> = self
                            .resampler_buffer
                            .drain(..frame_size * channels)
                            .collect();

                        AudioFrame::new(chunk, frame_size, channels, source_sample_rate)
                    };

                    // Send with interruption support
                    if let Some(cmd) = send_or_interrupt(
                        message_tx,
                        command_rx,
                        DecoderMessage::Frame(frame_to_send),
                    )? {
                        return Ok(DecoderLoopAction::Interrupted(cmd));
                    }
                    total_send_time += s_start.elapsed();
                }

                let processing_time = decode_time + total_resample_time;

                // Warn if actual processing is slow
                if processing_time > Duration::from_millis(10) {
                    log::warn!(
                        "[Decoder Thread] Slow processing: {:?} (Decode: {:?}, Resample: {:?})",
                        processing_time,
                        decode_time,
                        total_resample_time
                    );
                }

                // Update position
                let position_sec = decoder.position() as f64 / source_sample_rate as f64;
                event_tx
                    .send(ThreadEvent::PositionUpdate(position_sec))
                    .ok();

                Ok(DecoderLoopAction::Continue)
            }
            Ok(0) => {
                // End of stream
                log::debug!("[Decoder Thread] End of stream");

                // Flush remaining resampler buffer
                if let Some(resampler) = &mut self.resampler
                    && !self.resampler_buffer.is_empty()
                {
                    let channels = spec.channels as usize;
                    let source_sample_rate = spec.sample_rate;
                    let remaining_frames = self.resampler_buffer.len() / channels;

                    log::info!(
                        "[Decoder Thread] Flushing {} remaining samples ({} frames)",
                        self.resampler_buffer.len(),
                        remaining_frames
                    );

                    // Pad remaining samples to frame_size for resampler
                    let mut padded_chunk = self.resampler_buffer.clone();
                    padded_chunk.resize(frame_size * channels, 0.0);

                    let max_output_frames = resampler.output_frames_for_input(frame_size);
                    let output_len = max_output_frames * channels;

                    if self.resample_output_buffer.len() != output_len {
                        self.resample_output_buffer.resize(output_len, 0.0);
                    }

                    let context = ProcessContext {
                        sample_rate: source_sample_rate,
                        num_frames: frame_size,
                    };

                    // Process padded chunk to flush resampler state
                    if resampler
                        .process(&padded_chunk, &mut self.resample_output_buffer, &context)
                        .is_ok()
                    {
                        // Calculate actual output frames (may be more due to the resampling ratio)
                        let expected_frames =
                            (frame_size as f64 * resampler.ratio()).ceil() as usize;

                        if expected_frames > 0 {
                            let frame_data =
                                self.resample_output_buffer[..expected_frames * channels].to_vec();
                            let frame = AudioFrame::new(
                                frame_data,
                                expected_frames,
                                channels,
                                target_sample_rate,
                            );

                            // Send with interruption support
                            if let Some(cmd) = send_or_interrupt(
                                message_tx,
                                command_rx,
                                DecoderMessage::Frame(frame),
                            )? {
                                return Ok(DecoderLoopAction::Interrupted(cmd));
                            }

                            log::debug!(
                                "[Decoder Thread] Flushed {} frames through resampler",
                                expected_frames
                            );
                        }
                    } else {
                        log::warn!("[Decoder Thread] Failed to flush resampler");
                    }

                    self.resampler_buffer.clear();
                }

                if let Some(cmd) =
                    send_or_interrupt(message_tx, command_rx, DecoderMessage::EndOfStream)?
                {
                    return Ok(DecoderLoopAction::Interrupted(cmd));
                }

                event_tx.send(ThreadEvent::DecoderEndOfStream).ok();
                Ok(DecoderLoopAction::Stop)
            }
            Err(e) => {
                let err_msg = format!("Decode error: {:?}", e);
                event_tx
                    .send(ThreadEvent::DecoderError(err_msg.clone()))
                    .ok();
                Err(err_msg)
            }
            Ok(_) => unreachable!("decode_into returned negative frames?"),
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
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            self.hal_reader = None;
        }
    }

    /// Start silent source mode (for HAL input plugins)
    fn start_silent_source(&mut self) {
        self.stop(); // Clear any existing decoder
        self.silent_source = true;

        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            self.hal_reader = HalInputReader::new();
            if self.hal_reader.is_some() {
                log::info!("[Decoder Thread] Started HAL input reader");
            } else {
                log::warn!("[Decoder Thread] Failed to initialize HAL input reader");
            }
        }
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        log::info!("[Decoder Thread] Started silent source mode (HAL input not supported)");
    }

    /// Read from HAL input or generate silent frame
    fn process_hal_input(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        frame_size: usize,
        sample_rate: u32,
    ) -> Result<(), String> {
        // Static counter for periodic logging (avoid log spam)
        static LOG_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let count = LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        #[cfg(all(target_os = "macos", feature = "hal"))]
        if let Some(reader) = &mut self.hal_reader {
            // Read from HAL
            // HAL usually provides 2 channels
            let channels = 2;
            let buffer_len = frame_size * channels;

            if self.hal_input_buffer.len() != buffer_len {
                self.hal_input_buffer.resize(buffer_len, 0.0);
            }

            let samples_read = reader.read(&mut self.hal_input_buffer);

            // Calculate RMS for audio level detection
            let rms: f32 = if samples_read > 0 {
                let sum: f32 = self.hal_input_buffer[..samples_read]
                    .iter()
                    .map(|s| s * s)
                    .sum();
                (sum / samples_read as f32).sqrt()
            } else {
                0.0
            };

            // Log every 100 frames (~2 seconds at 48kHz/1024 frames)
            if count % 100 == 0 {
                let has_audio = rms > 0.0001;
                log::info!(
                    "[AUDIO FLOW] Decoder HAL read: {} samples, RMS={:.6}, has_audio={}, connected={}",
                    samples_read,
                    rms,
                    has_audio,
                    reader.is_connected()
                );
            }

            if samples_read < buffer_len {
                // Zero-fill remaining
                self.hal_input_buffer[samples_read..].fill(0.0);
            }

            let frame = AudioFrame::new(
                self.hal_input_buffer.clone(),
                frame_size,
                channels,
                sample_rate,
            );
            message_tx
                .send(DecoderMessage::Frame(frame))
                .map_err(|_| "Failed to send HAL frame")?;

            return Ok(());
        }

        // Log that we don't have a HAL reader
        if count % 100 == 0 {
            log::warn!("[AUDIO FLOW] Decoder: No HAL reader available, sending silent frames");
        }

        // Fallback to silent frame if no reader (or not macOS)
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
                    } else {
                        event_tx.send(ThreadEvent::SeekComplete).ok();
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
            // HAL Input / Silent Source mode
            if let Err(e) = state.process_hal_input(&message_tx, frame_size, target_sample_rate) {
                log::debug!("[Decoder Thread] HAL input error: {}", e);
                state.stop();
            }
            // For HAL input: only sleep briefly when no data is available to avoid busy-looping.
            // The HAL driver provides data in real-time, so we should consume it as fast as possible.
            // Don't sleep after successful reads - the processing/playback pipeline provides backpressure.
            #[cfg(all(target_os = "macos", feature = "hal"))]
            {
                // If HAL reader got no data, sleep briefly to avoid busy loop
                // The channel send to processing thread provides natural backpressure when data is available
                if state.hal_reader.as_ref().map_or(true, |r| !r.is_connected()) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                // When connected and data flowing, the mpsc channel backpressure handles timing
            }
            #[cfg(not(all(target_os = "macos", feature = "hal")))]
            {
                // Non-HAL silent source mode: sleep to maintain frame rate
                let frame_duration_ms = (frame_size as f64 / target_sample_rate as f64 * 1000.0) as u64;
                std::thread::sleep(std::time::Duration::from_millis(frame_duration_ms));
            }
        } else if state.decoder.is_some() && !state.paused {
            // File playback mode: decode from file
            match state.decode_chunk(
                &message_tx,
                &command_rx,
                &event_tx,
                frame_size,
                target_sample_rate,
            ) {
                Ok(DecoderLoopAction::Continue) => {
                    // Continue
                }
                Ok(DecoderLoopAction::Stop) => {
                    // End of stream - stop
                    state.stop();
                }
                Ok(DecoderLoopAction::Interrupted(cmd)) => {
                    // Handle interruption command immediately
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
                            } else {
                                event_tx.send(ThreadEvent::SeekComplete).ok();
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

    // log::debug!("[Decoder Thread] Stopped");
    Ok(())
}
