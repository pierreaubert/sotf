use super::super::{
    DecoderMessage, ProcessingCommand, ProcessingMessage, ProcessingResponse, ThreadEvent,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::isolated::isolated_external_plugin_status;
use super::misc::send_or_interrupt;
use sotf_plugins::{Host, PluginHost};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, SyncSender};

/// Short wait used while audio is active and the decoder queue briefly runs dry.
const ACTIVE_EMPTY_SLEEP_PROCESSING_US: u64 = 100;
/// Coarser wait once playback has explicitly stopped or reached end of stream.
const IDLE_EMPTY_SLEEP_PROCESSING_MS: u64 = 1;

/// Processing state
pub(super) struct ProcessingState {
    /// Current plugin host
    pub(super) host: PluginHost,
    /// Previous host for crossfading
    pub(super) prev_host: Option<PluginHost>,
    /// Crossfade progress (0.0 to 1.0, 1.0 = current host only)
    pub(super) crossfade_progress: f32,
    /// Crossfade step per frame
    pub(super) crossfade_step: f32,
    /// Number of channels
    pub(super) channels: usize,
    pub(super) bypassed: bool,
    pub(super) process_buffer: Vec<f32>,
    /// Buffer for previous host during crossfade
    pub(super) prev_process_buffer: Vec<f32>,
    /// Frame counter for diagnostic logging
    pub(super) frame_count: u64,
    /// Total output samples produced (for effective rate measurement)
    pub(super) total_output_samples: u64,
    /// Timestamp of first frame processed
    pub(super) first_frame_time: Option<std::time::Instant>,
    /// Sample rate (for effective rate calculation)
    pub(super) sample_rate: u32,
    /// Spare Arc from previous plugin_data_cache swap, reused via Arc::get_mut
    /// to avoid per-frame Vec allocation when no UI reader holds a reference.
    pub(super) spare_cache_arc: Option<std::sync::Arc<super::super::PluginDataVec>>,
    /// RT diagnostics: how many frames hit the cache fallback (allocation) path
    pub(super) cache_fallback_count: u64,
    /// RT diagnostics: how many frames reused the spare Arc (zero-alloc fast path)
    pub(super) cache_reuse_count: u64,
    /// RT diagnostics: max process_frame duration in the current reporting window
    pub(super) max_frame_duration: std::time::Duration,
    /// RT diagnostics: last time we logged diagnostics
    pub(super) last_rt_diag: std::time::Instant,
    /// RT diagnostics: how many frames took longer than the frame period
    pub(super) frames_over_budget: u64,
    /// RT diagnostics: how many recycle misses (fallback Vec allocation)
    pub(super) recycle_miss_count: u64,
    /// Optional nonblocking tap for live network PCM streaming.
    #[cfg(feature = "streaming")]
    pub(super) network_stream_tap: Option<sotf_streaming::PcmStreamHandle>,
}

impl ProcessingState {
    pub(super) fn new(
        channels: usize,
        sample_rate: u32,
        #[cfg(feature = "streaming")] network_stream_tap: Option<sotf_streaming::PcmStreamHandle>,
    ) -> Self {
        Self {
            host: PluginHost::new(channels, sample_rate),
            prev_host: None,
            crossfade_progress: 1.0,
            crossfade_step: 0.0,
            channels,
            bypassed: false,
            process_buffer: Vec::new(),
            prev_process_buffer: Vec::new(),
            frame_count: 0,
            total_output_samples: 0,
            first_frame_time: None,
            sample_rate,
            spare_cache_arc: Some(Arc::new(Vec::new())),
            cache_fallback_count: 0,
            cache_reuse_count: 0,
            max_frame_duration: std::time::Duration::ZERO,
            last_rt_diag: std::time::Instant::now(),
            frames_over_budget: 0,
            recycle_miss_count: 0,
            #[cfg(feature = "streaming")]
            network_stream_tap,
        }
    }

    /// Get the actual output channel count
    pub(super) fn output_channels(&self) -> usize {
        self.host.output_channels()
    }

    /// Get the output frame count for a given input frame count.
    /// Accounts for plugins that change frame count (like resamplers).
    pub(super) fn output_frames_for_input(&self, input_frames: usize) -> usize {
        if self.bypassed || self.host.plugin_count() == 0 {
            input_frames
        } else {
            self.host.output_frames_for_input(input_frames)
        }
    }

