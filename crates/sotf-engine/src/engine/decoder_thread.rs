// ============================================================================
// Decoder Thread - Audio Decoding + Resampling
// ============================================================================
//
// Decodes audio files using Symphonia and resamples if needed.

use super::{AudioFrame, DecoderCommand, DecoderMessage, DecoderResponse, ThreadEvent};
use crate::decoder::{
    AudioDecoder, AudioSource, AudioSpec, DecodedAudio,
    create_decoder_from_source_with_dsd_mode_and_metadata,
};
use sotf_plugins::{Plugin, ProcessContext, ResamplerPlugin};
use sotf_types::DsdOutputMode;
#[cfg(feature = "streaming")]
use sotf_types::StreamMetadata;
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::time::{Duration, Instant};

#[cfg(all(target_os = "macos", feature = "hal"))]
use driver_hal::HalInputReader;

const SPIN_MS_SLEEP_DECODER: u64 = 1;
const SEND_OR_INTERRUPT_MAX_RETRIES: usize = 200;
#[cfg(all(target_os = "macos", feature = "hal"))]
const HAL_RECONNECT_INTERVAL: Duration = Duration::from_secs(1);
/// Float PCM in CoreAudio can exceed 1.0 briefly, but values this large are
/// unsafe and indicate a feedback loop, stale/corrupt shared memory, or a
/// format/key mismatch. Drop the whole block instead of feeding a runaway path.
#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
const HAL_INPUT_RUNAWAY_PEAK_LIMIT: f32 = 8.0;
/// Maximum size of resample staging buffer to prevent unbounded growth.
/// This limits memory usage while ensuring we can handle typical resampling
/// ratios (e.g., 48kHz→44.1kHz produces ~940-frame blocks, so we allow ~4x that).
const MAX_RESAMPLE_STAGING_SAMPLES: usize = 1024 * 8 * 4; // 1024 frames * 8 channels * 4x margin

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
struct HalInputGuardTrip {
    peak: f32,
    invalid_samples: usize,
    over_limit_samples: usize,
}

#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
fn inspect_hal_input_block(samples: &[f32]) -> Option<HalInputGuardTrip> {
    let mut peak = 0.0f32;
    let mut invalid_samples = 0usize;
    let mut over_limit_samples = 0usize;

    for &sample in samples {
        if !sample.is_finite() {
            invalid_samples += 1;
            continue;
        }

        let abs_sample = sample.abs();
        peak = peak.max(abs_sample);
        if abs_sample > HAL_INPUT_RUNAWAY_PEAK_LIMIT {
            over_limit_samples += 1;
        }
    }

    if invalid_samples > 0 || over_limit_samples > 0 {
        Some(HalInputGuardTrip {
            peak,
            invalid_samples,
            over_limit_samples,
        })
    } else {
        None
    }
}

#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
fn guard_hal_input_block(samples: &mut [f32]) -> Option<HalInputGuardTrip> {
    let trip = inspect_hal_input_block(samples)?;
    samples.fill(0.0);
    Some(trip)
}

/// Action returned by decode loop
enum DecoderLoopAction {
    Continue,
    Stop,
    Interrupted(DecoderCommand),
}

/// Cursor-backed FIFO for decoded samples.
///
/// The decoder consumes from the front on every emitted frame. Keeping a cursor
/// avoids a `Vec::drain(..n)` memmove in the hot path; compaction only happens
/// before growth when the consumed prefix is large enough to matter.
#[derive(Debug, Default)]
struct SampleQueue {
    data: Vec<f32>,
    start: usize,
}

impl SampleQueue {
    fn new() -> Self {
        Self::default()
    }

