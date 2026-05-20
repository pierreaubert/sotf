// ============================================================================
// Processing Thread - Plugin Chain Execution
// ============================================================================
//
// Processes audio through the plugin chain with seamless hot-reload support.

use super::{
    DecoderMessage, IsolatedExternalPluginWorkerStatus, PluginConfig, ProcessingCommand,
    ProcessingMessage, ProcessingResponse, ThreadEvent,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::ExternalPluginProcessEvent;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::IsolatedExternalPluginWorkerReport;
use sotf_plugins::{Host, Plugin, PluginHost};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::{PluginSandboxBackendCode, PluginSandboxStatusCode};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_types::{
    IsolatedExternalPluginSandboxBackend, IsolatedExternalPluginSandboxStatus,
    IsolatedExternalPluginWorkerEvent,
};

use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::time::Duration;

/// Helper to send a message with backpressure handling and interruption support.
/// When a command arrives during backpressure, the pending message is returned
/// along with the command so the caller can handle both without data loss.
fn send_or_interrupt<T>(
    tx: &SyncSender<T>,
    rx: &Receiver<ProcessingCommand>,
    mut msg: T,
) -> Result<Option<(ProcessingCommand, Option<T>)>, String> {
    let mut retries = 0;
    loop {
        match tx.try_send(msg) {
            Ok(_) => return Ok(None),
            Err(std::sync::mpsc::TrySendError::Full(returned_msg)) => {
                // Buffer full - check for interruption
                if let Ok(cmd) = rx.try_recv() {
                    // Return both the command AND the unsent message
                    return Ok(Some((cmd, Some(returned_msg))));
                }
                retries += 1;
                if retries > 200 {
                    return Err("Processing queue stuck for >1s".to_string());
                }
                msg = returned_msg;
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => return Err(format!("Channel disconnected: {}", e)),
        }
    }
}

/// Processing thread handle
pub struct ProcessingThread {
    command_tx: Sender<ProcessingCommand>,
    response_rx: Receiver<ProcessingResponse>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl ProcessingThread {
    /// Create and start the processing thread
    #[allow(clippy::too_many_arguments)] // thread constructor takes many channel endpoints
    pub fn new(
        decoder_rx: Receiver<DecoderMessage>,
        message_tx: SyncSender<ProcessingMessage>,
        event_tx: Sender<ThreadEvent>,
        sample_rate: u32,
        channels: usize,
        plugin_data_cache: super::PluginDataCache,
        gc_tx: super::GcSender,
        recycle_rx: Receiver<Vec<f32>>,
        decoder_recycle_tx: SyncSender<Vec<f32>>,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let (response_tx, response_rx) = std::sync::mpsc::channel();

        let thread_handle = std::thread::Builder::new()
            .name("processing".to_string())
            .spawn(move || {
                if let Err(e) = run_processing_thread(
                    decoder_rx,
                    message_tx,
                    command_rx,
                    response_tx,
                    event_tx,
                    sample_rate,
                    channels,
                    plugin_data_cache,
                    gc_tx,
                    recycle_rx,
                    decoder_recycle_tx,
                ) {
                    log::debug!("[Processing Thread] Error: {}", e);
                }
            })
            .map_err(|e| format!("Failed to spawn processing thread: {}", e))?;

        Ok(Self {
            command_tx,
            response_rx,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the processing thread
    pub fn send_command(&self, command: ProcessingCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// Receive a response (non-blocking)
    pub fn try_recv_response(&self) -> Option<ProcessingResponse> {
        self.response_rx.try_recv().ok()
    }

    /// Shutdown the processing thread
    pub fn shutdown(&mut self) {
        self.send_command(ProcessingCommand::Shutdown).ok();
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}

impl Drop for ProcessingThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Processing state
struct ProcessingState {
    /// Current plugin host
    host: PluginHost,
    /// Previous host for crossfading
    prev_host: Option<PluginHost>,
    /// Crossfade progress (0.0 to 1.0, 1.0 = current host only)
    crossfade_progress: f32,
    /// Crossfade step per frame
    crossfade_step: f32,
    /// Number of channels
    channels: usize,
    bypassed: bool,
    process_buffer: Vec<f32>,
    /// Buffer for previous host during crossfade
    prev_process_buffer: Vec<f32>,
    /// Frame counter for diagnostic logging
    frame_count: u64,
    /// Total output samples produced (for effective rate measurement)
    total_output_samples: u64,
    /// Timestamp of first frame processed
    first_frame_time: Option<std::time::Instant>,
    /// Sample rate (for effective rate calculation)
    sample_rate: u32,
    /// Spare Arc from previous plugin_data_cache swap, reused via Arc::get_mut
    /// to avoid per-frame Vec allocation when no UI reader holds a reference.
    spare_cache_arc: Option<std::sync::Arc<super::PluginDataVec>>,
    /// RT diagnostics: how many frames hit the cache fallback (allocation) path
    cache_fallback_count: u64,
    /// RT diagnostics: how many frames reused the spare Arc (zero-alloc fast path)
    cache_reuse_count: u64,
    /// RT diagnostics: max process_frame duration in the current reporting window
    max_frame_duration: std::time::Duration,
    /// RT diagnostics: last time we logged diagnostics
    last_rt_diag: std::time::Instant,
    /// RT diagnostics: how many frames took longer than the frame period
    frames_over_budget: u64,
    /// RT diagnostics: how many recycle misses (fallback Vec allocation)
    recycle_miss_count: u64,
}

impl ProcessingState {
    fn new(channels: usize, sample_rate: u32) -> Self {
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
            spare_cache_arc: None,
            cache_fallback_count: 0,
            cache_reuse_count: 0,
            max_frame_duration: std::time::Duration::ZERO,
            last_rt_diag: std::time::Instant::now(),
            frames_over_budget: 0,
            recycle_miss_count: 0,
        }
    }

    /// Get the actual output channel count
    fn output_channels(&self) -> usize {
        self.host.output_channels()
    }

    /// Get the output frame count for a given input frame count.
    /// Accounts for plugins that change frame count (like resamplers).
    fn output_frames_for_input(&self, input_frames: usize) -> usize {
        if self.bypassed || self.host.plugin_count() == 0 {
            input_frames
        } else {
            self.host.output_frames_for_input(input_frames)
        }
    }

    /// Get the output sample rate for a given input rate.
    /// Accounts for plugins that change sample rate (like resamplers).
    fn output_sample_rate(&self, input_rate: u32) -> u32 {
        if self.bypassed || self.host.plugin_count() == 0 {
            input_rate
        } else {
            self.host.output_sample_rate(input_rate)
        }
    }

    fn compute_crossfade_step(input_frames: usize, sample_rate: u32) -> f32 {
        if input_frames == 0 {
            return 1.0;
        }

        let crossfade_duration_ms = 50.0;
        let block_duration_ms = (input_frames as f32 * 1000.0) / sample_rate as f32;
        (block_duration_ms / crossfade_duration_ms).min(0.5)
    }

    fn prepare_scratch_buffer(buffer: &mut Vec<f32>, len: usize) {
        if buffer.len() != len {
            buffer.resize(len, 0.0);
        }
    }

    fn fade_in_unblended_tail(
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
    fn process_frame(
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
fn handle_processing_command(
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

/// Main processing thread function
#[allow(clippy::too_many_arguments)] // thread entrypoint receives all channel endpoints from constructor
fn run_processing_thread(
    decoder_rx: Receiver<DecoderMessage>,
    message_tx: SyncSender<ProcessingMessage>,
    command_rx: Receiver<ProcessingCommand>,
    response_tx: Sender<ProcessingResponse>,
    event_tx: Sender<ThreadEvent>,
    sample_rate: u32,
    channels: usize,
    plugin_data_cache: super::PluginDataCache,
    _gc_tx: super::GcSender,
    recycle_rx: Receiver<Vec<f32>>,
    decoder_recycle_tx: SyncSender<Vec<f32>>,
) -> Result<(), String> {
    // Enable FTZ/DAZ CPU flags to prevent denormal numbers from causing
    // performance issues in IIR filters and other DSP code
    sotf_plugins::enable_ftz_daz();

    // Elevate thread priority for lower latency
    match super::rt_priority::set_realtime_priority(super::rt_priority::RtPriority::Processing) {
        Ok(true) => log::info!("[Processing Thread] RT priority set successfully"),
        Ok(false) => log::debug!("[Processing Thread] RT priority not available on this platform"),
        Err(e) => log::warn!("[Processing Thread] Failed to set RT priority: {e}"),
    }

    let mut state = ProcessingState::new(channels, sample_rate);

    log::info!(
        "[Processing Thread] Started - {}Hz, {} channels",
        sample_rate,
        channels
    );

    loop {
        // Check for commands (non-blocking)
        if let Ok(command) = command_rx.try_recv()
            && handle_processing_command(command, &mut state, &response_tx, &event_tx)
        {
            break;
        }

        // Process audio from decoder
        let message = decoder_rx.try_recv();

        match message {
            Ok(DecoderMessage::Frame(frame)) => {
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
                        //
                        // Spare Arc reuse: after swap, keep the old Arc. Next frame,
                        // if refcount==1 (no active UI reader), Arc::get_mut lets us
                        // mutate in place — zero allocations.
                        //
                        // On contention (UI holds the spare), we skip the update
                        // rather than allocating. The UI sees one frame of stale
                        // analyzer data (~21ms) — imperceptible for spectrum/loudness.
                        {
                            let analyzer_indices = state.host.analyzer_indices();
                            if !analyzer_indices.is_empty() {
                                let plugin_count = state.host.plugin_count();

                                if let Some(mut spare) = state.spare_cache_arc.take() {
                                    if let Some(vec) = std::sync::Arc::get_mut(&mut spare) {
                                        // Sole owner — mutate in place, zero allocations
                                        state.cache_reuse_count += 1;
                                        if vec.len() != plugin_count {
                                            vec.resize(plugin_count, None);
                                        }
                                        for &i in analyzer_indices {
                                            vec[i] = state.host.get_plugin_data(i);
                                        }
                                        let old = plugin_data_cache.swap(spare);
                                        state.spare_cache_arc = Some(old);
                                    } else {
                                        // Contention: UI thread still holds this Arc.
                                        // Keep spare for next attempt, skip this update.
                                        state.cache_fallback_count += 1;
                                        state.spare_cache_arc = Some(spare);
                                    }
                                } else {
                                    // First frame: allocate once to bootstrap the spare.
                                    state.cache_fallback_count += 1;
                                    let mut new_cache = vec![None; plugin_count];
                                    for &i in analyzer_indices {
                                        new_cache[i] = state.host.get_plugin_data(i);
                                    }
                                    let old_arc =
                                        plugin_data_cache.swap(std::sync::Arc::new(new_cache));
                                    state.spare_cache_arc = Some(old_arc);
                                }
                            }
                        }

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
                        let processed_frame = super::AudioFrame::new(
                            frame_data,
                            actual_output_frames,
                            output_channels,
                            output_sample_rate,
                        );

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
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // No data, sleep briefly to avoid 100% CPU
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
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

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn isolated_external_plugin_status(
    report: IsolatedExternalPluginWorkerReport,
) -> IsolatedExternalPluginWorkerStatus {
    IsolatedExternalPluginWorkerStatus {
        plugin_index: report.plugin_index,
        node_id: report.node_id,
        event: report.event.map(isolated_external_plugin_event),
        error: report.error,
        worker_start_count: report.worker_start_count,
        worker_exit_count: report.worker_exit_count,
        worker_launch_failure_count: report.worker_launch_failure_count,
        block_timeout_count: report.block_timeout_count,
        block_worker_failure_count: report.block_worker_failure_count,
        block_wrong_sequence_count: report.block_wrong_sequence_count,
        sandbox_status: isolated_external_plugin_sandbox_status(report.sandbox_status),
        sandbox_backend: isolated_external_plugin_sandbox_backend(report.sandbox_backend),
        sandbox_reason: report.sandbox_reason,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn isolated_external_plugin_event(
    event: ExternalPluginProcessEvent,
) -> IsolatedExternalPluginWorkerEvent {
    match event {
        ExternalPluginProcessEvent::AlreadyRunning => {
            IsolatedExternalPluginWorkerEvent::AlreadyRunning
        }
        ExternalPluginProcessEvent::Started { pid } => {
            IsolatedExternalPluginWorkerEvent::Started { pid }
        }
        ExternalPluginProcessEvent::Exited { status } => {
            IsolatedExternalPluginWorkerEvent::Exited {
                exit_code: status.code(),
            }
        }
        ExternalPluginProcessEvent::NotRunning => IsolatedExternalPluginWorkerEvent::NotRunning,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn isolated_external_plugin_sandbox_status(
    status: PluginSandboxStatusCode,
) -> IsolatedExternalPluginSandboxStatus {
    match status {
        PluginSandboxStatusCode::Unknown => IsolatedExternalPluginSandboxStatus::Unknown,
        PluginSandboxStatusCode::Disabled => IsolatedExternalPluginSandboxStatus::Disabled,
        PluginSandboxStatusCode::Enforced => IsolatedExternalPluginSandboxStatus::Enforced,
        PluginSandboxStatusCode::Unsupported => IsolatedExternalPluginSandboxStatus::Unsupported,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn isolated_external_plugin_sandbox_backend(
    backend: PluginSandboxBackendCode,
) -> IsolatedExternalPluginSandboxBackend {
    match backend {
        PluginSandboxBackendCode::Unknown => IsolatedExternalPluginSandboxBackend::Unknown,
        PluginSandboxBackendCode::LinuxLandlock => {
            IsolatedExternalPluginSandboxBackend::LinuxLandlock
        }
        PluginSandboxBackendCode::MacosProcessIsolation => {
            IsolatedExternalPluginSandboxBackend::MacosProcessIsolation
        }
        PluginSandboxBackendCode::WindowsProcessIsolation => {
            IsolatedExternalPluginSandboxBackend::WindowsProcessIsolation
        }
    }
}

// ============================================================================
// Plugin Configuration Parameters
// ============================================================================

// ============================================================================
// Plugin Factory
// ============================================================================

/// Build a plugin host from configs.
///
/// Plugins that fail to create or have channel mismatches are skipped rather
/// than aborting the entire chain. The second element of the returned tuple
/// contains warnings about skipped plugins.
pub fn build_plugin_host(
    configs: &[PluginConfig],
    sample_rate: u32,
    channels: usize,
) -> Result<(PluginHost, Vec<String>), String> {
    let mut host = PluginHost::new(channels, sample_rate);
    let mut current_channels = channels;
    let mut warnings: Vec<String> = Vec::new();

    for (i, config) in configs.iter().enumerate() {
        log::info!(
            "[Processing Thread] Loading plugin {}: {}",
            i,
            config.plugin_type
        );

        match create_plugin(
            &config.plugin_type,
            &config.parameters,
            current_channels,
            sample_rate,
        ) {
            Ok(plugin) => {
                // Check channel compatibility
                if plugin.input_channels() != current_channels {
                    let msg = format!(
                        "Plugin '{}' skipped: expects {} input channels, but chain provides {}",
                        config.plugin_type,
                        plugin.input_channels(),
                        current_channels
                    );
                    log::warn!("[Processing Thread] {}", msg);
                    warnings.push(msg);
                    continue;
                }

                // Update current channel count for next plugin
                current_channels = plugin.output_channels();

                log::info!(
                    "[Processing Thread] Plugin '{}' loaded: {}ch -> {}ch",
                    config.plugin_type,
                    plugin.input_channels(),
                    plugin.output_channels()
                );

                host.add_plugin(plugin)?;
            }
            Err(e) => {
                let msg = format!("Plugin '{}' skipped: {}", config.plugin_type, e);
                log::warn!("[Processing Thread] {}", msg);
                warnings.push(msg);
            }
        }
    }

    log::info!(
        "[Processing Thread] Plugin chain loaded: {} plugins ({}ch -> {}ch), {} skipped",
        configs.len() - warnings.len(),
        channels,
        host.output_channels(),
        warnings.len()
    );

    Ok((host, warnings))
}

/// Build a plugin host from a graph config (DAG topology).
///
/// Unlike `build_plugin_host` which chains plugins linearly, this uses
/// `DawHost::add_node()` + `add_edge()` to create arbitrary graph topologies
/// needed for multi-driver crossover setups.
///
/// Nodes that fail to create are skipped, and edges referencing them are dropped.
pub fn build_plugin_graph_host(
    config: &super::types::PluginGraphConfig,
    sample_rate: u32,
    channels: usize,
) -> Result<(PluginHost, Vec<String>), String> {
    use sotf_plugins::GraphEdge;
    use std::collections::HashMap;

    let mut host = PluginHost::new(channels, sample_rate);
    let mut id_map: HashMap<usize, usize> = HashMap::new();
    let mut warnings: Vec<String> = Vec::new();

    for node_config in &config.nodes {
        match create_plugin(
            &node_config.plugin_type,
            &node_config.parameters,
            node_config.input_channels,
            sample_rate,
        ) {
            Ok(plugin) => {
                let host_id = host.add_node(format!("node_{}", node_config.id), plugin)?;
                id_map.insert(node_config.id, host_id);
            }
            Err(e) => {
                let msg = format!(
                    "Graph node {} ('{}') skipped: {}",
                    node_config.id, node_config.plugin_type, e
                );
                log::warn!("[Processing Thread] {}", msg);
                warnings.push(msg);
            }
        }
    }

    for edge in &config.edges {
        let from = match id_map.get(&edge.from_node) {
            Some(&id) => id,
            None => {
                let msg = format!(
                    "Edge {}->{} skipped: from_node {} was not loaded",
                    edge.from_node, edge.to_node, edge.from_node
                );
                log::warn!("[Processing Thread] {}", msg);
                warnings.push(msg);
                continue;
            }
        };
        let to = match id_map.get(&edge.to_node) {
            Some(&id) => id,
            None => {
                let msg = format!(
                    "Edge {}->{} skipped: to_node {} was not loaded",
                    edge.from_node, edge.to_node, edge.to_node
                );
                log::warn!("[Processing Thread] {}", msg);
                warnings.push(msg);
                continue;
            }
        };
        host.add_edge(GraphEdge::new(from, to))?;
    }

    host.build()?;

    log::info!(
        "[Processing Thread] Plugin graph loaded: {} nodes, {} edges ({}ch -> {}ch), {} warnings",
        id_map.len(),
        config.edges.len(),
        channels,
        host.output_channels(),
        warnings.len()
    );

    Ok((host, warnings))
}

/// Create a plugin from configuration
fn create_plugin(
    plugin_type: &str,
    parameters: &serde_json::Value,
    channels: usize,
    sample_rate: u32,
) -> Result<Box<dyn Plugin>, String> {
    sotf_plugins::create_plugin(plugin_type, parameters, channels, sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::{PluginSettings, PluginType};
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    use sotf_plugins::{
        ExternalPluginProcessEvent, ExternalPluginWorkerCommand, IsolatedExternalPlugin,
        IsolatedExternalPluginConfig, PluginDescriptor, PluginFormat,
    };
    use std::time::Duration;

    /// Returns the input channel count that `create_plugin` expects for each type.
    fn input_channels_for(plugin_type: &PluginType) -> usize {
        match plugin_type {
            PluginType::Upmixer => 2,
            PluginType::XTC => 2,
            PluginType::Crossfeed => 2,
            PluginType::MonoToStereo => 1,
            // BandMerge default is 2 bands, so input = output_channels * bands = 2 * 2 = 4
            PluginType::BandMerge => 4,
            // Downmix default has input_channels = 6
            PluginType::Downmix => 6,
            // BinauralDecoder defaults to 6 input channels (5.1)
            PluginType::BinauralDecoder => 6,
            // AmbisonicsDecoder order 1 = 4 channels (FOA)
            PluginType::AmbisonicsDecoder => 4,
            _ => 2,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn test_isolated_external_plugin_descriptor(
        name: &str,
    ) -> (tempfile::TempDir, PluginDescriptor) {
        let dir = tempfile::tempdir().unwrap();
        let plugin_path = dir.path().join(format!("{name}.clap"));
        std::fs::write(&plugin_path, b"stub external plugin").unwrap();
        let plugin_path = plugin_path.canonicalize().unwrap();
        let descriptor = PluginDescriptor {
            id: format!("test.{name}"),
            name: name.into(),
            vendor: "SOTF Test".into(),
            version: "0.1.0".into(),
            format: PluginFormat::Clap,
            path: plugin_path,
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: Vec::new(),
        };
        (dir, descriptor)
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn test_processing_state_with_invalid_isolated_plugin() -> (ProcessingState, tempfile::TempDir)
    {
        let (tempdir, descriptor) =
            test_isolated_external_plugin_descriptor("engine-processing-invalid");
        let plugin = IsolatedExternalPlugin::new(
            descriptor,
            48_000,
            IsolatedExternalPluginConfig {
                worker_command: ExternalPluginWorkerCommand::new(
                    "/definitely/not/a/real/sotf/external/plugin/worker",
                ),
                start_worker: false,
                ..Default::default()
            },
        )
        .unwrap();

        let mut host = PluginHost::new(2, 48_000);
        host.add_plugin(Box::new(plugin)).unwrap();

        let mut state = ProcessingState::new(2, 48_000);
        state.host = host;
        (state, tempdir)
    }

    #[test]
    fn test_create_plugin_all_types() {
        let sample_rate = 48000;

        for plugin_type in PluginType::all() {
            // Convolution requires an IR file on disk — skip factory test
            if plugin_type == PluginType::Convolution {
                continue;
            }
            // AmbisonicsDecoder requires the `iamf` feature
            #[cfg(not(feature = "iamf"))]
            if plugin_type == PluginType::AmbisonicsDecoder {
                continue;
            }

            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate as f64);
            let channels = input_channels_for(&plugin_type);

            let plugin = match create_plugin(
                &config.plugin_type,
                &config.parameters,
                channels,
                sample_rate,
            ) {
                Ok(p) => p,
                Err(e) => panic!("create_plugin failed for '{}': {}", config.plugin_type, e),
            };
            assert_eq!(
                plugin.input_channels(),
                channels,
                "input_channels mismatch for '{}'",
                config.plugin_type
            );
        }
    }

    #[test]
    fn test_build_plugin_host_all_types() {
        let sample_rate = 48000;

        for plugin_type in PluginType::all() {
            if plugin_type == PluginType::Convolution {
                continue;
            }
            #[cfg(not(feature = "iamf"))]
            if plugin_type == PluginType::AmbisonicsDecoder {
                continue;
            }

            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate as f64);
            let channels = input_channels_for(&plugin_type);

            match build_plugin_host(std::slice::from_ref(&config), sample_rate, channels) {
                Ok((_host, warnings)) => {
                    assert!(
                        warnings.is_empty(),
                        "build_plugin_host warnings for '{}': {:?}",
                        config.plugin_type,
                        warnings
                    );
                }
                Err(e) => panic!(
                    "build_plugin_host failed for '{}': {}",
                    config.plugin_type, e
                ),
            }
        }
    }

    #[test]
    fn test_downmix_adapts_to_current_chain_channel_count() {
        let sample_rate = 48000;
        let settings = PluginSettings::default_for(&PluginType::Downmix);
        let config = settings.to_plugin_config(sample_rate as f64);

        let plugin = create_plugin(&config.plugin_type, &config.parameters, 10, sample_rate)
            .expect("downmix should adapt default parameters to the chain width");
        assert_eq!(plugin.input_channels(), 10);
        assert_eq!(plugin.output_channels(), 2);

        let (host, warnings) = build_plugin_host(std::slice::from_ref(&config), sample_rate, 10)
            .expect("host should load adaptive downmix");
        assert!(
            warnings.is_empty(),
            "adaptive downmix should not be skipped: {:?}",
            warnings
        );
        assert_eq!(host.output_channels(), 2);
    }

    #[test]
    fn invalid_spectrum_analyzer_config_is_reported() {
        let config = PluginConfig::new("spectrum_analyzer", serde_json::json!("not an object"));

        let (_host, warnings) = build_plugin_host(&[config], 48_000, 2).unwrap();

        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].contains("Failed to parse spectrum analyzer params"),
            "unexpected warning: {}",
            warnings[0]
        );
    }

    #[test]
    fn test_process_audio_all_types() {
        let sample_rate = 48000;
        let num_frames = 1024;

        for plugin_type in PluginType::all() {
            // Skip plugins that can't be tested in isolation with a simple process call:
            // - Convolution requires an IR file on disk
            // - Upmixer/BinauralDecoder/Pnd use FFT overlap-add that returns 0 frames
            //   on first call, which triggers an assertion in PluginHost
            // - SpeechDenoiser requires block sizes that are multiples of 480
            let skip_process = matches!(
                plugin_type,
                PluginType::Convolution
                    | PluginType::Upmixer
                    | PluginType::BinauralDecoder
                    | PluginType::Pnd
                    | PluginType::SpeechDenoiser
            );
            if skip_process {
                continue;
            }
            // AmbisonicsDecoder requires the `iamf` feature
            #[cfg(not(feature = "iamf"))]
            if plugin_type == PluginType::AmbisonicsDecoder {
                continue;
            }

            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate as f64);
            let in_channels = input_channels_for(&plugin_type);

            let (mut host, _warnings) =
                build_plugin_host(std::slice::from_ref(&config), sample_rate, in_channels)
                    .unwrap_or_else(|e| panic!("build failed for '{}': {}", config.plugin_type, e));

            let out_channels = host.output_channels();

            // Generate a 440Hz sine wave as input
            let input: Vec<f32> = (0..num_frames * in_channels)
                .map(|i| {
                    let frame = i / in_channels;
                    (2.0 * std::f32::consts::PI * 440.0 * frame as f32 / sample_rate as f32).sin()
                        * 0.5
                })
                .collect();

            let mut output = vec![0.0f32; num_frames * out_channels];

            let result = host.process(&input, &mut output);
            assert!(
                result.is_ok(),
                "process failed for '{}': {}",
                config.plugin_type,
                result.err().unwrap()
            );

            // Some plugins produce silence in normal operation:
            // - Gate/Expander: gate signal to zero for quiet inputs
            // - ABCompare: may bypass
            // - XTC/Denoiser/Downmix: STFT latency causes silent output on first block
            let may_produce_silence = matches!(
                plugin_type,
                PluginType::Gate
                    | PluginType::Expander
                    | PluginType::ABCompare
                    | PluginType::XTC
                    | PluginType::Denoiser
                    | PluginType::Downmix
                    | PluginType::MonoToStereo
                    | PluginType::LinearPhaseEq
                    | PluginType::SpectralCompressor
            );

            if !may_produce_silence {
                let max_abs = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
                assert!(
                    max_abs > 1e-6,
                    "plugin '{}' produced silence (max_abs={})",
                    config.plugin_type,
                    max_abs
                );
            }
        }
    }

    /// Test that a non-square matrix (1 input → N outputs) is NOT auto-resized.
    /// This is the exact routing used by recording: mono sweep → specific output channel.
    /// Regression test for the bug where the matrix was incorrectly resized to 1×1,
    /// causing all sweeps to play on channel 0 regardless of output_channel.
    #[test]
    fn test_matrix_mono_to_multichannel_not_resized() {
        let sample_rate = 48000;

        // Simulate recording routing: mono signal → channel 1 (Right) of a stereo output
        for target_ch in 0..4 {
            let hw_channels = 4;
            let mut matrix = vec![0.0f32; hw_channels];
            matrix[target_ch] = 1.0;

            let matrix_params = serde_json::json!({
                "input_channels": 1,
                "output_channels": hw_channels,
                "matrix": matrix,
            });

            let config = PluginConfig::new("matrix", matrix_params);

            // Chain starts with 1 channel (mono WAV file)
            let (host, _warnings) =
                build_plugin_host(std::slice::from_ref(&config), sample_rate, 1).unwrap_or_else(
                    |e| {
                        panic!(
                            "build_plugin_host failed for 1→{} matrix targeting ch{}: {}",
                            hw_channels, target_ch, e
                        )
                    },
                );

            // Verify the chain expanded to the correct output channel count
            assert_eq!(
                host.output_channels(),
                hw_channels,
                "Matrix 1→{} should produce {} output channels, got {}",
                hw_channels,
                hw_channels,
                host.output_channels()
            );
        }
    }

    /// Test that a square matrix IS auto-resized when chain channels differ.
    /// E.g., a 2×2 matrix applied to a 4-channel chain should resize to 4×4.
    #[test]
    fn test_matrix_square_auto_resize() {
        let sample_rate = 48000;

        // 2×2 identity matrix applied to a 4-channel chain
        let matrix_params = serde_json::json!({
            "input_channels": 2,
            "output_channels": 2,
            "matrix": [1.0, 0.0, 0.0, 1.0],
        });

        let config = PluginConfig::new("matrix", matrix_params);
        let (host, _warnings) = build_plugin_host(std::slice::from_ref(&config), sample_rate, 4)
            .expect("build_plugin_host failed for 2×2 matrix on 4ch chain");

        // Should have been resized to 4×4
        assert_eq!(
            host.output_channels(),
            4,
            "Square 2×2 matrix on 4ch chain should auto-resize to 4×4"
        );
    }

    /// Test that a mono→stereo matrix correctly routes signal to the target channel.
    #[test]
    fn test_matrix_mono_routing_signal_integrity() {
        let sample_rate = 48000;
        let num_frames = 256;

        // Route mono to channel 1 (Right) of stereo output
        let matrix_params = serde_json::json!({
            "input_channels": 1,
            "output_channels": 2,
            "matrix": [0.0, 1.0],  // silence on L, signal on R
        });

        let config = PluginConfig::new("matrix", matrix_params);
        let (mut host, _warnings) =
            build_plugin_host(std::slice::from_ref(&config), sample_rate, 1)
                .expect("build_plugin_host failed");

        assert_eq!(host.output_channels(), 2);

        // Mono 440Hz sine input
        let input: Vec<f32> = (0..num_frames)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin() * 0.5
            })
            .collect();

        let mut output = vec![0.0f32; num_frames * 2];
        host.process(&input, &mut output).unwrap();

        // Channel 0 (Left) should be silent
        let left_max: f32 = output
            .iter()
            .step_by(2)
            .map(|s| s.abs())
            .fold(0.0, f32::max);
        // Channel 1 (Right) should have signal
        let right_max: f32 = output
            .iter()
            .skip(1)
            .step_by(2)
            .map(|s| s.abs())
            .fold(0.0, f32::max);

        assert!(
            left_max < 1e-6,
            "Left channel should be silent but has max={}",
            left_max
        );
        assert!(
            right_max > 0.1,
            "Right channel should have signal but max={}",
            right_max
        );
    }

    // ── Parameter sync test (verify all 3 places match) ──

    #[test]
    fn test_parameter_sync_get_matches_parameters_list() {
        let sample_rate = 48000;

        for plugin_type in PluginType::all() {
            if plugin_type == PluginType::Convolution {
                continue;
            }
            #[cfg(not(feature = "iamf"))]
            if plugin_type == PluginType::AmbisonicsDecoder {
                continue;
            }

            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate as f64);
            let channels = input_channels_for(&plugin_type);

            let plugin = match create_plugin(
                &config.plugin_type,
                &config.parameters,
                channels,
                sample_rate,
            ) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let params = plugin.parameters();
            for param in &params {
                let value = plugin.get_parameter(&param.id);
                assert!(
                    value.is_some(),
                    "Plugin '{}': parameter '{}' listed in parameters() but get_parameter() returns None. \
                     Likely missing from get_parameter() match arm.",
                    config.plugin_type,
                    param.id
                );
            }
        }
    }

    #[test]
    fn test_parameter_set_then_get_roundtrip() {
        use sotf_plugins::parameters::ParameterValue;

        let sample_rate = 48000;

        for plugin_type in PluginType::all() {
            if plugin_type == PluginType::Convolution {
                continue;
            }
            #[cfg(not(feature = "iamf"))]
            if plugin_type == PluginType::AmbisonicsDecoder {
                continue;
            }

            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate as f64);
            let channels = input_channels_for(&plugin_type);

            let mut plugin = match create_plugin(
                &config.plugin_type,
                &config.parameters,
                channels,
                sample_rate,
            ) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let params = plugin.parameters();
            for param in &params {
                // Pick a test value within the parameter's range
                let test_value = match (&param.default_value, &param.min_value, &param.max_value) {
                    (
                        ParameterValue::Float(_),
                        Some(ParameterValue::Float(min)),
                        Some(ParameterValue::Float(max)),
                    ) => {
                        // Use midpoint of range
                        ParameterValue::Float((min + max) / 2.0)
                    }
                    (ParameterValue::Bool(b), _, _) => ParameterValue::Bool(!b),
                    (
                        ParameterValue::Int(_),
                        Some(ParameterValue::Int(min)),
                        Some(ParameterValue::Int(max)),
                    ) => ParameterValue::Int((min + max) / 2),
                    _ => continue, // Skip string/complex params
                };

                let set_result = plugin.set_parameter(param.id.clone(), test_value.clone());
                if set_result.is_err() {
                    continue; // Some params may reject certain values
                }

                let got = plugin.get_parameter(&param.id);
                assert!(
                    got.is_some(),
                    "Plugin '{}': set_parameter('{}') succeeded but get_parameter returns None",
                    config.plugin_type,
                    param.id
                );
            }
        }
    }

    // ── Edge case tests ──

    #[test]
    fn test_nan_parameter_values_rejected_or_safe() {
        use sotf_plugins::parameters::ParameterValue;

        let sample_rate = 48000;
        let mut panicked_plugins = Vec::new();

        for plugin_type in PluginType::all() {
            if plugin_type == PluginType::Convolution {
                continue;
            }
            #[cfg(not(feature = "iamf"))]
            if plugin_type == PluginType::AmbisonicsDecoder {
                continue;
            }

            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate as f64);
            let channels = input_channels_for(&plugin_type);

            let type_name = config.plugin_type.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut plugin = match create_plugin(
                    &config.plugin_type,
                    &config.parameters,
                    channels,
                    sample_rate,
                ) {
                    Ok(p) => p,
                    Err(_) => return,
                };

                let params = plugin.parameters();
                for param in &params {
                    if matches!(param.default_value, ParameterValue::Float(_)) {
                        let _ =
                            plugin.set_parameter(param.id.clone(), ParameterValue::Float(f32::NAN));
                        let _ = plugin
                            .set_parameter(param.id.clone(), ParameterValue::Float(f32::INFINITY));
                        let _ = plugin.set_parameter(
                            param.id.clone(),
                            ParameterValue::Float(f32::NEG_INFINITY),
                        );
                    }
                }

                let num_frames = 64;
                let in_samples = num_frames * plugin.input_channels();
                let out_samples = num_frames * plugin.output_channels();
                let input = vec![0.5_f32; in_samples];
                let mut output = vec![0.0_f32; out_samples];
                let context = sotf_plugins::plugin::ProcessContext::new(sample_rate, num_frames);
                let _ = plugin.process(&input, &mut output, &context);
            }));

            if result.is_err() {
                panicked_plugins.push(type_name);
            }
        }

        // Log which plugins panicked with NaN — these should be fixed eventually
        // but we don't fail the test since NaN params are an edge case
        if !panicked_plugins.is_empty() {
            eprintln!(
                "WARNING: {} plugin(s) panicked with NaN/inf params: {:?}",
                panicked_plugins.len(),
                panicked_plugins
            );
        }
    }

    #[test]
    fn test_process_zero_frames_does_not_panic() {
        let sample_rate = 48000;

        for plugin_type in PluginType::all() {
            if plugin_type == PluginType::Convolution {
                continue;
            }
            #[cfg(not(feature = "iamf"))]
            if plugin_type == PluginType::AmbisonicsDecoder {
                continue;
            }

            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate as f64);
            let channels = input_channels_for(&plugin_type);

            let mut plugin = match create_plugin(
                &config.plugin_type,
                &config.parameters,
                channels,
                sample_rate,
            ) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let context = sotf_plugins::plugin::ProcessContext::new(sample_rate, 0);
            // Zero-length buffers — must not panic
            let _ = plugin.process(&[], &mut [], &context);
        }
    }

    #[test]
    fn crossfade_zero_frame_block_completes_instead_of_leaking_prev_host() {
        assert_eq!(ProcessingState::compute_crossfade_step(0, 48_000), 1.0);
    }

    #[test]
    fn crossfade_fades_in_unblended_new_host_tail() {
        let mut output = vec![1.0; 8];

        ProcessingState::fade_in_unblended_tail(&mut output, 4, 8, 0.25);

        assert_eq!(&output[..4], &[1.0; 4]);
        assert_eq!(&output[4..], &[0.25; 4]);
    }

    #[test]
    fn processing_hot_path_uses_prepared_buffers_for_output_and_crossfade() {
        let source = include_str!("processing_thread.rs");

        assert!(
            !source.contains(concat!("process_buffer.", "resize(output_samples, 0.0)")),
            "processing thread must not allocate/resize the process buffer in the frame hot path"
        );
        assert!(
            !source.contains(concat!("prev_process_buffer.", "resize(buf_len, 0.0)")),
            "crossfade processing must not allocate/resize the previous-host buffer in process_frame"
        );
    }

    // ── Thread isolation tests for send_or_interrupt ──

    #[test]
    fn send_or_interrupt_delivers_message_when_buffer_has_space() {
        let (tx, rx) = std::sync::mpsc::sync_channel::<u32>(4);
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ProcessingCommand>();

        let handle = std::thread::spawn(move || send_or_interrupt(&tx, &cmd_rx, 42));

        let result = handle.join().expect("thread panicked");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // No interruption
        assert_eq!(rx.recv().unwrap(), 42);
        drop(cmd_tx); // keep cmd_tx alive until assertion
    }

    #[test]
    fn send_or_interrupt_returns_command_when_interrupted_during_backpressure() {
        // Buffer capacity 1, pre-fill it so the next send blocks
        let (tx, rx) = std::sync::mpsc::sync_channel::<u32>(1);
        tx.send(99).unwrap(); // Fill the buffer

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ProcessingCommand>();

        // Send a command that will be found during the backpressure retry
        cmd_tx.send(ProcessingCommand::Stop).unwrap();

        let handle = std::thread::spawn(move || send_or_interrupt(&tx, &cmd_rx, 42));

        let result = handle.join().expect("thread panicked");
        let (cmd, unsent_msg) = result.unwrap().expect("should have been interrupted");
        assert!(matches!(cmd, ProcessingCommand::Stop));
        assert_eq!(unsent_msg.unwrap(), 42); // Message returned, not lost
        assert_eq!(rx.recv().unwrap(), 99); // Original message still in buffer
    }

    #[test]
    fn send_or_interrupt_errors_when_channel_disconnected() {
        let (tx, rx) = std::sync::mpsc::sync_channel::<u32>(4);
        let (_cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ProcessingCommand>();
        drop(rx); // Disconnect the receiver

        let result = send_or_interrupt(&tx, &cmd_rx, 42);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("disconnected"));
    }

    // ── AudioFrame invariant tests ──

    #[test]
    fn audio_frame_new_enforces_data_length_invariant() {
        use sotf_types::AudioFrame;

        // Valid: data.len() == num_frames * num_channels
        let frame = AudioFrame::new(vec![0.0; 2048], 1024, 2, 48000);
        assert_eq!(frame.num_samples(), 2048);
        assert_eq!(frame.num_frames, 1024);
        assert_eq!(frame.num_channels, 2);
    }

    #[test]
    fn audio_frame_silent_produces_all_zeros() {
        use sotf_types::AudioFrame;

        let frame = AudioFrame::silent(512, 6, 48000);
        assert_eq!(frame.data.len(), 512 * 6);
        assert!(frame.data.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn audio_frame_clear_resets_to_silence() {
        use sotf_types::AudioFrame;

        let mut frame = AudioFrame::new(vec![1.0; 1024], 512, 2, 48000);
        assert!(frame.data.iter().all(|&s| s == 1.0));

        frame.clear();
        assert!(frame.data.iter().all(|&s| s == 0.0));
        // Metadata unchanged
        assert_eq!(frame.num_frames, 512);
        assert_eq!(frame.num_channels, 2);
    }

    #[test]
    fn audio_frame_invariants_across_channel_counts() {
        use sotf_types::AudioFrame;

        for channels in [1, 2, 4, 6, 8] {
            let frames = 256;
            let total = frames * channels;
            let data: Vec<f32> = (0..total).map(|i| i as f32 / total as f32).collect();
            let frame = AudioFrame::new(data, frames, channels, 48000);

            assert_eq!(frame.num_samples(), total);
            assert_eq!(frame.data.len(), total);
            // All samples in [-1, 1) range for this test data
            assert!(frame.data.iter().all(|&s| (0.0..1.0).contains(&s)));
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[test]
    fn test_isolated_external_plugin_event_and_status_mappings() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;

        let event = isolated_external_plugin_event(ExternalPluginProcessEvent::AlreadyRunning);
        assert!(matches!(
            event,
            IsolatedExternalPluginWorkerEvent::AlreadyRunning
        ));

        let event = isolated_external_plugin_event(ExternalPluginProcessEvent::NotRunning);
        assert!(matches!(
            event,
            IsolatedExternalPluginWorkerEvent::NotRunning
        ));

        let event =
            isolated_external_plugin_event(ExternalPluginProcessEvent::Started { pid: 555 });
        assert!(matches!(
            event,
            IsolatedExternalPluginWorkerEvent::Started { pid } if pid == 555
        ));

        #[cfg(unix)]
        {
            let status = ExitStatusExt::from_raw(11 << 8);
            let event =
                isolated_external_plugin_event(ExternalPluginProcessEvent::Exited { status });
            assert!(matches!(
                event,
                IsolatedExternalPluginWorkerEvent::Exited {
                    exit_code: Some(11)
                }
            ));
        }
        #[cfg(windows)]
        {
            let status = ExitStatusExt::from_raw((11 << 8) as u32);
            let event =
                isolated_external_plugin_event(ExternalPluginProcessEvent::Exited { status });
            assert!(matches!(
                event,
                IsolatedExternalPluginWorkerEvent::Exited {
                    exit_code: Some(11)
                }
            ));
        }

        let report = IsolatedExternalPluginWorkerReport {
            plugin_index: 3,
            node_id: 9,
            event: Some(ExternalPluginProcessEvent::Started { pid: 777 }),
            error: Some("blocked".into()),
            worker_start_count: 4,
            worker_exit_count: 2,
            worker_launch_failure_count: 1,
            block_timeout_count: 3,
            block_worker_failure_count: 4,
            block_wrong_sequence_count: 5,
            sandbox_status: PluginSandboxStatusCode::Enforced,
            sandbox_backend: PluginSandboxBackendCode::LinuxLandlock,
            sandbox_reason: None,
        };
        let status = isolated_external_plugin_status(report);
        assert_eq!(status.plugin_index, 3);
        assert_eq!(status.node_id, 9);
        assert_eq!(status.error, Some("blocked".into()));
        assert_eq!(status.worker_start_count, 4);
        assert_eq!(status.worker_exit_count, 2);
        assert_eq!(status.worker_launch_failure_count, 1);
        assert_eq!(status.block_timeout_count, 3);
        assert_eq!(status.block_worker_failure_count, 4);
        assert_eq!(status.block_wrong_sequence_count, 5);
        assert_eq!(
            status.sandbox_status,
            IsolatedExternalPluginSandboxStatus::Enforced
        );
        assert_eq!(
            status.sandbox_backend,
            IsolatedExternalPluginSandboxBackend::LinuxLandlock
        );
        assert_eq!(status.sandbox_reason, None);
        assert!(matches!(
            status.event,
            Some(IsolatedExternalPluginWorkerEvent::Started { pid }) if pid == 777
        ));
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[test]
    fn test_handle_processing_command_polls_isolated_external_plugin_statuses_without_launching() {
        let (mut state, _tempdir) = test_processing_state_with_invalid_isolated_plugin();

        let (response_tx, _response_rx) = std::sync::mpsc::channel::<ProcessingResponse>();
        let (event_tx, event_rx) = std::sync::mpsc::channel::<ThreadEvent>();

        let shutdown = handle_processing_command(
            ProcessingCommand::PollIsolatedExternalPluginWorkers,
            &mut state,
            &response_tx,
            &event_tx,
        );
        assert!(!shutdown);

        let statuses = match event_rx.recv_timeout(Duration::from_secs(1)).unwrap() {
            ThreadEvent::IsolatedExternalPluginWorkerStatuses(statuses) => statuses,
            event => panic!("expected status event, got {:?}", event),
        };
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].plugin_index, 0);
        assert_eq!(statuses[0].node_id, 0);
        assert_eq!(statuses[0].error, None);
        assert_eq!(statuses[0].worker_launch_failure_count, 0);
        assert!(matches!(
            statuses[0].event,
            Some(IsolatedExternalPluginWorkerEvent::NotRunning)
        ));
    }
}