    /// Get the output sample rate for a given input rate.
    /// Accounts for plugins that change sample rate (like resamplers).
    pub(super) fn output_sample_rate(&self, input_rate: u32) -> u32 {
        if self.bypassed || self.host.plugin_count() == 0 {
            input_rate
        } else {
            self.host.output_sample_rate(input_rate)
        }
    }

    pub(super) fn compute_crossfade_step(input_frames: usize, sample_rate: u32) -> f32 {
        if input_frames == 0 {
            return 1.0;
        }

        let crossfade_duration_ms = 50.0;
        let block_duration_ms = (input_frames as f32 * 1000.0) / sample_rate as f32;
        (block_duration_ms / crossfade_duration_ms).min(0.5)
    }

    pub(super) fn prepare_scratch_buffer(buffer: &mut Vec<f32>, len: usize) {
        if buffer.len() != len {
            buffer.resize(len, 0.0);
        }
    }

    pub(super) fn fade_in_unblended_tail(
        output: &mut [f32],
        blend_samples: usize,
        output_samples: usize,
        alpha: f32,
    ) {
        if output_samples <= blend_samples {
            return;
        }

        for sample in &mut output[blend_samples..output_samples] {
            *sample *= alpha;
        }
    }

    /// Process a frame
    /// Returns the actual number of output frames written
    pub(super) fn process_frame(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        input_frames: usize,
    ) -> Result<usize, String> {
        if self.bypassed {
            // Bypass — copy input to output. When the plugin chain changes
            // channel count (e.g. upmixer 2ch→5ch), output is sized for the
            // host's output_channels while input has input_channels.
            // Copy what fits, zero-fill the rest to avoid a panic.
            let copy_len = input.len().min(output.len());
            output[..copy_len].copy_from_slice(&input[..copy_len]);
            if copy_len < output.len() {
                output[copy_len..].fill(0.0);
            }
            return Ok(input_frames);
        }

        // Handle crossfade if in progress
        if let Some(ref mut prev_host) = self.prev_host {
            let actual_frames = self.host.process(input, output)?;

            // Size prev_process_buffer to match output (not actual_frames from
            // new host), so prev_host has enough room even if it produces a
            // slightly different frame count.
            let buf_len = output.len();
            Self::prepare_scratch_buffer(&mut self.prev_process_buffer, buf_len);

            let output_samples = actual_frames * self.channels;
            let prev_actual = prev_host.process(input, &mut self.prev_process_buffer[..buf_len])?;
            let blend_samples = output_samples.min(prev_actual * self.channels);

            // Compute crossfade step from actual frame size (~50ms crossfade)
            if self.crossfade_step == 0.0 {
                self.crossfade_step = Self::compute_crossfade_step(input_frames, self.sample_rate);
            }

            // Blend buffers: output = (1-alpha)*prev + alpha*current
            let alpha = self.crossfade_progress;
            sotf_plugins::simd::blend_simd(
                &mut output[..blend_samples],
                &self.prev_process_buffer[..blend_samples],
                alpha,
            );
            Self::fade_in_unblended_tail(output, blend_samples, output_samples, alpha);

            // Advance crossfade
            self.crossfade_progress = (self.crossfade_progress + self.crossfade_step).min(1.0);
            if self.crossfade_progress >= 1.0 {
                self.prev_host = None;
            }

            Ok(actual_frames)
        } else {
            // Normal processing - returns actual output frames
            self.host.process(input, output)
        }
    }
}