    fn len(&self) -> usize {
        self.data.len().saturating_sub(self.start)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn as_slice(&self) -> &[f32] {
        &self.data[self.start..]
    }

    fn prefix(&self, len: usize) -> &[f32] {
        &self.as_slice()[..len]
    }

    fn extend_from_slice(&mut self, samples: &[f32]) {
        self.compact_if_needed(samples.len());
        self.data.extend_from_slice(samples);
    }

    fn consume(&mut self, len: usize) {
        debug_assert!(len <= self.len());
        self.start += len.min(self.len());
        if self.start == self.data.len() {
            self.clear();
        }
    }

    fn clear(&mut self) {
        self.data.clear();
        self.start = 0;
    }

    fn compact_if_needed(&mut self, incoming_len: usize) {
        if self.start == 0 {
            return;
        }
        if self.start == self.data.len() {
            self.clear();
            return;
        }

        let retained = self.len();
        let would_grow_past_consumed = retained + incoming_len > self.start;
        if self.start >= 8192 && would_grow_past_consumed {
            self.data.copy_within(self.start.., 0);
            self.data.truncate(retained);
            self.start = 0;
        }
    }
}

const DECODER_LOCAL_FRAME_POOL_SIZE: usize = 8;
const DECODER_LOCAL_FRAME_CAPACITY: usize = 1024 * 8;

/// Take frame_send_buffer for sending, then restore it from a recycled Vec or
/// the local spare pool so the steady-state handoff does not allocate.
fn take_frame_buffer(
    frame_send_buffer: &mut Vec<f32>,
    recycle_rx: &Receiver<Vec<f32>>,
    local_pool: &mut Vec<Vec<f32>>,
    len: usize,
) -> Vec<f32> {
    let mut frame_data = std::mem::take(frame_send_buffer);
    frame_data.truncate(len);

    *frame_send_buffer = match recycle_rx.try_recv() {
        Ok(mut v) => {
            v.clear();
            v
        }
        Err(_) => local_pool.pop().unwrap_or_default(),
    };

    frame_data
}

#[cfg(any(test, all(target_os = "macos", feature = "hal")))]
fn frames_to_sample_count(frames: usize, channels: usize, max_samples: usize) -> usize {
    frames.saturating_mul(channels).min(max_samples)
}

/// Helper to send a message with backpressure handling and interruption support
fn send_or_interrupt<T>(
    tx: &SyncSender<T>,
    rx: &Receiver<DecoderCommand>,
    mut msg: T,
) -> Result<Option<(DecoderCommand, T)>, String> {
    let mut retries = 0;
    loop {
        match tx.try_send(msg) {
            Ok(_) => return Ok(None),
            Err(std::sync::mpsc::TrySendError::Full(returned_msg)) => {
                // Buffer full - check for interruption
                if let Ok(cmd) = rx.try_recv() {
                    return Ok(Some((cmd, returned_msg)));
                }
                retries += 1;
                if retries > SEND_OR_INTERRUPT_MAX_RETRIES {
                    return Err("Decoder output queue stuck for >200ms".to_string());
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
        dsd_output: DsdOutputMode,
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
                    dsd_output,
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
    resampler_buffer: SampleQueue,
    paused: bool,
    current_source: Option<AudioSource>,
    spec: Option<AudioSpec>,
    silent_source: bool, // For HAL input plugins (no file source)
    silent_source_channels: usize,
    decode_buffer: Option<DecodedAudio>,
    resample_output_buffer: Vec<f32>,
    /// Staging buffer: accumulates resampled output across decode chunks so that
    /// only complete `frame_size`-frame blocks are forwarded to the processing thread.
    /// Without this, a 48 kHz source resampled to 44.1 kHz produces ~940-frame blocks
    /// which are smaller than the upmixer's hop size (1024), causing the upmixer to
    /// never fire an FFT block and produce silence.
    resample_staging: SampleQueue,
    /// Pre-allocated buffer for chunk processing (avoids allocation in hot path)
    chunk_buffer: Vec<f32>,
    /// Pre-allocated buffer for frame sending (avoids allocation in hot path)
    frame_send_buffer: Vec<f32>,
    /// Local spare buffers used when the recycle queue is temporarily empty.
    frame_buffer_pool: Vec<Vec<f32>>,
    /// Receives recycled Vec<f32> buffers from the processing thread
    recycle_rx: Receiver<Vec<f32>>,
    /// Queued next source for gapless playback. When set and the current source ends,
    /// the decoder seamlessly transitions to this source without sending EndOfStream/Flush.
    queued_next: Option<AudioSource>,
    dsd_output: DsdOutputMode,
    #[cfg(feature = "streaming")]
    stream_metadata_rx: Option<crate::decoder::SourceMetadataReceiver>,
    #[cfg(feature = "streaming")]
    stream_metadata: Option<StreamMetadata>,

    #[cfg(all(target_os = "macos", feature = "hal"))]
    hal_input_buffer: Vec<f32>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    hal_reader: Option<HalInputReader>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    last_hal_reconnect_attempt: Option<Instant>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    last_hal_cipher_reload_attempt: Option<Instant>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    last_hal_sample_rate: Option<u32>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    last_hal_channels: Option<usize>,
}

impl DecoderState {
    fn new(recycle_rx: Receiver<Vec<f32>>, dsd_output: DsdOutputMode) -> Self {
        let mut frame_buffer_pool = Vec::with_capacity(DECODER_LOCAL_FRAME_POOL_SIZE);
        for _ in 0..DECODER_LOCAL_FRAME_POOL_SIZE {
            frame_buffer_pool.push(Vec::with_capacity(DECODER_LOCAL_FRAME_CAPACITY));
        }

        Self {
            decoder: None,
            resampler: None,
            resampler_buffer: SampleQueue::new(),
            paused: false,
            current_source: None,
            spec: None,
            silent_source: false,
            silent_source_channels: 2,
            decode_buffer: None,
            resample_output_buffer: Vec::new(),
            resample_staging: SampleQueue::new(),
            // Pre-allocate for typical frame size (1024 frames * 8 channels)
            chunk_buffer: Vec::with_capacity(1024 * 8),
            frame_send_buffer: Vec::with_capacity(1024 * 8),
            frame_buffer_pool,
            recycle_rx,
            queued_next: None,
            dsd_output,
            #[cfg(feature = "streaming")]
            stream_metadata_rx: None,
            #[cfg(feature = "streaming")]
            stream_metadata: None,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            hal_input_buffer: Vec::new(),
            #[cfg(all(target_os = "macos", feature = "hal"))]
            hal_reader: None,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            last_hal_reconnect_attempt: None,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            last_hal_cipher_reload_attempt: None,
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
        let (decoder, metadata_rx) =
            create_decoder_from_source_with_dsd_mode_and_metadata(&source, self.dsd_output)
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
        #[cfg(feature = "streaming")]
        {
            self.stream_metadata_rx = metadata_rx;
        }
        #[cfg(not(feature = "streaming"))]
        let _ = metadata_rx;

        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            self.hal_reader = None;
        }

        Ok(())
    }

    fn try_gapless_transition_preserving_buffers(
        &mut self,
        target_sample_rate: u32,
    ) -> Result<Option<AudioSource>, String> {
        let Some(next_source) = self.queued_next.take() else {
            return Ok(None);
        };

        let current_spec = self.spec.as_ref().ok_or("No spec")?.clone();
        let (next_decoder, metadata_rx) =
            create_decoder_from_source_with_dsd_mode_and_metadata(&next_source, self.dsd_output)
                .map_err(|e| format!("Failed to create decoder: {:?}", e))?;
        let next_spec = next_decoder.spec().clone();

        let compatible = current_spec.channels == next_spec.channels
            && current_spec.sample_rate == next_spec.sample_rate
            && (current_spec.sample_rate != target_sample_rate) == self.resampler.is_some();

        if !compatible {
            self.queued_next = Some(next_source);
            return Ok(None);
        }

        log::info!(
            "[Decoder Thread] Gapless transition preserving decoder buffers: {}",
            next_source.display_name()
        );

        self.decoder = Some(next_decoder);
        self.current_source = Some(next_source.clone());
        self.spec = Some(next_spec);
        #[cfg(feature = "streaming")]
        {
            self.stream_metadata_rx = metadata_rx;
        }
        #[cfg(not(feature = "streaming"))]
        let _ = metadata_rx;
        self.decode_buffer = None;
        self.paused = false;
        self.silent_source = false;

        Ok(Some(next_source))
    }

    #[cfg(feature = "streaming")]
    fn clear_stream_metadata(&mut self, event_tx: &Sender<ThreadEvent>) {
        self.stream_metadata_rx = None;
        if self.stream_metadata.take().is_some() {
            event_tx.send(ThreadEvent::StreamMetadataChanged(None)).ok();
        }
    }

    #[cfg(feature = "streaming")]
    fn poll_stream_metadata(&mut self, event_tx: &Sender<ThreadEvent>) {
        use std::sync::mpsc::TryRecvError;

        let mut changed = false;
        loop {
            let event = match self.stream_metadata_rx.as_ref() {
                Some(rx) => match rx.try_recv() {
                    Ok(event) => event,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.stream_metadata_rx = None;
                        break;
                    }
                },
                None => break,
            };

            let mut next = self.stream_metadata.clone().unwrap_or_default();
            match event {
                sotf_streaming::StreamMetadata::Icy(icy) => {
                    next.stream_title = icy.stream_title;
                    next.stream_url = icy.stream_url;
                    changed = true;
                }
                sotf_streaming::StreamMetadata::ContentType(content_type) => {
                    next.content_type = Some(content_type);
                    changed = true;
                }
                sotf_streaming::StreamMetadata::Bitrate(kbps) => {
                    next.bitrate_kbps = Some(kbps);
                    changed = true;
                }
            }

            self.stream_metadata = Some(next);
        }

        if changed {
            event_tx
                .send(ThreadEvent::StreamMetadataChanged(
                    self.stream_metadata.clone(),
                ))
                .ok();
        }
    }

    fn send_decoder_message(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        command_rx: &Receiver<DecoderCommand>,
        response_tx: &Sender<DecoderResponse>,
        message: DecoderMessage,
    ) -> Result<Option<DecoderCommand>, String> {
        let mut pending = message;

        loop {
            match send_or_interrupt(message_tx, command_rx, pending)? {
                None => return Ok(None),
                Some((cmd, returned_message)) => {
                    if matches!(returned_message, DecoderMessage::EndOfStream) {
                        return Ok(Some(cmd));
                    }

                    match cmd {
                        DecoderCommand::QueueNext(source) => {
                            log::debug!(
                                "[Decoder Thread] Queued next while sending frame: {}",
                                source.display_name()
                            );
                            self.queued_next = Some(source);
                            response_tx.send(DecoderResponse::Ok).ok();
                            pending = returned_message;
                        }
                        DecoderCommand::CancelNext => {
                            log::debug!("[Decoder Thread] Cancelled queued next while sending");
                            self.queued_next = None;
                            response_tx.send(DecoderResponse::Ok).ok();
                            pending = returned_message;
                        }
                        other => return Ok(Some(other)),
                    }
                }
            }
        }
    }

    fn send_prepared_frame(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        command_rx: &Receiver<DecoderCommand>,
        response_tx: &Sender<DecoderResponse>,
        sample_len: usize,
        num_frames: usize,
        channels: usize,
        sample_rate: u32,
    ) -> Result<Option<DecoderCommand>, String> {
        let frame_data = take_frame_buffer(
            &mut self.frame_send_buffer,
            &self.recycle_rx,
            &mut self.frame_buffer_pool,
            sample_len,
        );
        let frame = AudioFrame::new(frame_data, num_frames, channels, sample_rate);
        self.send_decoder_message(
            message_tx,
            command_rx,
            response_tx,
            DecoderMessage::Frame(frame),
        )
    }

    fn emit_resample_staging(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        command_rx: &Receiver<DecoderCommand>,
        response_tx: &Sender<DecoderResponse>,
        frame_size: usize,
        channels: usize,
        sample_rate: u32,
        emit_partial: bool,
    ) -> Result<Option<DecoderCommand>, String> {
        let send_chunk_len = frame_size * channels;

        while self.resample_staging.len() >= send_chunk_len {
            if self.frame_send_buffer.len() < send_chunk_len {
                self.frame_send_buffer.resize(send_chunk_len, 0.0);
            }
            self.frame_send_buffer[..send_chunk_len]
                .copy_from_slice(self.resample_staging.prefix(send_chunk_len));
            self.resample_staging.consume(send_chunk_len);

            if let Some(cmd) = self.send_prepared_frame(
                message_tx,
                command_rx,
                response_tx,
                send_chunk_len,
                frame_size,
                channels,
                sample_rate,
            )? {
                return Ok(Some(cmd));
            }
        }

        if emit_partial && !self.resample_staging.is_empty() {
            let aligned_len =
                self.resample_staging.len() - (self.resample_staging.len() % channels);
            if aligned_len > 0 {
                if self.frame_send_buffer.len() < aligned_len {
                    self.frame_send_buffer.resize(aligned_len, 0.0);
                }
                self.frame_send_buffer[..aligned_len]
                    .copy_from_slice(self.resample_staging.prefix(aligned_len));
                self.resample_staging.consume(aligned_len);

                if let Some(cmd) = self.send_prepared_frame(
                    message_tx,
                    command_rx,
                    response_tx,
                    aligned_len,
                    aligned_len / channels,
                    channels,
                    sample_rate,
                )? {
                    return Ok(Some(cmd));
                }
            }
            if !self.resample_staging.is_empty() {
                log::warn!(
                    "[Decoder Thread] Dropping {} unaligned resample staging samples at EOS",
                    self.resample_staging.len()
                );
                self.resample_staging.clear();
            }
        }

        Ok(None)
    }

    /// Decode and send chunks
    fn decode_chunk(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        command_rx: &Receiver<DecoderCommand>,
        response_tx: &Sender<DecoderResponse>,
        event_tx: &Sender<ThreadEvent>,
        frame_size: usize,
        target_sample_rate: u32,
    ) -> Result<DecoderLoopAction, String> {
        let spec = self.spec.as_ref().ok_or("No spec")?.clone();

        // Use internal buffer for decoding
        if self.decode_buffer.is_none() {
            self.decode_buffer = Some(DecodedAudio::new(spec.clone()));
        }

        // Decode next chunk
        let decode_result = {
            let decoder = self.decoder.as_mut().ok_or("No decoder")?;
            let decode_buffer = self
                .decode_buffer
                .as_mut()
                .ok_or("Decoder invariant violated: decode buffer missing before decoding")?;
            decoder.decode_into(decode_buffer)
        };

        match decode_result {
            Ok(frames_decoded) if frames_decoded > 0 => {
                let channels = spec.channels as usize;
                let source_sample_rate = spec.sample_rate;

                let mut total_resample_time = Duration::ZERO;
                let mut total_send_time = Duration::ZERO;

                // Add to buffer (reusing resampler_buffer as general sample buffer)
                let decoded_samples = &self
                    .decode_buffer
                    .as_ref()
                    .ok_or("Decoder invariant violated: decode buffer missing after decode")?
                    .samples;
                self.resampler_buffer.extend_from_slice(decoded_samples);

                // Process buffer in frame_size chunks
                while self.resampler_buffer.len() >= frame_size * channels {
                    // If resampling, we need enough input samples for one output frame
                    // But here we simplify: just process fixed input chunks

                    let s_start = Instant::now();
                    let chunk_len = frame_size * channels;

                    if let Some(resampler) = &mut self.resampler {
                        // Copy chunk to pre-allocated buffer.
                        if self.chunk_buffer.len() < chunk_len {
                            self.chunk_buffer.resize(chunk_len, 0.0);
                        }
                        self.chunk_buffer[..chunk_len]
                            .copy_from_slice(self.resampler_buffer.prefix(chunk_len));
                        self.resampler_buffer.consume(chunk_len);

                        // Resample
                        let max_output_frames = resampler.output_frames_for_input(frame_size);
                        let output_len = max_output_frames * channels;

                        if self.resample_output_buffer.len() < output_len {
                            self.resample_output_buffer.resize(output_len, 0.0);
                        }

                        let context = ProcessContext::new(source_sample_rate, frame_size);

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
                            // Staging buffer growing too large — consume the oldest
                            // complete-frame-sized blocks to stay under the cap
                            // while preserving any partial frame at the tail.
                            let send_chunk_len_here = frame_size * channels;
                            let excess = new_staging_len - MAX_RESAMPLE_STAGING_SAMPLES;
                            // Round up to whole frame chunks to keep alignment
                            let drain_amount = if send_chunk_len_here > 0 {
                                excess.div_ceil(send_chunk_len_here) * send_chunk_len_here
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
                            self.resample_staging.consume(drain_amount);
                        }
                        self.resample_staging
                            .extend_from_slice(&self.resample_output_buffer[..frame_len]);

                        let s_inner = Instant::now();
                        if let Some(cmd) = self.emit_resample_staging(
                            message_tx,
                            command_rx,
                            response_tx,
                            frame_size,
                            channels,
                            target_sample_rate,
                            false,
                        )? {
                            return Ok(DecoderLoopAction::Interrupted(cmd));
                        }
                        total_send_time += s_inner.elapsed();

                        // All sending handled in the inner loop above; continue outer loop.
                        continue;
                    } else {
                        // No resampling - copy chunk to frame_send_buffer and take ownership
                        if self.frame_send_buffer.len() < chunk_len {
                            self.frame_send_buffer.resize(chunk_len, 0.0);
                        }
                        self.frame_send_buffer[..chunk_len]
                            .copy_from_slice(self.resampler_buffer.prefix(chunk_len));
                        self.resampler_buffer.consume(chunk_len);

                        // Send with interruption support
                        if let Some(cmd) = self.send_prepared_frame(
                            message_tx,
                            command_rx,
                            response_tx,
                            chunk_len,
                            frame_size,
                            channels,
                            source_sample_rate,
                        )? {
                            return Ok(DecoderLoopAction::Interrupted(cmd));
                        }
                        total_send_time += s_start.elapsed();
                    }
                }

                // Update position
                let position_sec = self
                    .decoder
                    .as_ref()
                    .map(|decoder| decoder.position() as f64 / source_sample_rate as f64)
                    .unwrap_or(0.0);
                event_tx
                    .send(ThreadEvent::PositionUpdate(position_sec))
                    .ok();

                Ok(DecoderLoopAction::Continue)
            }
            Ok(0) => {
                // End of stream
                log::debug!("[Decoder Thread] End of stream");

                // If the next source has the same decoder shape, carry the
                // residual input/resampler/staging state across the boundary.
                // That lets frame-size blocks straddle the file transition
                // instead of zero-padding or truncating the current source tail.
                match self.try_gapless_transition_preserving_buffers(target_sample_rate) {
                    Ok(Some(next_source)) => {
                        event_tx
                            .send(ThreadEvent::DecoderGaplessTransition(next_source))
                            .ok();
                        return Ok(DecoderLoopAction::Continue);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let msg = format!("Gapless transition failed: {}", e);
                        log::warn!("[Decoder Thread] {}", msg);
                        event_tx.send(ThreadEvent::DecoderError(msg)).ok();
                    }
                }

                let channels = spec.channels as usize;
                let source_sample_rate = spec.sample_rate;

                if self.resampler.is_some() {
                    let remaining_samples = self.resampler_buffer.len();
                    if remaining_samples > 0 {
                        let remaining_frames = remaining_samples / channels;

                        log::info!(
                            "[Decoder Thread] Flushing {} remaining samples ({} frames)",
                            remaining_samples,
                            remaining_frames
                        );

                        // Use chunk_buffer for padded chunk.
                        let padded_len = frame_size * channels;
                        if self.chunk_buffer.len() < padded_len {
                            self.chunk_buffer.resize(padded_len, 0.0);
                        }
                        let copy_len = remaining_samples.min(padded_len);
                        self.chunk_buffer[..copy_len]
                            .copy_from_slice(&self.resampler_buffer.as_slice()[..copy_len]);
                        self.chunk_buffer[copy_len..padded_len].fill(0.0);
                        self.resampler_buffer.clear();

                        let resampler = self.resampler.as_mut().ok_or(
                            "Decoder invariant violated: resampler missing in EOS flush path",
                        )?;
                        let max_output_frames = resampler.output_frames_for_input(frame_size);
                        let output_len = max_output_frames * channels;

                        if self.resample_output_buffer.len() < output_len {
                            self.resample_output_buffer.resize(output_len, 0.0);
                        }

                        let context = ProcessContext::new(source_sample_rate, frame_size);

                        match resampler.process(
                            &self.chunk_buffer[..padded_len],
                            &mut self.resample_output_buffer,
                            &context,
                        ) {
                            Ok(actual_output_frames) => {
                                if actual_output_frames > 0 {
                                    let frame_len = actual_output_frames * channels;
                                    self.resample_staging.extend_from_slice(
                                        &self.resample_output_buffer[..frame_len],
                                    );
                                    log::debug!(
                                        "[Decoder Thread] Flushed {} frames through resampler",
                                        actual_output_frames
                                    );
                                }
                            }
                            Err(e) => {
                                log::warn!("[Decoder Thread] Failed to flush resampler: {}", e);
                            }
                        }
                    }

                    if let Some(cmd) = self.emit_resample_staging(
                        message_tx,
                        command_rx,
                        response_tx,
                        frame_size,
                        channels,
                        target_sample_rate,
                        true,
                    )? {
                        return Ok(DecoderLoopAction::Interrupted(cmd));
                    }
                } else if !self.resampler_buffer.is_empty() {
                    let remaining_samples = self.resampler_buffer.len();
                    let aligned_len = remaining_samples - (remaining_samples % channels);
                    if aligned_len > 0 {
                        if self.frame_send_buffer.len() < aligned_len {
                            self.frame_send_buffer.resize(aligned_len, 0.0);
                        }
                        self.frame_send_buffer[..aligned_len]
                            .copy_from_slice(self.resampler_buffer.prefix(aligned_len));
                        self.resampler_buffer.consume(aligned_len);

                        if let Some(cmd) = self.send_prepared_frame(
                            message_tx,
                            command_rx,
                            response_tx,
                            aligned_len,
                            aligned_len / channels,
                            channels,
                            source_sample_rate,
                        )? {
                            return Ok(DecoderLoopAction::Interrupted(cmd));
                        }
                    }
                    if !self.resampler_buffer.is_empty() {
                        log::warn!(
                            "[Decoder Thread] Dropping {} unaligned decoded samples at EOS",
                            self.resampler_buffer.len()
                        );
                        self.resampler_buffer.clear();
                    }
                }

                // Incompatible queued sources (for example a sample-rate change)
                // transition after the current source has been explicitly flushed.
                if let Some(next_source) = self.queued_next.take() {
                    log::info!(
                        "[Decoder Thread] Gapless transition to: {}",
                        next_source.display_name()
                    );

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

                if let Some(cmd) = self.send_decoder_message(
                    message_tx,
                    command_rx,
                    response_tx,
                    DecoderMessage::EndOfStream,
                )? {
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
            self.resample_staging.clear();

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
        self.resample_staging.clear();
        self.current_source = None;
        self.spec = None;
        self.silent_source = false;
        self.queued_next = None;
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            self.hal_reader = None;
            self.last_hal_reconnect_attempt = None;
            self.last_hal_cipher_reload_attempt = None;
        }
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    fn try_reconnect_hal_reader(&mut self, force: bool) {
        if self.hal_reader.as_ref().is_some_and(|r| r.is_connected()) {
            return;
        }

        let now = Instant::now();
        if !force
            && self
                .last_hal_reconnect_attempt
                .is_some_and(|last| now.duration_since(last) < HAL_RECONNECT_INTERVAL)
        {
            return;
        }

        self.last_hal_reconnect_attempt = Some(now);
        match HalInputReader::new() {
            Some(reader) => {
                log::info!("[Decoder Thread] Connected HAL input reader");
                self.hal_reader = Some(reader);
            }
            None => {
                log::debug!("[Decoder Thread] HAL input reader still unavailable");
                self.hal_reader = None;
            }
        }
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    fn reload_hal_cipher_if_needed(&mut self) -> bool {
        let Some(reader) = self.hal_reader.as_mut() else {
            return false;
        };
        if !reader.needs_cipher_reload() {
            return true;
        }

        let now = Instant::now();
        if self
            .last_hal_cipher_reload_attempt
            .is_some_and(|last| now.duration_since(last) < HAL_RECONNECT_INTERVAL)
        {
            return false;
        }
        self.last_hal_cipher_reload_attempt = Some(now);

        match reader.reload_cipher() {
            Ok(()) => {
                log::info!("[Decoder Thread] Reloaded HAL input cipher after key change");
                true
            }
            Err(e) => {
                log::warn!("[Decoder Thread] HAL input cipher reload failed: {}", e);
                false
            }
        }
    }

    /// Start silent source mode (for HAL input plugins)
    fn start_silent_source(&mut self, channels: usize) {
        self.stop(); // Clear any existing decoder
        self.silent_source = true;
        self.silent_source_channels = channels.max(1);

        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            self.try_reconnect_hal_reader(true);
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
        #[cfg_attr(
            not(all(target_os = "macos", feature = "hal")),
            allow(unused_variables)
        )]
        response_tx: &Sender<DecoderResponse>,
        frame_size: usize,
        target_sample_rate: u32,
    ) -> Result<(bool, Option<DecoderCommand>), String> {
        // Static counter for periodic logging (avoid log spam)
        static LOG_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let count = LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        #[cfg(all(target_os = "macos", feature = "hal"))]
        self.try_reconnect_hal_reader(false);

        #[cfg(all(target_os = "macos", feature = "hal"))]
        if self.hal_reader.is_some() && !self.reload_hal_cipher_if_needed() {
            return Ok((false, None));
        }

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

            let frames_read = reader.read(&mut self.hal_input_buffer);
            let samples_read = frames_to_sample_count(frames_read, hal_channels, buffer_len);

            if samples_read < buffer_len {
                self.hal_input_buffer[samples_read..].fill(0.0);
            }

            if let Some(trip) = guard_hal_input_block(&mut self.hal_input_buffer[..buffer_len]) {
                static HAL_INPUT_GUARD_LOG_COUNTER: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let guard_count = HAL_INPUT_GUARD_LOG_COUNTER
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    + 1;
                if guard_count == 1 || guard_count.is_multiple_of(100) {
                    log::warn!(
                        "[Decoder Thread] HAL input guard dropped runaway/corrupt block: peak={:.3}, invalid_samples={}, over_limit_samples={}, trip_count={}",
                        trip.peak,
                        trip.invalid_samples,
                        trip.over_limit_samples,
                        guard_count
                    );
                }
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

                let resampler = self.resampler.as_mut().ok_or(
                    "Decoder invariant violated: HAL resampler missing after configuration",
                )?;
                let max_output_frames = resampler.output_frames_for_input(frame_size);
                let output_len = max_output_frames * hal_channels;

                if self.resample_output_buffer.len() < output_len {
                    self.resample_output_buffer.resize(output_len, 0.0);
                }

                let context = ProcessContext::new(hal_sample_rate, frame_size);

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

                let frame_data = take_frame_buffer(
                    &mut self.frame_send_buffer,
                    &self.recycle_rx,
                    &mut self.frame_buffer_pool,
                    frame_len,
                );

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
                match self.send_decoder_message(
                    message_tx,
                    command_rx,
                    response_tx,
                    DecoderMessage::Frame(frame),
                ) {
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

                if self.frame_send_buffer.len() < buffer_len {
                    self.frame_send_buffer.resize(buffer_len, 0.0);
                }
                self.frame_send_buffer[..buffer_len]
                    .copy_from_slice(&self.hal_input_buffer[..buffer_len]);
                let frame_data = take_frame_buffer(
                    &mut self.frame_send_buffer,
                    &self.recycle_rx,
                    &mut self.frame_buffer_pool,
                    buffer_len,
                );

                // Send frame with HAL sample rate (which matches target)
                let frame = AudioFrame::new(frame_data, frame_size, hal_channels, hal_sample_rate);

                match self.send_decoder_message(
                    message_tx,
                    command_rx,
                    response_tx,
                    DecoderMessage::Frame(frame),
                ) {
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
        let silent_channels = self.silent_source_channels.max(1);
        let silent_len = frame_size * silent_channels;
        if self.frame_send_buffer.len() < silent_len {
            self.frame_send_buffer.resize(silent_len, 0.0);
        }
        self.frame_send_buffer[..silent_len].fill(0.0);

        let frame_data = take_frame_buffer(
            &mut self.frame_send_buffer,
            &self.recycle_rx,
            &mut self.frame_buffer_pool,
            silent_len,
        );

        let frame = AudioFrame::new(frame_data, frame_size, silent_channels, target_sample_rate);
        if let Some(cmd) = self.send_decoder_message(
            message_tx,
            command_rx,
            response_tx,
            DecoderMessage::Frame(frame),
        )? {
            return Ok((false, Some(cmd)));
        }

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
    dsd_output: DsdOutputMode,
) -> Result<(), String> {
    let mut state = DecoderState::new(recycle_rx, dsd_output);

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
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
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
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
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
                DecoderCommand::StartSilentSource(channels) => {
                    state.start_silent_source(channels);
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
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
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

        #[cfg(feature = "streaming")]
        state.poll_stream_metadata(&event_tx);

        // Generate frames based on mode
        if state.silent_source && !state.paused {
            // HAL Input / Silent Source mode
            match state.process_hal_input(
                &message_tx,
                &command_rx,
                &response_tx,
                frame_size,
                target_sample_rate,
            ) {
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
                                #[cfg(feature = "streaming")]
                                state.clear_stream_metadata(&event_tx);
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
                                #[cfg(feature = "streaming")]
                                state.clear_stream_metadata(&event_tx);
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
                                #[cfg(feature = "streaming")]
                                state.clear_stream_metadata(&event_tx);
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
                            DecoderCommand::StartSilentSource(channels) => {
                                state.start_silent_source(channels);
                            }
                            DecoderCommand::QueueNext(path) => {
                                log::debug!(
                                    "[Decoder Thread] Queued next (from HAL interrupt): {:?}",
                                    path
                                );
                                state.queued_next = Some(path);
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                            DecoderCommand::CancelNext => {
                                log::debug!(
                                    "[Decoder Thread] Cancelled queued next (from HAL interrupt)"
                                );
                                state.queued_next = None;
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                        }
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                Err(e) => {
                    crate::rate_limited_log!(warn, 5, "[Decoder Thread] HAL input error: {}", e);
                    state.stop();
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
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
                &response_tx,
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
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
                }
                Ok(DecoderLoopAction::Interrupted(cmd)) => {
                    // Handle interruption command immediately
                    match cmd {
                        DecoderCommand::Play(path) => {
                            message_tx.send(DecoderMessage::Flush).ok();
                            state.stop();
                            #[cfg(feature = "streaming")]
                            state.clear_stream_metadata(&event_tx);
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
                            #[cfg(feature = "streaming")]
                            state.clear_stream_metadata(&event_tx);
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
                        DecoderCommand::StartSilentSource(channels) => {
                            state.start_silent_source(channels);
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
                            log::debug!(
                                "[Decoder Thread] Queued next (from interrupt): {:?}",
                                path
                            );
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
                            #[cfg(feature = "streaming")]
                            state.clear_stream_metadata(&event_tx);
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
                    crate::rate_limited_log!(warn, 5, "[Decoder Thread] Error: {}", e);
                    state.stop();
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
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
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn send_or_interrupt_returns_unsent_message_with_interrupting_command() {
        let (message_tx, message_rx) = std::sync::mpsc::sync_channel(1);
        message_tx.send(DecoderMessage::EndOfStream).unwrap();

        let (command_tx, command_rx) = std::sync::mpsc::channel();
        command_tx
            .send(DecoderCommand::QueueNext(PathBuf::from("next.wav").into()))
            .unwrap();

        let result = send_or_interrupt(&message_tx, &command_rx, DecoderMessage::Flush).unwrap();
        let Some((DecoderCommand::QueueNext(_), pending)) = result else {
            panic!("expected QueueNext command with the unsent message");
        };

        assert!(matches!(pending, DecoderMessage::Flush));
        assert!(matches!(
            message_rx.try_recv().unwrap(),
            DecoderMessage::EndOfStream
        ));
    }

    #[test]
    fn send_or_interrupt_errors_when_queue_stays_full() {
        let (message_tx, _message_rx) = std::sync::mpsc::sync_channel(1);
        message_tx.send(DecoderMessage::EndOfStream).unwrap();

        let (_command_tx, command_rx) = std::sync::mpsc::channel();

        let result = send_or_interrupt(&message_tx, &command_rx, DecoderMessage::Flush);

        assert!(result.unwrap_err().contains("queue stuck"));
    }

    #[test]
    fn silent_source_fallback_send_is_interruptible_when_queue_full() {
        let (message_tx, message_rx) = std::sync::mpsc::sync_channel(1);
        message_tx.send(DecoderMessage::Flush).unwrap();
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let (response_tx, _response_rx) = std::sync::mpsc::channel();
        command_tx.send(DecoderCommand::Stop).unwrap();

        let (_recycle_tx, recycle_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut state = DecoderState::new(recycle_rx, DsdOutputMode::Disabled);
            state.start_silent_source(2);
            let result =
                state.process_hal_input(&message_tx, &command_rx, &response_tx, 16, 48_000);
            done_tx.send(result).ok();
        });

        let result = done_rx.recv_timeout(std::time::Duration::from_millis(250));
        drop(message_rx);
        let result = result.expect("silent-source send blocked instead of honoring Stop command");
        let (_sent, cmd) = result.expect("silent-source processing failed");
        assert!(matches!(cmd, Some(DecoderCommand::Stop)));
    }

    #[test]
    fn decoder_frame_buffer_handoff_has_no_allocation_fallback() {
        let source = include_str!("decoder_thread.rs");
        assert!(
            !source.contains(concat!("Err(_) => Vec::", "with_capacity(len)")),
            "decoder frame handoff must use preallocated/recycled buffers instead of allocating on recycle miss"
        );
    }

    #[test]
    fn hal_input_reloads_stale_cipher_before_reading() {
        let source = include_str!("decoder_thread.rs");
        let reload_call = source
            .find("!self.reload_hal_cipher_if_needed()")
            .expect("HAL input path should refresh stale encryption ciphers");
        let read_call = source
            .find("let frames_read = reader.read(&mut self.hal_input_buffer)")
            .expect("HAL input path should read from the shared-memory reader");

        assert!(
            source.contains("reader.needs_cipher_reload()")
                && source.contains("reader.reload_cipher()"),
            "decoder should detect and reload stale HAL input ciphers"
        );
        assert!(
            reload_call < read_call,
            "decoder must reload a stale cipher before reading, otherwise key rotation yields silence"
        );
    }

    #[test]
    fn sample_queue_consume_advances_cursor_without_front_memmove() {
        let mut queue = SampleQueue::new();
        queue.extend_from_slice(&[0.0, 1.0, 2.0, 3.0]);

        let original_ptr = queue.as_slice().as_ptr();
        queue.consume(2);

        assert_eq!(queue.as_slice(), &[2.0, 3.0]);
        assert_eq!(queue.as_slice().as_ptr(), unsafe { original_ptr.add(2) });
    }

    #[test]
    fn hal_read_frame_count_converts_to_sample_count() {
        let frame_size = 1024;
        let channels = 2;
        let buffer_len = frame_size * channels;

        assert_eq!(
            frames_to_sample_count(frame_size, channels, buffer_len),
            buffer_len
        );
        assert_eq!(frames_to_sample_count(192, channels, buffer_len), 384);
        assert_eq!(
            frames_to_sample_count(frame_size + 1, channels, buffer_len),
            buffer_len
        );
    }

    #[test]
    fn hal_input_guard_allows_normal_float_pcm() {
        let mut samples = vec![-1.25, -0.5, 0.0, 0.5, 1.25, 2.0];

        let trip = guard_hal_input_block(&mut samples);

        assert_eq!(trip, None);
        assert_eq!(samples, vec![-1.25, -0.5, 0.0, 0.5, 1.25, 2.0]);
    }

    #[test]
    fn hal_input_guard_silences_impossible_peak() {
        let mut samples = vec![0.1, -0.2, 36.4, 0.3];

        let trip = guard_hal_input_block(&mut samples).expect("guard should trip");

        assert_eq!(trip.invalid_samples, 0);
        assert_eq!(trip.over_limit_samples, 1);
        assert_eq!(trip.peak, 36.4);
        assert!(samples.iter().all(|&sample| sample == 0.0));
    }

    #[test]
    fn hal_input_guard_silences_non_finite_samples() {
        let mut samples = vec![0.1, f32::NAN, f32::INFINITY, -0.2];

        let trip = guard_hal_input_block(&mut samples).expect("guard should trip");

        assert_eq!(trip.invalid_samples, 2);
        assert_eq!(trip.over_limit_samples, 0);
        assert_eq!(trip.peak, 0.2);
        assert!(samples.iter().all(|&sample| sample == 0.0));
    }

    #[test]
    fn silent_source_fallback_uses_configured_channel_count() {
        let (_recycle_tx, recycle_rx) = std::sync::mpsc::channel();
        let mut state = DecoderState::new(recycle_rx, DsdOutputMode::Disabled);
        state.start_silent_source(6);

        let (message_tx, message_rx) = std::sync::mpsc::sync_channel(1);
        let (_command_tx, command_rx) = std::sync::mpsc::channel();
        let (response_tx, _response_rx) = std::sync::mpsc::channel();

        let (processed, pending) = state
            .process_hal_input(&message_tx, &command_rx, &response_tx, 128, 48_000)
            .unwrap();

        assert!(processed);
        assert!(pending.is_none());
        let DecoderMessage::Frame(frame) = message_rx.try_recv().unwrap() else {
            panic!("expected silent audio frame");
        };
        assert_eq!(frame.num_frames, 128);
        assert_eq!(frame.num_channels, 6);
        assert_eq!(frame.data.len(), 128 * 6);
        assert!(frame.data.iter().all(|&sample| sample == 0.0));
    }

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

        let mut staging = SampleQueue::new();
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
                staging.consume(send_chunk_len);
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
            staging.consume(send_chunk_len);
            emitted_blocks += 1;
        }
        assert_eq!(
            emitted_blocks, 4,
            "Expected a fourth full block after the fifth chunk"
        );
    }
}
