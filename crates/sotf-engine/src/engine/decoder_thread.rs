// ============================================================================
// Decoder Thread - Audio Decoding + Resampling
// ============================================================================
//
// Decodes audio files using Symphonia and resamples if needed.

use super::{AudioFrame, DecoderCommand, DecoderMessage, DecoderResponse, ThreadEvent};
use crate::decoder::{AudioDecoder, AudioSource, AudioSpec, DecodedAudio, create_decoder_from_source};
use sotf_plugins::{Plugin, ProcessContext, ResamplerPlugin};
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::time::{Duration, Instant};

#[cfg(all(target_os = "macos", feature = "hal"))]
use driver_hal::HalInputReader;

const SPIN_MS_SLEEP_DECODER: u64 = 1;
/// Maximum size of resample staging buffer to prevent unbounded growth.
/// This limits memory usage while ensuring we can handle typical resampling
/// ratios (e.g., 48kHz→44.1kHz produces ~940-frame blocks, so we allow ~4x that).
const MAX_RESAMPLE_STAGING_SAMPLES: usize = 1024 * 8 * 4; // 1024 frames * 8 channels * 4x margin

/// Action returned by decode loop
enum DecoderLoopAction {
    Continue,
    Stop,
    Interrupted(DecoderCommand),
}

/// Take frame_send_buffer for sending, then restore it from a recycled
/// Vec (zero alloc in steady state) or fall back to Vec::with_capacity.
fn take_frame_buffer(
    frame_send_buffer: &mut Vec<f32>,
    recycle_rx: &Receiver<Vec<f32>>,
    len: usize,
) -> Vec<f32> {
    let mut frame_data = std::mem::take(frame_send_buffer);
    frame_data.truncate(len);

    *frame_send_buffer = match recycle_rx.try_recv() {
        Ok(mut v) => {
            v.clear();
            v
        }
        Err(_) => Vec::with_capacity(len),
    };

    frame_data
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
                // Sleep 5ms instead of 1ms to reduce CPU wakeups
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(format!("Channel disconnected: {}", e)),
        }
    }
}