/// Handle a processing command
/// Returns true if shutdown requested
pub(super) fn handle_processing_command(
    command: ProcessingCommand,
    state: &mut ProcessingState,
    response_tx: &Sender<ProcessingResponse>,
    event_tx: &Sender<ThreadEvent>,
) -> bool {
    let _ = event_tx;
    match command {
        ProcessingCommand::UpdateHost(new_host) => {
            let new_host = *new_host;
            let output_channels = new_host.output_channels();
            log::trace!(
                "[Processing Thread] UpdateHost: Plugin host updated, output_channels={}",
                output_channels
            );

            // Initiate crossfade if channel counts match and we have an existing chain
            if state.host.output_channels() == output_channels && state.host.plugin_count() > 0 {
                state.prev_host = Some(std::mem::replace(&mut state.host, new_host));
                state.crossfade_progress = 0.0;

                // crossfade_step is computed lazily in process_frame using
                // the actual input_frames, so it adapts to any frame size.
                // Initialize to 0 so first process_frame computes it.
                state.crossfade_step = 0.0;
            } else {
                // Immediate swap for first host or channel mismatch
                state.host = new_host;
                state.prev_host = None;
                state.crossfade_progress = 1.0;
            }

            state.channels = output_channels;

            // Pre-size the spare cache Arc so analyzer data updates never need to
            // allocate on the audio hot path, even on the first frame.
            let plugin_count = state.host.plugin_count();
            state.spare_cache_arc = Some(Arc::new(vec![None; plugin_count]));

            let latency_samples = state.host.total_latency_samples();
            response_tx
                .send(ProcessingResponse::PluginChainUpdated {
                    output_channels,
                    latency_samples,
                })
                .ok();
        }
        ProcessingCommand::SetParameter {
            plugin_index,
            param_id,
            value,
        } => {
            log::info!(
                "[Processing Thread] Set parameter: plugin {} param {} = {}",
                plugin_index,
                param_id,
                value
            );

            // Parse string value to ParameterValue
            let param_value = sotf_plugins::ParameterValue::parse(&value);

            match state
                .host
                .set_plugin_parameter(plugin_index, &param_id, param_value)
            {
                Ok(_) => {
                    log::debug!(
                        "[Processing Thread] Parameter set successfully on plugin {}",
                        plugin_index
                    );
                    response_tx.send(ProcessingResponse::Ok).ok();
                }
                Err(e) => {
                    log::warn!(
                        "[Processing Thread] Failed to set parameter on plugin {}: {}",
                        plugin_index,
                        e
                    );
                    response_tx
                        .send(ProcessingResponse::Error(format!(
                            "Failed to set parameter: {}",
                            e
                        )))
                        .ok();
                }
            }
        }
        ProcessingCommand::Bypass(bypass) => {
            state.bypassed = bypass;
            log::debug!("[Processing Thread] Bypass: {}", bypass);
            response_tx.send(ProcessingResponse::Ok).ok();
        }
        ProcessingCommand::GetPluginData(index) => match state.host.get_plugin_data(index) {
            Some(data) => {
                response_tx.send(ProcessingResponse::PluginData(data)).ok();
            }
            None => {
                response_tx
                    .send(ProcessingResponse::Error(format!(
                        "Plugin {} data not available",
                        index
                    )))
                    .ok();
            }
        },
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        ProcessingCommand::PollIsolatedExternalPluginWorkers => {
            let reports = state.host.poll_isolated_external_plugin_workers();
            for report in &reports {
                if let Some(error) = &report.error {
                    event_tx
                        .send(ThreadEvent::ProcessingWarning(format!(
                            "external plugin worker poll failed (plugin {}, node {}): {}",
                            report.plugin_index, report.node_id, error
                        )))
                        .ok();
                }
            }
            let statuses = reports
                .into_iter()
                .map(isolated_external_plugin_status)
                .collect::<Vec<_>>();
            event_tx
                .send(ThreadEvent::IsolatedExternalPluginWorkerStatuses(statuses))
                .ok();
        }
        ProcessingCommand::Stop => {
            log::debug!("[Processing Thread] Stopped");
        }
        ProcessingCommand::Shutdown => {
            log::debug!("[Processing Thread] Shutting down");
            return true;
        }
    }
    false
}

