use super::super::{AudioFrame, DecoderCommand, DecoderMessage, DecoderResponse, ThreadEvent};
#[cfg(all(target_os = "macos", feature = "hal"))]
use super::consts::HAL_RECONNECT_INTERVAL;
use super::consts::send_or_interrupt;
use super::consts::{
    DECODER_LOCAL_FRAME_CAPACITY, DECODER_LOCAL_FRAME_POOL_SIZE, MAX_ENGINE_SAMPLE_CAPACITY,
    MAX_RESAMPLE_OUTPUT_SAMPLES, MAX_RESAMPLE_STAGING_SAMPLES,
};
#[cfg(all(target_os = "macos", feature = "hal"))]
use super::hal_input_guard_trip::guard_hal_input_block;
#[cfg(all(target_os = "macos", feature = "hal"))]
use super::misc::frames_to_sample_count;
use super::misc::take_frame_buffer;
use super::sample_queue::SampleQueue;
use super::types::DecoderLoopAction;
use crate::DsdOutputMode;
#[cfg(feature = "streaming")]
use crate::StreamMetadata;
use crate::decoder::{
    AudioDecoder, AudioSource, AudioSpec, DecodedAudio,
    create_decoder_from_source_with_dsd_mode_and_metadata,
};
#[cfg(all(target_os = "macos", feature = "hal"))]
use driver_hal::HalInputReader;
use sotf_plugins::{Plugin, ProcessContext, ResamplerPlugin};
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::time::{Duration, Instant};

const POSITION_UPDATE_INTERVAL: Duration = Duration::from_millis(100);

/// Decoder state
pub(super) struct DecoderState {
    pub(super) decoder: Option<Box<dyn AudioDecoder>>,
    pub(super) resampler: Option<ResamplerPlugin>,
    pub(super) resampler_buffer: SampleQueue,
    pub(super) paused: bool,
    pub(super) current_source: Option<AudioSource>,
    pub(super) spec: Option<AudioSpec>,
    pub(super) silent_source: bool, // For HAL input plugins (no file source)
    pub(super) silent_source_channels: usize,
    pub(super) decode_buffer: Option<DecodedAudio>,
    pub(super) resample_output_buffer: Vec<f32>,
    /// Staging buffer: accumulates resampled output across decode chunks so that
    /// only complete `frame_size`-frame blocks are forwarded to the processing thread.
    /// Without this, a 48 kHz source resampled to 44.1 kHz produces ~940-frame blocks
    /// which are smaller than the upmixer's hop size (1024), causing the upmixer to
    /// never fire an FFT block and produce silence.
    pub(super) resample_staging: SampleQueue,
    /// Pre-allocated buffer for chunk processing (avoids allocation in hot path)
    pub(super) chunk_buffer: Vec<f32>,
    /// Pre-allocated buffer for frame sending (avoids allocation in hot path)
    pub(super) frame_send_buffer: Vec<f32>,
    /// Local spare buffers used when the recycle queue is temporarily empty.
    pub(super) frame_buffer_pool: Vec<Vec<f32>>,
    /// Receives recycled Vec<f32> buffers from the processing thread
    pub(super) recycle_rx: Receiver<Vec<f32>>,
    /// Queued next source for gapless playback. When set and the current source ends,
    /// the decoder seamlessly transitions to this source without sending EndOfStream/Flush.
    pub(super) queued_next: Option<AudioSource>,
    /// Throttles unbounded manager events and their per-send node allocations.
    pub(super) last_position_update: Instant,
    pub(super) dsd_output: DsdOutputMode,
    #[cfg(feature = "streaming")]
    pub(super) stream_metadata_rx: Option<crate::decoder::SourceMetadataReceiver>,
    #[cfg(feature = "streaming")]
    pub(super) stream_metadata: Option<StreamMetadata>,

    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) hal_input_buffer: Vec<f32>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) hal_reader: Option<HalInputReader>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) last_hal_reconnect_attempt: Option<Instant>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) last_hal_cipher_reload_attempt: Option<Instant>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) last_hal_sample_rate: Option<u32>,
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) last_hal_channels: Option<usize>,
}