/// Decoder thread handle
pub struct DecoderThread {
    command_tx: Sender<DecoderCommand>,
    response_rx: Receiver<DecoderResponse>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl DecoderThread {
    /// Create and start the decoder thread
    pub fn new(
        message_tx: SyncSender<DecoderMessage>,
        event_tx: Sender<ThreadEvent>,
        target_sample_rate: u32,
        frame_size: usize,
        recycle_rx: Receiver<Vec<f32>>,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let (response_tx, response_rx) = std::sync::mpsc::channel();

        let thread_handle = std::thread::Builder::new()
            .name("decoder".to_string())
            .spawn(move || {
                if let Err(e) = run_decoder_thread(
                    message_tx,
                    command_rx,
                    response_tx,
                    event_tx,
                    target_sample_rate,
                    frame_size,
                    recycle_rx,
                ) {
                    log::error!("[Decoder Thread] Error: {}", e);
                }
            })
            .map_err(|e| format!("Failed to spawn decoder thread: {}", e))?;

        Ok(Self {
            command_tx,
            response_rx,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the decoder thread
    pub fn send_command(&self, command: DecoderCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    pub fn try_recv_response(&self) -> Option<DecoderResponse> {
        self.response_rx.try_recv().ok()
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
    current_source: Option<AudioSource>,
    spec: Option<AudioSpec>,
    silent_source: bool, // For HAL input plugins (no file source)
    decode_buffer: Option<DecodedAudio>,
    resample_output_buffer: Vec<f32>,
    /// Staging buffer: accumulates resampled output across decode chunks so that
    /// only complete `frame_size`-frame blocks are forwarded to the processing thread.
    /// Without this, a 48 kHz source resampled to 44.1 kHz produces ~940-frame blocks
    /// which are smaller than the upmixer's hop size (1024), causing the upmixer to
    /// never fire an FFT block and produce silence.
    resample_staging: Vec<f32>,
    /// Pre-allocated buffer for chunk processing (avoids allocation in hot path)
    chunk_buffer: Vec<f32>,
    /// Pre-allocated buffer for frame sending (avoids allocation in hot path)
    frame_send_buffer: Vec<f32>,
    /// Receives recycled Vec<f32> buffers from the processing thread
    recycle_rx: Receiver<Vec<f32>>,
    /// Queued next source for gapless playback. When set and the current source ends,
    /// the decoder seamlessly transitions to this source without sending EndOfStream/Flush.
    queued_next: Option<AudioSource>,

    #[cfg(all(target_os = "macos", feature = "hal"))]
    hal_input_buffer: Vec<f32>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    hal_reader: Option<HalInputReader>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    last_hal_sample_rate: Option<u32>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    last_hal_channels: Option<usize>,
}

impl DecoderState {
    fn new(recycle_rx: Receiver<Vec<f32>>) -> Self {
        Self {
            decoder: None,
            resampler: None,
            resampler_buffer: Vec::new(),
            paused: false,
            current_source: None,
            spec: None,
            silent_source: false,
            decode_buffer: None,
            resample_output_buffer: Vec::new(),
            resample_staging: Vec::new(),
            // Pre-allocate for typical frame size (1024 frames * 8 channels)
            chunk_buffer: Vec::with_capacity(1024 * 8),
            frame_send_buffer: Vec::with_capacity(1024 * 8),
            recycle_rx,
            queued_next: None,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            hal_input_buffer: Vec::new(),
            #[cfg(all(target_os = "macos", feature = "hal"))]
            hal_reader: None,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            last_hal_sample_rate: None,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            last_hal_channels: None,
        }
    }

    /// Start playing a new audio source
    fn play(
        &mut self,
        source: AudioSource,
        target_sample_rate: u32,
        frame_size: usize,
    ) -> Result<(), String> {
        let decoder = create_decoder_from_source(&source)
            .map_err(|e| format!("Failed to create decoder: {:?}", e))?;

        // Get audio spec
        let spec = decoder.spec().clone();
        let source_sample_rate = spec.sample_rate;
        let channels = spec.channels as usize;

        log::info!(
            "[Decoder Thread] Playing: {} ({}Hz, {}ch)",
            source.display_name(),
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
        self.resample_staging.clear();
        self.paused = false;
        self.current_source = Some(source);
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

        // Decode next chunk
        match decoder.decode_into(decode_buffer) {
            Ok(frames_decoded) if frames_decoded > 0 => {
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
                    let chunk_len = frame_size * channels;

                    if let Some(resampler) = &mut self.resampler {
                        // Copy chunk to pre-allocated buffer (avoids drain().collect() allocation)
                        if self.chunk_buffer.len() < chunk_len {
                            self.chunk_buffer.resize(chunk_len, 0.0);
                        }
                        self.chunk_buffer[..chunk_len]
                            .copy_from_slice(&self.resampler_buffer[..chunk_len]);
                        self.resampler_buffer.drain(..chunk_len);

                        // Resample
                        let max_output_frames = resampler.output_frames_for_input(frame_size);
                        let output_len = max_output_frames * channels;

                        if self.resample_output_buffer.len() < output_len {
                            self.resample_output_buffer.resize(output_len, 0.0);
                        }

                        let context = ProcessContext {
                            sample_rate: source_sample_rate,
                            num_frames: frame_size,
                        };

                        let r_start = Instant::now();
                        let actual_output_frames = resampler
                            .process(
                                &self.chunk_buffer[..chunk_len],
                                &mut self.resample_output_buffer,
                                &context,
                            )
                            .map_err(|e| format!("Resampling failed: {}", e))?;
                        total_resample_time += r_start.elapsed();

                        // Accumulate resampled output into staging buffer.
                        // The resampler produces a variable number of output frames per
                        // input chunk (e.g. ~940 frames for 48 kHz → 44.1 kHz with a
                        // 1024-frame input). If we forwarded those ~940-frame blocks
                        // directly, plugins with a hop size ≥ frame_size (like the
                        // upmixer at hop=1024) would never accumulate enough input to
                        // fire an FFT block and would produce silence. Staging here
                        // ensures we always forward exactly `frame_size` frames.
                        let frame_len = actual_output_frames * channels;
                        let new_staging_len = self.resample_staging.len() + frame_len;
                        if new_staging_len > MAX_RESAMPLE_STAGING_SAMPLES {
                            // Staging buffer growing too large — drain the oldest
                            // complete-frame-sized blocks to stay under the cap
                            // while preserving any partial frame at the tail.
                            let send_chunk_len_here = frame_size * channels;
                            let excess = new_staging_len - MAX_RESAMPLE_STAGING_SAMPLES;
                            // Round up to whole frame chunks to keep alignment
                            let drain_amount = if send_chunk_len_here > 0 {
                                excess.div_ceil(send_chunk_len_here)
                                    * send_chunk_len_here
                            } else {
                                excess
                            };
                            let drain_amount = drain_amount.min(self.resample_staging.len());
                            log::warn!(
                                "[Decoder Thread] Resample staging buffer would exceed {} samples (current: {}), draining {} oldest samples",
                                MAX_RESAMPLE_STAGING_SAMPLES,
                                self.resample_staging.len(),
                                drain_amount,
                            );
                            self.resample_staging.drain(..drain_amount);
                        }
                        self.resample_staging
                            .extend_from_slice(&self.resample_output_buffer[..frame_len]);

                        // Emit as many complete frame_size blocks as are available.
                        let send_chunk_len = frame_size * channels;
                        while self.resample_staging.len() >= send_chunk_len {
                            if self.frame_send_buffer.len() < send_chunk_len {
                                self.frame_send_buffer.resize(send_chunk_len, 0.0);
                            }
                            self.frame_send_buffer[..send_chunk_len]
                                .copy_from_slice(&self.resample_staging[..send_chunk_len]);
                            self.resample_staging.drain(..send_chunk_len);

                            let frame_data = take_frame_buffer(
                                &mut self.frame_send_buffer,
                                &self.recycle_rx,
                                send_chunk_len,
                            );

                            let frame = AudioFrame::new(
                                frame_data,
                                frame_size,
                                channels,
                                target_sample_rate,
                            );

                            let s_inner = Instant::now();
                            if let Some(cmd) = send_or_interrupt(
                                message_tx,
                                command_rx,
                                DecoderMessage::Frame(frame),
                            )? {
                                return Ok(DecoderLoopAction::Interrupted(cmd));
                            }
                            total_send_time += s_inner.elapsed();
                        }

                        // All sending handled in the inner loop above; continue outer loop.
                        continue;
                    } else {
                        // No resampling - copy chunk to frame_send_buffer and take ownership
                        if self.frame_send_buffer.len() < chunk_len {
                            self.frame_send_buffer.resize(chunk_len, 0.0);
                        }
                        self.frame_send_buffer[..chunk_len]
                            .copy_from_slice(&self.resampler_buffer[..chunk_len]);
                        self.resampler_buffer.drain(..chunk_len);

                        let frame_data = take_frame_buffer(
                            &mut self.frame_send_buffer,
                            &self.recycle_rx,
                            chunk_len,
                        );

                        let frame =
                            AudioFrame::new(frame_data, frame_size, channels, source_sample_rate);
                        debug_assert_eq!(
                            frame.data.len(),
                            frame.num_frames * frame.num_channels,
                            "Non-resampled frame data size mismatch: data.len()={}, num_frames={}, num_channels={}",
                            frame.data.len(),
                            frame.num_frames,
                            frame.num_channels,
                        );

                        // Send with interruption support
                        if let Some(cmd) =
                            send_or_interrupt(message_tx, command_rx, DecoderMessage::Frame(frame))?
                        {
                            return Ok(DecoderLoopAction::Interrupted(cmd));
                        }
                        total_send_time += s_start.elapsed();
                    }
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
                    let remaining_samples = self.resampler_buffer.len();
                    let remaining_frames = remaining_samples / channels;

                    log::info!(
                        "[Decoder Thread] Flushing {} remaining samples ({} frames)",
                        remaining_samples,
                        remaining_frames
                    );

                    // Use chunk_buffer for padded chunk (avoids .clone() allocation)
                    let padded_len = frame_size * channels;
                    if self.chunk_buffer.len() < padded_len {
                        self.chunk_buffer.resize(padded_len, 0.0);
                    }
                    // Copy remaining samples and zero-pad
                    let copy_len = remaining_samples.min(padded_len);
                    self.chunk_buffer[..copy_len]
                        .copy_from_slice(&self.resampler_buffer[..copy_len]);
                    self.chunk_buffer[copy_len..padded_len].fill(0.0);

                    let max_output_frames = resampler.output_frames_for_input(frame_size);
                    let output_len = max_output_frames * channels;

                    if self.resample_output_buffer.len() < output_len {
                        self.resample_output_buffer.resize(output_len, 0.0);
                    }

                    let context = ProcessContext {
                        sample_rate: source_sample_rate,
                        num_frames: frame_size,
                    };

                    // Process padded chunk to flush resampler state
                    if let Ok(actual_output_frames) = resampler.process(
                        &self.chunk_buffer[..padded_len],
                        &mut self.resample_output_buffer,
                        &context,
                    ) {
                        // Use actual output frames returned by resampler.process()
                        if actual_output_frames > 0 {
                            let frame_len = actual_output_frames * channels;
                            // Use frame_send_buffer (avoids .to_vec() allocation)
                            if self.frame_send_buffer.len() < frame_len {
                                self.frame_send_buffer.resize(frame_len, 0.0);
                            }
                            self.frame_send_buffer[..frame_len]
                                .copy_from_slice(&self.resample_output_buffer[..frame_len]);

                            let frame_data = take_frame_buffer(
                                &mut self.frame_send_buffer,
                                &self.recycle_rx,
                                frame_len,
                            );

                            let frame = AudioFrame::new(
                                frame_data,
                                actual_output_frames,
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
                                actual_output_frames
                            );
                        }
                    } else {
                        log::warn!("[Decoder Thread] Failed to flush resampler");
                    }

                    self.resampler_buffer.clear();
                }

                // Gapless playback: if a next file is queued, transition seamlessly
                // without sending EndOfStream or Flush. The audio pipeline continues
                // uninterrupted with frames from the new file.
                if let Some(next_source) = self.queued_next.take() {
                    log::info!(
                        "[Decoder Thread] Gapless transition to: {}",
                        next_source.display_name()
                    );

                    // Keep resampler state — clear buffers but don't destroy the
                    // resampler. The new source may or may not need resampling; we
                    // re-evaluate below.
                    self.decoder = None;
                    self.resampler_buffer.clear();
                    self.resample_staging.clear();
                    self.decode_buffer = None;

                    match self.play(next_source.clone(), target_sample_rate, frame_size) {
                        Ok(()) => {
                            event_tx
                                .send(ThreadEvent::DecoderGaplessTransition(next_source))
                                .ok();
                            return Ok(DecoderLoopAction::Continue);
                        }
                        Err(e) => {
                            let msg = format!(
                                "Gapless transition to '{}' failed: {}",
                                next_source.display_name(),
                                e
                            );
                            log::warn!("[Decoder Thread] {}, falling through to EndOfStream", msg);
                            event_tx.send(ThreadEvent::DecoderError(msg)).ok();
                            // Fall through to normal end-of-stream below
                        }
                    }
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
        self.current_source = None;
        self.spec = None;
        self.silent_source = false;
        self.queued_next = None;
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
    /// Process HAL input. Returns:
    /// - `Ok((true, None))` — frame sent successfully
    /// - `Ok((false, None))` — no frame to send
    /// - `Ok((false, Some(cmd)))` — send was interrupted by a command that must be handled
    fn process_hal_input(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        #[cfg_attr(
            not(all(target_os = "macos", feature = "hal")),
            allow(unused_variables)
        )]
        command_rx: &Receiver<DecoderCommand>,
        frame_size: usize,
        target_sample_rate: u32,
    ) -> Result<(bool, Option<DecoderCommand>), String> {
        // Static counter for periodic logging (avoid log spam)
        static LOG_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let count = LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        #[cfg(all(target_os = "macos", feature = "hal"))]
        if let Some(reader) = &mut self.hal_reader {
            // Check if we have enough frames available
            if reader.available_read_frames() < frame_size {
                return Ok((false, None));
            }

            // Read from HAL - use actual HAL config
            let hal_sample_rate = reader.sample_rate();
            let hal_channels = reader.channel_count() as usize;
            let buffer_len = frame_size * hal_channels;

            if self.hal_input_buffer.len() != buffer_len {
                self.hal_input_buffer.resize(buffer_len, 0.0);
            }

            let samples_read = reader.read(&mut self.hal_input_buffer);

            if samples_read < buffer_len {
                self.hal_input_buffer[samples_read..].fill(0.0);
            }

            // Check if resampling is needed
            if hal_sample_rate != target_sample_rate {
                // Initialize or re-initialize resampler if needed
                let config_changed = self.resampler.is_none()
                    || self.last_hal_sample_rate != Some(hal_sample_rate)
                    || self.last_hal_channels != Some(hal_channels);

                if config_changed {
                    log::info!(
                        "[Decoder Thread] Creating HAL resampler: {}Hz {}ch -> {}Hz",
                        hal_sample_rate,
                        hal_channels,
                        target_sample_rate
                    );
                    self.resampler = Some(
                        ResamplerPlugin::new(
                            hal_channels,
                            hal_sample_rate,
                            target_sample_rate,
                            frame_size,
                        )
                        .map_err(|e| format!("Failed to create HAL resampler: {}", e))?,
                    );
                    self.last_hal_sample_rate = Some(hal_sample_rate);
                    self.last_hal_channels = Some(hal_channels);
                    // Clear any previous resampler buffer to avoid glitches
                    self.resample_output_buffer.clear();
                }

                let resampler = self.resampler.as_mut().unwrap();
                let max_output_frames = resampler.output_frames_for_input(frame_size);
                let output_len = max_output_frames * hal_channels;

                if self.resample_output_buffer.len() < output_len {
                    self.resample_output_buffer.resize(output_len, 0.0);
                }

                let context = ProcessContext {
                    sample_rate: hal_sample_rate,
                    num_frames: frame_size,
                };

                // Process resampling
                let actual_output_frames = resampler
                    .process(
                        &self.hal_input_buffer[..buffer_len],
                        &mut self.resample_output_buffer,
                        &context,
                    )
                    .map_err(|e| format!("HAL resampling failed: {}", e))?;

                // Copy to frame_send_buffer
                let frame_len = actual_output_frames * hal_channels;
                if self.frame_send_buffer.len() < frame_len {
                    self.frame_send_buffer.resize(frame_len, 0.0);
                }
                self.frame_send_buffer[..frame_len]
                    .copy_from_slice(&self.resample_output_buffer[..frame_len]);

                let frame_data =
                    take_frame_buffer(&mut self.frame_send_buffer, &self.recycle_rx, frame_len);

                // Send frame with TARGET sample rate
                let frame = AudioFrame::new(
                    frame_data,
                    actual_output_frames,
                    hal_channels,
                    target_sample_rate,
                );

                // Use send_or_interrupt so we can still receive commands
                // (Stop/Shutdown/Seek) while the downstream pipeline is full,
                // preventing a deadlock if processing or playback stalls.
                match send_or_interrupt(message_tx, command_rx, DecoderMessage::Frame(frame)) {
                    Ok(Some(cmd)) => {
                        log::debug!("[Decoder Thread] HAL send interrupted by command");
                        return Ok((false, Some(cmd)));
                    }
                    Ok(None) => {} // sent successfully
                    Err(e) => return Err(format!("Failed to send resampled HAL frame: {}", e)),
                }
            } else {
                // No resampling needed
                if self.resampler.is_some() {
                    log::info!("[Decoder Thread] Removing HAL resampler (rates match)");
                    self.resampler = None;
                    self.last_hal_sample_rate = Some(hal_sample_rate);
                }

                // Use take/restore pattern for zero-copy where possible
                let mut frame_data = std::mem::take(&mut self.hal_input_buffer);
                frame_data.truncate(buffer_len);
                self.hal_input_buffer = Vec::with_capacity(buffer_len);

                // Send frame with HAL sample rate (which matches target)
                let frame = AudioFrame::new(frame_data, frame_size, hal_channels, hal_sample_rate);

                match send_or_interrupt(message_tx, command_rx, DecoderMessage::Frame(frame)) {
                    Ok(Some(cmd)) => {
                        log::debug!("[Decoder Thread] HAL send interrupted by command");
                        return Ok((false, Some(cmd)));
                    }
                    Ok(None) => {}
                    Err(e) => return Err(format!("Failed to send HAL frame: {}", e)),
                }
            }

            return Ok((true, None));
        }

        // Log that we don't have a HAL reader
        if count.is_multiple_of(100) {
            log::warn!("[AUDIO FLOW] Decoder: No HAL reader available, sending silent frames");
        }

        // Fallback to silent frame if no reader (or not macOS)
        // Use frame_send_buffer to avoid allocation for silent frames
        let silent_len = frame_size * 2; // Assume stereo
        if self.frame_send_buffer.len() < silent_len {
            self.frame_send_buffer.resize(silent_len, 0.0);
        }
        self.frame_send_buffer[..silent_len].fill(0.0);

        let frame_data =
            take_frame_buffer(&mut self.frame_send_buffer, &self.recycle_rx, silent_len);

        let frame = AudioFrame::new(frame_data, frame_size, 2, target_sample_rate);
        message_tx
            .send(DecoderMessage::Frame(frame))
            .map_err(|_| "Failed to send silent frame")?;

        Ok((true, None))
    }
}

/// Main decoder thread function
fn run_decoder_thread(
    message_tx: SyncSender<DecoderMessage>,
    command_rx: Receiver<DecoderCommand>,
    response_tx: Sender<DecoderResponse>,
    event_tx: Sender<ThreadEvent>,
    target_sample_rate: u32,
    frame_size: usize,
    recycle_rx: Receiver<Vec<f32>>,
) -> Result<(), String> {
    let mut state = DecoderState::new(recycle_rx);

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
                DecoderCommand::Play(source) => {
                    message_tx.send(DecoderMessage::Flush).ok();
                    state.stop();
                    if let Err(e) = state.play(source, target_sample_rate, frame_size) {
                        log::debug!("[Decoder Thread] Play failed: {}", e);
                        event_tx.send(ThreadEvent::DecoderError(e)).ok();
                        response_tx
                            .send(DecoderResponse::Error(
                                "Failed to start playback".to_string(),
                            ))
                            .ok();
                    } else {
                        response_tx.send(DecoderResponse::Ok).ok();
                    }
                }
                DecoderCommand::PlayAt(source, position) => {
                    message_tx.send(DecoderMessage::Flush).ok();
                    state.stop();
                    if let Err(e) = state.play(source, target_sample_rate, frame_size) {
                        log::debug!("[Decoder Thread] PlayAt (load) failed: {}", e);
                        event_tx.send(ThreadEvent::DecoderError(e)).ok();
                        response_tx
                            .send(DecoderResponse::Error(
                                "Failed to start playback".to_string(),
                            ))
                            .ok();
                    } else if let Err(e) = state.seek(position) {
                        log::debug!("[Decoder Thread] PlayAt (seek) failed: {}", e);
                        event_tx.send(ThreadEvent::DecoderError(e)).ok();
                        response_tx
                            .send(DecoderResponse::Error(
                                "Failed to seek during play_at".to_string(),
                            ))
                            .ok();
                    } else {
                        response_tx.send(DecoderResponse::Ok).ok();
                    }
                }
                DecoderCommand::StartSilentSource => {
                    state.start_silent_source();
                }
                DecoderCommand::Pause => {
                    state.paused = true;
                    log::debug!("[Decoder Thread] Paused");
                    response_tx.send(DecoderResponse::Ok).ok();
                }
                DecoderCommand::Resume => {
                    state.paused = false;
                    log::debug!("[Decoder Thread] Resumed");
                    response_tx.send(DecoderResponse::Ok).ok();
                }
                DecoderCommand::Seek(position) => {
                    message_tx.send(DecoderMessage::Flush).ok();
                    if let Err(e) = state.seek(position) {
                        log::debug!("[Decoder Thread] Seek failed: {}", e);
                        response_tx.send(DecoderResponse::Error(e)).ok();
                    } else {
                        event_tx.send(ThreadEvent::SeekComplete).ok();
                        response_tx.send(DecoderResponse::Ok).ok();
                    }
                }
                DecoderCommand::QueueNext(source) => {
                    log::debug!("[Decoder Thread] Queued next: {}", source.display_name());
                    state.queued_next = Some(source);
                    response_tx.send(DecoderResponse::Ok).ok();
                }
                DecoderCommand::CancelNext => {
                    log::debug!("[Decoder Thread] Cancelled queued next");
                    state.queued_next = None;
                    response_tx.send(DecoderResponse::Ok).ok();
                }
                DecoderCommand::Stop => {
                    state.stop();
                    message_tx.send(DecoderMessage::Flush).ok();
                    log::debug!("[Decoder Thread] Stopped");
                    response_tx.send(DecoderResponse::Ok).ok();
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
            match state.process_hal_input(&message_tx, &command_rx, frame_size, target_sample_rate)
            {
                Ok((true, _)) => {
                    // Frame processed successfully
                    // Don't sleep if connected - rely on backpressure from message_tx
                }
                Ok((false, pending_cmd)) => {
                    // No frame processed — either not enough data or send was
                    // interrupted by a command. If a command was consumed from
                    // command_rx by send_or_interrupt, handle it now so it isn't lost.
                    if let Some(cmd) = pending_cmd {
                        // Re-inject the command by processing it inline.
                        // This mirrors the command handling at the top of the loop.
                        match cmd {
                            DecoderCommand::Stop => {
                                state.stop();
                                message_tx.send(DecoderMessage::Flush).ok();
                                log::debug!("[Decoder Thread] Stopped (from HAL interrupt)");
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                            DecoderCommand::Shutdown => {
                                log::debug!("[Decoder Thread] Shutting down (from HAL interrupt)");
                                return Ok(());
                            }
                            DecoderCommand::Pause => {
                                state.paused = true;
                                log::debug!("[Decoder Thread] Paused (from HAL interrupt)");
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                            DecoderCommand::Resume => {
                                state.paused = false;
                                log::debug!("[Decoder Thread] Resumed (from HAL interrupt)");
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                            DecoderCommand::Seek(position) => {
                                message_tx.send(DecoderMessage::Flush).ok();
                                if let Err(e) = state.seek(position) {
                                    response_tx.send(DecoderResponse::Error(e)).ok();
                                } else {
                                    event_tx.send(ThreadEvent::SeekComplete).ok();
                                    response_tx.send(DecoderResponse::Ok).ok();
                                }
                            }
                            DecoderCommand::Play(path) => {
                                message_tx.send(DecoderMessage::Flush).ok();
                                state.stop();
                                if let Err(e) = state.play(path, target_sample_rate, frame_size) {
                                    log::debug!(
                                        "[Decoder Thread] Play failed (from HAL interrupt): {}",
                                        e
                                    );
                                    event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                    response_tx
                                        .send(DecoderResponse::Error(
                                            "Failed to start playback".to_string(),
                                        ))
                                        .ok();
                                } else {
                                    response_tx.send(DecoderResponse::Ok).ok();
                                }
                            }
                            DecoderCommand::PlayAt(path, position) => {
                                message_tx.send(DecoderMessage::Flush).ok();
                                state.stop();
                                if let Err(e) = state.play(path, target_sample_rate, frame_size) {
                                    log::debug!(
                                        "[Decoder Thread] PlayAt failed (from HAL interrupt): {}",
                                        e
                                    );
                                    event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                    response_tx
                                        .send(DecoderResponse::Error(
                                            "Failed to start playback".to_string(),
                                        ))
                                        .ok();
                                } else if let Err(e) = state.seek(position) {
                                    log::debug!(
                                        "[Decoder Thread] PlayAt seek failed (from HAL interrupt): {}",
                                        e
                                    );
                                    event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                    response_tx
                                        .send(DecoderResponse::Error(
                                            "Failed to seek during play_at".to_string(),
                                        ))
                                        .ok();
                                } else {
                                    response_tx.send(DecoderResponse::Ok).ok();
                                }
                            }
                            DecoderCommand::StartSilentSource => {
                                state.start_silent_source();
                            }
                            DecoderCommand::QueueNext(path) => {
                                log::debug!("[Decoder Thread] Queued next (from HAL interrupt): {:?}", path);
                                state.queued_next = Some(path);
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                            DecoderCommand::CancelNext => {
                                log::debug!("[Decoder Thread] Cancelled queued next (from HAL interrupt)");
                                state.queued_next = None;
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                        }
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                Err(e) => {
                    log::debug!("[Decoder Thread] HAL input error: {}", e);
                    state.stop();
                }
            }

            // Handle sleep for non-HAL mode or disconnected HAL
            #[cfg(all(target_os = "macos", feature = "hal"))]
            {
                if state.hal_reader.as_ref().is_none_or(|r| !r.is_connected()) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
            #[cfg(not(all(target_os = "macos", feature = "hal")))]
            {
                // Non-HAL silent source mode: sleep to maintain frame rate
                let frame_duration_ms =
                    (frame_size as f64 / target_sample_rate as f64 * 1000.0) as u64;
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
                            message_tx.send(DecoderMessage::Flush).ok();
                            state.stop();
                            if let Err(e) = state.play(path, target_sample_rate, frame_size) {
                                log::debug!("[Decoder Thread] Play failed: {}", e);
                                event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                response_tx
                                    .send(DecoderResponse::Error(
                                        "Failed to start playback".to_string(),
                                    ))
                                    .ok();
                            } else {
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                        }
                        DecoderCommand::PlayAt(path, position) => {
                            message_tx.send(DecoderMessage::Flush).ok();
                            state.stop();
                            if let Err(e) = state.play(path, target_sample_rate, frame_size) {
                                log::debug!("[Decoder Thread] PlayAt (load) failed: {}", e);
                                event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                response_tx
                                    .send(DecoderResponse::Error(
                                        "Failed to start playback".to_string(),
                                    ))
                                    .ok();
                            } else if let Err(e) = state.seek(position) {
                                log::debug!("[Decoder Thread] PlayAt (seek) failed: {}", e);
                                event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                response_tx
                                    .send(DecoderResponse::Error(
                                        "Failed to seek during play_at".to_string(),
                                    ))
                                    .ok();
                            } else {
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                        }
                        DecoderCommand::StartSilentSource => {
                            state.start_silent_source();
                        }
                        DecoderCommand::Pause => {
                            state.paused = true;
                            log::debug!("[Decoder Thread] Paused");
                            response_tx.send(DecoderResponse::Ok).ok();
                        }
                        DecoderCommand::Resume => {
                            state.paused = false;
                            log::debug!("[Decoder Thread] Resumed");
                            response_tx.send(DecoderResponse::Ok).ok();
                        }
                        DecoderCommand::Seek(position) => {
                            message_tx.send(DecoderMessage::Flush).ok();
                            if let Err(e) = state.seek(position) {
                                log::debug!("[Decoder Thread] Seek failed: {}", e);
                                response_tx.send(DecoderResponse::Error(e)).ok();
                            } else {
                                event_tx.send(ThreadEvent::SeekComplete).ok();
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                        }
                        DecoderCommand::QueueNext(path) => {
                            log::debug!("[Decoder Thread] Queued next (from interrupt): {:?}", path);
                            state.queued_next = Some(path);
                            response_tx.send(DecoderResponse::Ok).ok();
                        }
                        DecoderCommand::CancelNext => {
                            log::debug!("[Decoder Thread] Cancelled queued next (from interrupt)");
                            state.queued_next = None;
                            response_tx.send(DecoderResponse::Ok).ok();
                        }
                        DecoderCommand::Stop => {
                            state.stop();
                            message_tx.send(DecoderMessage::Flush).ok();
                            log::debug!("[Decoder Thread] Stopped");
                            response_tx.send(DecoderResponse::Ok).ok();
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

#[cfg(test)]
mod tests {
    /// Regression test for the upmixer silence bug with cross-rate resampling.
    ///
    /// Root cause: the rubato resampler produces a variable number of output frames
    /// per input chunk (e.g. ~940 frames for 48 kHz → 44.1 kHz with a 1024-frame
    /// input block).  If those sub-`frame_size` blocks were forwarded directly to
    /// the processing thread, plugins whose hop size equals `frame_size` (like the
    /// upmixer at hop = 1024) would never accumulate enough input to fire an FFT
    /// block, producing complete silence.
    ///
    /// The fix is a `resample_staging` buffer that accumulates resampled output and
    /// only emits complete `frame_size`-frame blocks.  This test verifies the
    /// staging logic in isolation: when fed chunks smaller than `frame_size` the
    /// staging buffer must hold them, and when the accumulated total reaches
    /// `frame_size` it must emit exactly one full block.
    #[test]
    fn test_resample_staging_emits_full_frame_size_blocks() {
        let frame_size: usize = 1024;
        let channels: usize = 2;
        let send_chunk_len = frame_size * channels;

        // Simulate the resampler producing ~940 frames per 1024-frame input
        // (48 kHz → 44.1 kHz ratio ≈ 0.91875).
        let resampled_chunk_frames: usize = 940;
        let resampled_chunk_len = resampled_chunk_frames * channels;

        let mut staging: Vec<f32> = Vec::new();
        let mut emitted_blocks: usize = 0;

        // Feed several resampled chunks and count how many full blocks are emitted.
        // Two chunks of 940 = 1880 samples → one complete 1024-frame block with
        // 856 frames left in staging.
        for chunk_idx in 0..4 {
            // Each sample is tagged with chunk index for easy debugging.
            let chunk: Vec<f32> = (0..resampled_chunk_len)
                .map(|i| (chunk_idx * 1000 + i) as f32)
                .collect();
            staging.extend_from_slice(&chunk);

            while staging.len() >= send_chunk_len {
                staging.drain(..send_chunk_len);
                emitted_blocks += 1;
            }
        }

        // 4 chunks × 1880 samples = 7520 samples.
        // 7520 / 2048 = 3 full blocks (6144 samples), remainder 1376 samples = 688 frames.
        assert_eq!(
            emitted_blocks, 3,
            "Expected 3 full blocks after 4 × 940-frame chunks"
        );
        let expected_remainder = (4 * resampled_chunk_len) - (3 * send_chunk_len);
        assert_eq!(
            staging.len(),
            expected_remainder,
            "Staging buffer should hold the partial remainder (688 frames)"
        );
        assert!(
            staging.len() < send_chunk_len,
            "Remainder must be less than one full block"
        );

        // Feed one more chunk: 1376 + 1880 = 3256 → 1 more full block.
        let chunk: Vec<f32> = vec![0.0; resampled_chunk_len];
        staging.extend_from_slice(&chunk);
        while staging.len() >= send_chunk_len {
            staging.drain(..send_chunk_len);
            emitted_blocks += 1;
        }
        assert_eq!(
            emitted_blocks, 4,
            "Expected a fourth full block after the fifth chunk"
        );
    }
}