/// Update the shared plugin-data cache with the latest analyzer results.
///
/// Uses a spare `Arc` so that, when no UI reader holds the old cache, the
/// update is a zero-allocation in-place mutation.  When the UI is currently
/// reading the spare Arc, the update is skipped rather than allocating a new
/// buffer.
///
/// Returns `true` if the cache was updated this frame.
pub(super) fn update_plugin_data_cache(
    state: &mut ProcessingState,
    plugin_data_cache: &super::super::PluginDataCache,
) -> bool {
    let analyzer_indices = state.host.analyzer_indices();
    if analyzer_indices.is_empty() {
        return false;
    }

    let plugin_count = state.host.plugin_count();

    if let Some(mut spare) = state.spare_cache_arc.take() {
        if let Some(vec) = Arc::get_mut(&mut spare) {
            // Sole owner — mutate in place, zero allocations.
            state.cache_reuse_count += 1;
            if vec.len() != plugin_count {
                vec.resize(plugin_count, None);
            }
            for &i in analyzer_indices {
                vec[i] = state.host.get_plugin_data(i);
            }
            let old = plugin_data_cache.swap(spare);
            state.spare_cache_arc = Some(old);
            true
        } else {
            // Contention: UI thread still holds this Arc. Keep the spare for
            // the next attempt and skip this update — no allocation.
            state.spare_cache_arc = Some(spare);
            false
        }
    } else {
        // Defensive bootstrap (should not happen after init because the spare
        // is pre-sized at build time). Count it so diagnostics can spot it.
        state.cache_fallback_count += 1;
        let mut new_cache = vec![None; plugin_count];
        for &i in analyzer_indices {
            new_cache[i] = state.host.get_plugin_data(i);
        }
        let old_arc = plugin_data_cache.swap(Arc::new(new_cache));
        state.spare_cache_arc = Some(old_arc);
        true
    }
}