impl DecoderState {
    pub(super) fn new(recycle_rx: Receiver<Vec<f32>>, dsd_output: DsdOutputMode) -> Self {
        let mut frame_buffer_pool = Vec::with_capacity(DECODER_LOCAL_FRAME_POOL_SIZE);
        for _ in 0..DECODER_LOCAL_FRAME_POOL_SIZE {
            frame_buffer_pool.push(Vec::with_capacity(DECODER_LOCAL_FRAME_CAPACITY));
        }

        Self {
            decoder: None,
            resampler: None,
            // Accumulates decoded samples; sized for several max-size blocks so
            // the resampling loop rarely needs to grow it.
            resampler_buffer: SampleQueue::with_capacity(MAX_ENGINE_SAMPLE_CAPACITY * 2),
            paused: false,
            current_source: None,
            spec: None,
            silent_source: false,
            silent_source_channels: 2,
            decode_buffer: None,
            // Sized for the worst common resampling ratio; `ensure_buffer_len`
            // will grow it if an oversized block is requested.
            resample_output_buffer: Vec::with_capacity(MAX_RESAMPLE_OUTPUT_SAMPLES),
            // Holds several complete max-size blocks to absorb resampler jitter.
            resample_staging: SampleQueue::with_capacity(MAX_RESAMPLE_STAGING_SAMPLES),
            // Pre-allocated for the worst-case engine block (frames * channels).
            chunk_buffer: Vec::with_capacity(MAX_ENGINE_SAMPLE_CAPACITY),
            frame_send_buffer: Vec::with_capacity(MAX_ENGINE_SAMPLE_CAPACITY),
            frame_buffer_pool,
            recycle_rx,
            queued_next: None,
            last_position_update: Instant::now()
                .checked_sub(POSITION_UPDATE_INTERVAL)
                .unwrap_or_else(Instant::now),
            dsd_output,
            #[cfg(feature = "streaming")]
            stream_metadata_rx: None,
            #[cfg(feature = "streaming")]
            stream_metadata: None,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            hal_input_buffer: Vec::with_capacity(MAX_ENGINE_SAMPLE_CAPACITY),
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

    /// Resize `buffer` within its pre-allocated capacity.
    /// After this call `buffer.len() >= len`; existing contents are preserved.
    /// Oversized blocks are reported instead of allocating on the decoder hot path.
    #[inline]
    pub(super) fn ensure_buffer_len(buffer: &mut Vec<f32>, len: usize) -> Result<(), String> {
        if buffer.capacity() < len {
            return Err(format!(
                "Decoder scratch buffer capacity {} is too small for {} samples",
                buffer.capacity(),
                len
            ));
        }
        if buffer.len() < len {
            buffer.resize(len, 0.0);
        }
        Ok(())
    }

    /// Move a freshly decoded allocation into the sample queue when no
    /// residual samples are buffered; append only when block boundaries span
    /// decoder packets. Returns true when ownership was transferred.
    pub(super) fn queue_decoded_samples(&mut self) -> Result<bool, String> {
        let decoded_samples = &mut self
            .decode_buffer
            .as_mut()
            .ok_or("Decoder invariant violated: decode buffer missing after decode")?
            .samples;
        if self.resampler_buffer.is_empty() {
            self.resampler_buffer.clear();
            std::mem::swap(&mut self.resampler_buffer.data, decoded_samples);
            Ok(true)
        } else {
            self.resampler_buffer.extend_from_slice(decoded_samples);
            Ok(false)
        }
    }

    /// Transfer an exact, unconsumed decoder block directly to the next
    /// pipeline stage instead of copying it through `frame_send_buffer`.
    pub(super) fn take_exact_decoded_block(&mut self, len: usize) -> Option<Vec<f32>> {
        if self.resampler_buffer.start != 0 || self.resampler_buffer.data.len() != len {
            return None;
        }
        let data = take_frame_buffer(
            &mut self.resampler_buffer.data,
            &self.recycle_rx,
            &mut self.frame_buffer_pool,
            len,
        );
        self.resampler_buffer.start = 0;
        Some(data)
    }

    /// Start playing a new audio source
    pub(super) fn play(
        &mut self,
        source: AudioSource,
        target_sample_rate: u32,
        frame_size: usize,
    ) -> Result<(), String> {
        self.last_position_update = Instant::now()
            .checked_sub(POSITION_UPDATE_INTERVAL)
            .unwrap_or_else(Instant::now);
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

    pub(super) fn try_gapless_transition_preserving_buffers(
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
    pub(super) fn clear_stream_metadata(
        &mut self,
        event_tx: &crossbeam::channel::Sender<ThreadEvent>,
    ) {
        self.stream_metadata_rx = None;
        if self.stream_metadata.take().is_some() {
            event_tx
                .try_send(ThreadEvent::StreamMetadataChanged(None))
                .ok();
        }
    }

    #[cfg(feature = "streaming")]
    pub(super) fn poll_stream_metadata(
        &mut self,
        event_tx: &crossbeam::channel::Sender<ThreadEvent>,
    ) {
        use std::sync::mpsc::TryRecvError;

        let mut changed = false;
        while let Some(rx) = self.stream_metadata_rx.as_ref() {
            let event = match rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.stream_metadata_rx = None;
                    break;
                }
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
                .try_send(ThreadEvent::StreamMetadataChanged(
                    self.stream_metadata.clone(),
                ))
                .ok();
        }
    }

    pub(super) fn send_decoder_message(
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

    #[allow(
        clippy::too_many_arguments,
        reason = "decoder message helper: one argument per frame metadata field"
    )]
    pub(super) fn send_prepared_frame(
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
        let frame = AudioFrame::try_new(frame_data, num_frames, channels, sample_rate)
            .map_err(|error| format!("decoder produced an invalid output frame: {error}"))?;
        self.send_decoder_message(
            message_tx,
            command_rx,
            response_tx,
            DecoderMessage::Frame(frame),
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "decoder message helper: one argument per frame metadata field"
    )]
    pub(super) fn emit_resample_staging(
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
            Self::ensure_buffer_len(&mut self.frame_send_buffer, send_chunk_len)?;
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
                Self::ensure_buffer_len(&mut self.frame_send_buffer, aligned_len)?;
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
    pub(super) fn decode_chunk(
        &mut self,
        message_tx: &SyncSender<DecoderMessage>,
        command_rx: &Receiver<DecoderCommand>,
        response_tx: &Sender<DecoderResponse>,
        event_tx: &crossbeam::channel::Sender<ThreadEvent>,
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
                self.queue_decoded_samples()?;

                // Process buffer in frame_size chunks
                while self.resampler_buffer.len() >= frame_size * channels {
                    // If resampling, we need enough input samples for one output frame
                    // But here we simplify: just process fixed input chunks

                    let s_start = Instant::now();
                    let chunk_len = frame_size * channels;

                    if let Some(resampler) = &mut self.resampler {
                        // Copy chunk to pre-allocated buffer.
                        Self::ensure_buffer_len(&mut self.chunk_buffer, chunk_len)?;
                        self.chunk_buffer[..chunk_len]
                            .copy_from_slice(self.resampler_buffer.prefix(chunk_len));
                        self.resampler_buffer.consume(chunk_len);

                        // Resample
                        let max_output_frames = resampler.output_frames_for_input(frame_size);
                        let output_len = max_output_frames * channels;

                        Self::ensure_buffer_len(&mut self.resample_output_buffer, output_len)?;

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
                        // Exact one-block decoder packets can transfer their
                        // allocation directly to the pipeline. Partial or
                        // multi-block packets retain the queue/copy path.
                        let interrupted =
                            if let Some(frame_data) = self.take_exact_decoded_block(chunk_len) {
                                let frame = AudioFrame::try_new(
                                    frame_data,
                                    frame_size,
                                    channels,
                                    source_sample_rate,
                                )
                                .map_err(|error| {
                                    format!("decoder produced an invalid direct frame: {error}")
                                })?;
                                self.send_decoder_message(
                                    message_tx,
                                    command_rx,
                                    response_tx,
                                    DecoderMessage::Frame(frame),
                                )?
                            } else {
                                Self::ensure_buffer_len(&mut self.frame_send_buffer, chunk_len)?;
                                self.frame_send_buffer[..chunk_len]
                                    .copy_from_slice(self.resampler_buffer.prefix(chunk_len));
                                self.resampler_buffer.consume(chunk_len);
                                self.send_prepared_frame(
                                    message_tx,
                                    command_rx,
                                    response_tx,
                                    chunk_len,
                                    frame_size,
                                    channels,
                                    source_sample_rate,
                                )?
                            };

                        if let Some(cmd) = interrupted {
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
                if position_update_due(self.last_position_update.elapsed()) {
                    event_tx
                        .try_send(ThreadEvent::PositionUpdate(position_sec))
                        .ok();
                    self.last_position_update = Instant::now();
                }

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
                            .try_send(ThreadEvent::DecoderGaplessTransition(next_source))
                            .ok();
                        return Ok(DecoderLoopAction::Continue);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let msg = format!("Gapless transition failed: {}", e);
                        log::warn!("[Decoder Thread] {}", msg);
                        event_tx.try_send(ThreadEvent::DecoderError(msg)).ok();
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
                        Self::ensure_buffer_len(&mut self.chunk_buffer, padded_len)?;
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

                        Self::ensure_buffer_len(&mut self.resample_output_buffer, output_len)?;

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
                        Self::ensure_buffer_len(&mut self.frame_send_buffer, aligned_len)?;
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
                                .try_send(ThreadEvent::DecoderGaplessTransition(next_source))
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
                            event_tx.try_send(ThreadEvent::DecoderError(msg)).ok();
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

                // Position updates are normally rate-limited to avoid flooding the
                // manager thread. Emit one final update at EOF so playback state does
                // not remain at the last periodic update while the output drains.
                let final_position_sec = self
                    .decoder
                    .as_ref()
                    .map(|decoder| decoder.position() as f64 / source_sample_rate as f64)
                    .unwrap_or(0.0);
                event_tx
                    .try_send(ThreadEvent::PositionUpdate(final_position_sec))
                    .ok();
                self.last_position_update = Instant::now();
                event_tx.try_send(ThreadEvent::DecoderEndOfStream).ok();
                Ok(DecoderLoopAction::Stop)
            }
            Err(e) => {
                let err_msg = format!("Decode error: {:?}", e);
                event_tx
                    .try_send(ThreadEvent::DecoderError(err_msg.clone()))
                    .ok();
                Err(err_msg)
            }
            Ok(_) => unreachable!("decode_into returned negative frames?"),
        }
    }

    /// Seek to position in seconds
    pub(super) fn seek(&mut self, position: f64) -> Result<(), String> {
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
    pub(super) fn stop(&mut self) {
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
    pub(super) fn try_reconnect_hal_reader(&mut self, force: bool) {
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
    pub(super) fn reload_hal_cipher_if_needed(&mut self) -> bool {
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
    pub(super) fn start_silent_source(&mut self, channels: usize) {
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
    pub(super) fn process_hal_input(
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

            // Keep the HAL input buffer exactly `buffer_len`: truncate any stale
            // tail and zero-fill any grown region before reading.
            Self::ensure_buffer_len(&mut self.hal_input_buffer, buffer_len)?;
            self.hal_input_buffer.truncate(buffer_len);

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

                Self::ensure_buffer_len(&mut self.resample_output_buffer, output_len)?;

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
                Self::ensure_buffer_len(&mut self.frame_send_buffer, frame_len)?;
                self.frame_send_buffer[..frame_len]
                    .copy_from_slice(&self.resample_output_buffer[..frame_len]);

                let frame_data = take_frame_buffer(
                    &mut self.frame_send_buffer,
                    &self.recycle_rx,
                    &mut self.frame_buffer_pool,
                    frame_len,
                );

                // Send frame with TARGET sample rate
                let frame = AudioFrame::try_new(
                    frame_data,
                    actual_output_frames,
                    hal_channels,
                    target_sample_rate,
                )
                .map_err(|error| format!("HAL resampler produced an invalid frame: {error}"))?;

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

                Self::ensure_buffer_len(&mut self.frame_send_buffer, buffer_len)?;
                self.frame_send_buffer[..buffer_len]
                    .copy_from_slice(&self.hal_input_buffer[..buffer_len]);
                let frame_data = take_frame_buffer(
                    &mut self.frame_send_buffer,
                    &self.recycle_rx,
                    &mut self.frame_buffer_pool,
                    buffer_len,
                );

                // Send frame with HAL sample rate (which matches target)
                let frame =
                    AudioFrame::try_new(frame_data, frame_size, hal_channels, hal_sample_rate)
                        .map_err(|error| format!("HAL input produced an invalid frame: {error}"))?;

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
        Self::ensure_buffer_len(&mut self.frame_send_buffer, silent_len)?;
        self.frame_send_buffer[..silent_len].fill(0.0);

        let frame_data = take_frame_buffer(
            &mut self.frame_send_buffer,
            &self.recycle_rx,
            &mut self.frame_buffer_pool,
            silent_len,
        );

        let frame =
            AudioFrame::try_new(frame_data, frame_size, silent_channels, target_sample_rate)
                .map_err(|error| format!("silent source produced an invalid frame: {error}"))?;
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

fn position_update_due(elapsed: Duration) -> bool {
    elapsed >= POSITION_UPDATE_INTERVAL
}

#[cfg(test)]
mod position_update_tests {
    use super::{POSITION_UPDATE_INTERVAL, position_update_due};
    use std::time::Duration;

    #[test]
    fn position_updates_are_limited_to_ten_hz() {
        assert!(!position_update_due(Duration::from_millis(99)));
        assert!(position_update_due(POSITION_UPDATE_INTERVAL));
        assert!(position_update_due(Duration::from_millis(150)));
    }
}