/// Main processing thread function
#[allow(clippy::too_many_arguments)] // thread entrypoint receives all channel endpoints from constructor
pub(super) fn run_processing_thread(
    decoder_rx: Receiver<DecoderMessage>,
    message_tx: SyncSender<ProcessingMessage>,
    command_rx: Receiver<ProcessingCommand>,
    response_tx: Sender<ProcessingResponse>,
    event_tx: Sender<ThreadEvent>,
    sample_rate: u32,
    channels: usize,
    plugin_data_cache: super::super::PluginDataCache,
    _gc_tx: super::super::GcSender,
    recycle_rx: Receiver<Vec<f32>>,
    decoder_recycle_tx: SyncSender<Vec<f32>>,
    #[cfg(feature = "streaming")] network_stream_tap: Option<sotf_streaming::PcmStreamHandle>,
) -> Result<(), String> {
    // Enable FTZ/DAZ CPU flags to prevent denormal numbers from causing
    // performance issues in IIR filters and other DSP code
    sotf_plugins::enable_ftz_daz();

    // Elevate thread priority for lower latency
    match super::super::rt_priority::set_realtime_priority(
        super::super::rt_priority::RtPriority::Processing,
    ) {
        Ok(true) => log::info!("[Processing Thread] RT priority set successfully"),
        Ok(false) => log::debug!("[Processing Thread] RT priority not available on this platform"),
        Err(e) => log::warn!("[Processing Thread] Failed to set RT priority: {e}"),
    }

    let mut state = ProcessingState::new(
        channels,
        sample_rate,
        #[cfg(feature = "streaming")]
        network_stream_tap,
    );

    log::info!(
        "[Processing Thread] Started - {}Hz, {} channels",
        sample_rate,
        channels
    );

    let mut decoder_stream_active = true;

    loop {
        // Check for commands (non-blocking)
        if let Ok(command) = command_rx.try_recv() {
            if matches!(
                command,
                ProcessingCommand::Stop | ProcessingCommand::Shutdown
            ) {
                decoder_stream_active = false;
            }
            if handle_processing_command(command, &mut state, &response_tx, &event_tx) {
                break;
            }
        }

        // Process audio from decoder. Use a timeout receive instead of
        // try_recv + sleep so arriving frames wake the processing thread
        // immediately while idle engines still back off.
        let message = if decoder_stream_active {
            decoder_rx.recv_timeout(std::time::Duration::from_micros(
                ACTIVE_EMPTY_SLEEP_PROCESSING_US,
            ))
        } else {
            decoder_rx.recv_timeout(std::time::Duration::from_millis(
                IDLE_EMPTY_SLEEP_PROCESSING_MS,
            ))
        };

        match message {
            Ok(DecoderMessage::Frame(frame)) => {
                decoder_stream_active = true;
                let output_channels = state.output_channels();

                // Query plugin chain for actual output size (accounts for resampler)
                let output_frames = state.output_frames_for_input(frame.num_frames);
                let output_samples = output_frames * output_channels;

                // Query plugin chain for actual output sample rate
                let output_sample_rate = state.output_sample_rate(frame.sample_rate);

                let mut process_buffer = std::mem::take(&mut state.process_buffer);
                if process_buffer.len() != output_samples {
                    ProcessingState::prepare_scratch_buffer(&mut process_buffer, output_samples);
                }

                let frame_start = std::time::Instant::now();
                match state.process_frame(&frame.data, &mut process_buffer, frame.num_frames) {
                    Ok(actual_output_frames) => {
                        let frame_elapsed = frame_start.elapsed();
                        if frame_elapsed > state.max_frame_duration {
                            state.max_frame_duration = frame_elapsed;
                        }
                        // Budget = frame_period. If processing exceeds it, the pipeline falls behind.
                        let frame_budget = std::time::Duration::from_secs_f64(
                            frame.num_frames as f64 / state.sample_rate as f64,
                        );
                        if frame_elapsed > frame_budget {
                            state.frames_over_budget += 1;
                        }

                        // Recycle the decoder frame's buffer back for reuse
                        decoder_recycle_tx.try_send(frame.data).ok();

                        // Use actual output frame count from processing (not max)
                        let actual_output_samples = actual_output_frames * output_channels;

                        state.frame_count += 1;

                        // Update shared plugin data cache so the UI can read
                        // analyzer results without blocking the audio pipeline.
                        let _ = update_plugin_data_cache(&mut state, &plugin_data_cache);

                        // Track timing for effective rate measurement
                        if state.first_frame_time.is_none() {
                            state.first_frame_time = Some(std::time::Instant::now());
                        }
                        state.total_output_samples += actual_output_samples as u64;

                        // Reuse a recycled Vec from the playback thread if available.
                        // Steady state should never allocate thanks to pre-filled recycle queues.
                        let frame_data = {
                            let mut buf = match recycle_rx.try_recv() {
                                Ok(mut v) => {
                                    v.clear();
                                    // Ensure capacity without re-allocating if possible.
                                    // reserve() is a no-op if capacity is already sufficient.
                                    if v.capacity() < actual_output_samples {
                                        v.reserve(actual_output_samples);
                                    }
                                    v
                                }
                                Err(_) => {
                                    // Fallback if recycle queue is empty (ramp-up or stall)
                                    state.recycle_miss_count += 1;
                                    Vec::with_capacity(actual_output_samples)
                                }
                            };
                            buf.extend_from_slice(&process_buffer[..actual_output_samples]);
                            buf
                        };

                        // Use actual output frame count and sample rate (accounts for resampler)
                        let processed_frame = super::super::AudioFrame::new(
                            frame_data,
                            actual_output_frames,
                            output_channels,
                            output_sample_rate,
                        );

                        #[cfg(feature = "streaming")]
                        if let Some(tap) = &state.network_stream_tap {
                            tap.publish(
                                &processed_frame.data,
                                processed_frame.num_frames,
                                processed_frame.num_channels,
                                processed_frame.sample_rate,
                            );
                        }

                        let mut pending_msg = Some(ProcessingMessage::Frame(processed_frame));
                        // Retry sending until the message is delivered or we shut down
                        while let Some(msg) = pending_msg.take() {
                            match send_or_interrupt(&message_tx, &command_rx, msg) {
                                Ok(Some((cmd, unsent))) => {
                                    let old_channels = state.channels;
                                    pending_msg = unsent;
                                    if handle_processing_command(
                                        cmd,
                                        &mut state,
                                        &response_tx,
                                        &event_tx,
                                    ) {
                                        break;
                                    }
                                    // If channels changed, discard the stale frame
                                    if state.channels != old_channels {
                                        pending_msg = None;
                                    }
                                }
                                Ok(None) => {
                                    // Sent successfully
                                }
                                Err(e) => {
                                    log::debug!("[Processing Thread] Send error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        event_tx.send(ThreadEvent::ProcessingError(e)).ok();
                    }
                }
                state.process_buffer = process_buffer;

                // RT diagnostics: log every 5 seconds
                if state.last_rt_diag.elapsed() >= std::time::Duration::from_secs(5) {
                    let total_cache = state.cache_reuse_count + state.cache_fallback_count;
                    let fallback_pct = if total_cache > 0 {
                        state.cache_fallback_count as f64 / total_cache as f64 * 100.0
                    } else {
                        0.0
                    };
                    log::info!(
                        "[Processing Thread] RT_DIAG: frames={}, cache_reuse={}, cache_fallback={} ({:.1}%), \
                         max_frame={:.2}ms, over_budget={}, recycle_miss={}",
                        state.frame_count,
                        state.cache_reuse_count,
                        state.cache_fallback_count,
                        fallback_pct,
                        state.max_frame_duration.as_secs_f64() * 1000.0,
                        state.frames_over_budget,
                        state.recycle_miss_count,
                    );
                    // Log per-analyzer contention stats
                    let analyzer_stats = state.host.take_analyzer_contention_stats();
                    for (idx, contention, updates) in &analyzer_stats {
                        if *contention > 0 && *updates > 0 {
                            log::warn!(
                                "[Processing Thread] RT_DIAG: analyzer[{}] contention={}/{} ({:.1}%)",
                                idx,
                                contention,
                                updates,
                                *contention as f64 / *updates as f64 * 100.0,
                            );
                        }
                    }
                    // Reset per-window counters
                    state.cache_reuse_count = 0;
                    state.cache_fallback_count = 0;
                    state.max_frame_duration = std::time::Duration::ZERO;
                    state.frames_over_budget = 0;
                    state.recycle_miss_count = 0;
                    state.last_rt_diag = std::time::Instant::now();
                }
            }
            Ok(DecoderMessage::EndOfStream) => {
                decoder_stream_active = false;
                let mut pending_msg = Some(ProcessingMessage::EndOfStream);
                while let Some(msg) = pending_msg.take() {
                    match send_or_interrupt(&message_tx, &command_rx, msg) {
                        Ok(Some((cmd, unsent))) => {
                            let old_channels = state.channels;
                            pending_msg = unsent;
                            if handle_processing_command(cmd, &mut state, &response_tx, &event_tx) {
                                break;
                            }
                            if state.channels != old_channels {
                                pending_msg = None;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            }
            Ok(DecoderMessage::Flush) => {
                decoder_stream_active = true;
                // Reset plugin state (IIR filter history, compressor envelopes,
                // limiter lookahead, upmixer FFT buffers) so that stale pre-seek
                // audio doesn't cause transient artifacts in post-seek output.
                state.host.reset();
                if let Some(ref mut prev) = state.prev_host {
                    prev.reset();
                }
                let mut pending_msg = Some(ProcessingMessage::Flush);
                while let Some(msg) = pending_msg.take() {
                    match send_or_interrupt(&message_tx, &command_rx, msg) {
                        Ok(Some((cmd, unsent))) => {
                            let old_channels = state.channels;
                            pending_msg = unsent;
                            if handle_processing_command(cmd, &mut state, &response_tx, &event_tx) {
                                break;
                            }
                            if state.channels != old_channels {
                                pending_msg = None;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                log::debug!("[Processing Thread] Decoder queue disconnected");
                break;
            }
        }
    }

    // Log effective playback rate measurement
    if let Some(start_time) = state.first_frame_time {
        let elapsed = start_time.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        let output_channels = state.output_channels();
        let total_frames = if output_channels > 0 {
            state.total_output_samples / output_channels as u64
        } else {
            0
        };
        let audio_duration_secs = total_frames as f64 / state.sample_rate as f64;
        let effective_rate = if elapsed_secs > 0.0 {
            (total_frames as f64 / elapsed_secs) as u64
        } else {
            0
        };
        let speed_ratio = if elapsed_secs > 0.0 {
            audio_duration_secs / elapsed_secs
        } else {
            0.0
        };
        log::warn!(
            "[Processing Thread] PLAYBACK RATE: {} frames in {:.3}s = {} effective Hz (expected {}Hz), audio_duration={:.3}s, speed_ratio={:.4}x, plugins={}",
            total_frames,
            elapsed_secs,
            effective_rate,
            state.sample_rate,
            audio_duration_secs,
            speed_ratio,
            state.host.plugin_count(),
        );
    }

    log::debug!("[Processing Thread] Stopped");
    Ok(())
}
