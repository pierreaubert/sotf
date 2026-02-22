// ============================================================================
// Processing Thread - Plugin Chain Execution
// ============================================================================
//
// Processes audio through the plugin chain with seamless hot-reload support.

use super::{
    DecoderMessage, PluginConfig, ProcessingCommand, ProcessingMessage, ProcessingResponse,
    ThreadEvent,
};
use sotf_plugins::{
    CompressorPluginParams, ConvolutionPlugin, ConvolutionPluginParams, CrossoverPlugin,
    CrossoverPluginParams, DelayPlugin, DelayPluginParams, DenoiserPlugin, DenoiserPluginParams,
    EqPluginParams, ExpanderPluginParams, FletcherMunsonPluginParams, GainPluginParams,
    GatePluginParams, Host, LimiterPluginParams, LoudnessCompensationPluginParams,
    LoudnessMonitorPlugin, MultibandCompressorPluginParams, MultibandExpanderPluginParams, Plugin,
    PluginHost, SpectrumAnalyzerPlugin, SpectrumConfig, UpmixerPluginParams,
};

use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::time::Duration;

const SPIN_MS_SIGNAL: u64 = 1;

/// Helper to send a message with backpressure handling and interruption support.
/// When a command arrives during backpressure, the pending message is returned
/// along with the command so the caller can handle both without data loss.
fn send_or_interrupt<T>(
    tx: &SyncSender<T>,
    rx: &Receiver<ProcessingCommand>,
    mut msg: T,
) -> Result<Option<(ProcessingCommand, Option<T>)>, String> {
    loop {
        match tx.try_send(msg) {
            Ok(_) => return Ok(None),
            Err(std::sync::mpsc::TrySendError::Full(returned_msg)) => {
                // Buffer full - check for interruption
                if let Ok(cmd) = rx.try_recv() {
                    // Return both the command AND the unsent message
                    return Ok(Some((cmd, Some(returned_msg))));
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

/// Parse a string value into a ParameterValue
/// Tries to detect the type intelligently:
/// - "true"/"false" -> Bool
/// - Integer string -> Int
/// - Float string -> Float
/// - JSON object/array -> String (for complex types like Vec<ChannelState>)
/// - Otherwise -> String
fn parse_parameter_value(value: &str) -> sotf_plugins::ParameterValue {
    // Try boolean
    if value == "true" {
        return sotf_plugins::ParameterValue::Bool(true);
    }
    if value == "false" {
        return sotf_plugins::ParameterValue::Bool(false);
    }

    // Try integer
    if let Ok(i) = value.parse::<i32>() {
        return sotf_plugins::ParameterValue::Int(i);
    }

    // Try float
    if let Ok(f) = value.parse::<f32>() {
        return sotf_plugins::ParameterValue::Float(f);
    }

    // Treat as string (for JSON or other complex types)
    sotf_plugins::ParameterValue::String(value.to_string())
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

    /// Process a frame
    /// Returns the actual number of output frames written
    fn process_frame(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        input_frames: usize,
    ) -> Result<usize, String> {
        if self.bypassed {
            // Bypass - just copy
            output.copy_from_slice(input);
            return Ok(input_frames);
        }

        // Handle crossfade if in progress
        if let Some(ref mut prev_host) = self.prev_host {
            let actual_frames = self.host.process(input, output)?;
            
            let output_samples = actual_frames * self.channels;
            if self.prev_process_buffer.len() < output_samples {
                self.prev_process_buffer.resize(output_samples, 0.0);
            }
            
            let _ = prev_host.process(input, &mut self.prev_process_buffer[..output_samples])?;
            
            // Blend buffers: output = (1-alpha)*prev + alpha*current
            let alpha = self.crossfade_progress;
            sotf_plugins::simd::blend_simd(output, &self.prev_process_buffer[..output_samples], alpha);
            
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
) -> bool {
    match command {
        ProcessingCommand::UpdateHost(new_host) => {
            let output_channels = new_host.output_channels();
            log::trace!(
                "[Processing Thread] UpdateHost: Plugin host updated, output_channels={}",
                output_channels
            );

            // Initiate crossfade if channel counts match and we have an existing chain
            if state.host.output_channels() == output_channels && state.host.plugin_count() > 0 {
                state.prev_host = Some(std::mem::replace(&mut state.host, new_host));
                state.crossfade_progress = 0.0;
                
                // Crossfade over ~50ms
                // For a 1024 frame size at 48kHz, this is ~2.3 blocks.
                // We ensure it takes at least 2 blocks for a smooth transition.
                let crossfade_duration_ms = 50.0;
                let block_duration_ms = (1024.0 * 1000.0) / state.sample_rate as f32;
                state.crossfade_step = (block_duration_ms / crossfade_duration_ms).min(0.5);
            } else {
                // Immediate swap for first host or channel mismatch
                state.host = new_host;
                state.prev_host = None;
                state.crossfade_progress = 1.0;
            }
            
            state.channels = output_channels;

            response_tx
                .send(ProcessingResponse::PluginChainUpdated { output_channels })
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
            let param_value = parse_parameter_value(&value);

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
fn run_processing_thread(
    decoder_rx: Receiver<DecoderMessage>,
    message_tx: SyncSender<ProcessingMessage>,
    command_rx: Receiver<ProcessingCommand>,
    response_tx: Sender<ProcessingResponse>,
    event_tx: Sender<ThreadEvent>,
    sample_rate: u32,
    channels: usize,
    plugin_data_cache: super::PluginDataCache,
    gc_tx: super::GcSender,
    recycle_rx: Receiver<Vec<f32>>,
    decoder_recycle_tx: SyncSender<Vec<f32>>,
) -> Result<(), String> {
    // Enable FTZ/DAZ CPU flags to prevent denormal numbers from causing
    // performance issues in IIR filters and other DSP code
    sotf_plugins::enable_ftz_daz();

    let mut state = ProcessingState::new(channels, sample_rate);

    log::info!(
        "[Processing Thread] Started - {}Hz, {} channels",
        sample_rate,
        channels
    );

    loop {
        // Check for commands (non-blocking)
        if let Ok(command) = command_rx.try_recv() {
            if handle_processing_command(command, &mut state, &response_tx) {
                break;
            }
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
                    process_buffer.resize(output_samples, 0.0);
                }

                match state.process_frame(&frame.data, &mut process_buffer, frame.num_frames) {
                    Ok(actual_output_frames) => {
                        // Recycle the decoder frame's buffer back for reuse
                        decoder_recycle_tx.send(frame.data).ok();

                        // Use actual output frame count from processing (not max)
                        let actual_output_samples = actual_output_frames * output_channels;

                        state.frame_count += 1;

                        // Update shared plugin data cache so the UI can read
                        // analyzer results without blocking the audio pipeline.
                        // Uses spare Arc reuse: after swap, keep the old Arc. Next
                        // frame, if refcount==1 (no active UI reader), Arc::get_mut
                        // lets us mutate in place — zero allocations in steady state.
                        {
                            let analyzer_indices = state.host.analyzer_indices();
                            if !analyzer_indices.is_empty() {
                                let plugin_count = state.host.plugin_count();

                                let reused = if let Some(mut spare) = state.spare_cache_arc.take() {
                                    if let Some(vec) = std::sync::Arc::get_mut(&mut spare) {
                                        // Sole owner — mutate in place, zero allocations
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
                                        // UI thread still reading — send to GC, fall back to clone
                                        gc_tx.try_send(super::gc_thread::GcItem::AnyArc(spare)).ok();
                                        false
                                    }
                                } else {
                                    false
                                };

                                if !reused {
                                    // First frame or rare contention: clone + allocate
                                    let old = plugin_data_cache.load();
                                    let mut new_cache = (**old).clone();
                                    if new_cache.len() != plugin_count {
                                        new_cache.resize(plugin_count, None);
                                    }
                                    for &i in analyzer_indices {
                                        new_cache[i] = state.host.get_plugin_data(i);
                                    }
                                    let old_arc = plugin_data_cache.swap(std::sync::Arc::new(new_cache));
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
                                    pending_msg = unsent;
                                    if handle_processing_command(cmd, &mut state, &response_tx) {
                                        break;
                                    }
                                    // Loop to retry sending the unsent message
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
            }
            Ok(DecoderMessage::EndOfStream) => {
                let mut pending_msg = Some(ProcessingMessage::EndOfStream);
                while let Some(msg) = pending_msg.take() {
                    match send_or_interrupt(&message_tx, &command_rx, msg) {
                        Ok(Some((cmd, unsent))) => {
                            pending_msg = unsent;
                            if handle_processing_command(cmd, &mut state, &response_tx) {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            }
            Ok(DecoderMessage::Flush) => {
                let mut pending_msg = Some(ProcessingMessage::Flush);
                while let Some(msg) = pending_msg.take() {
                    match send_or_interrupt(&message_tx, &command_rx, msg) {
                        Ok(Some((cmd, unsent))) => {
                            pending_msg = unsent;
                            if handle_processing_command(cmd, &mut state, &response_tx) {
                                break;
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

// ============================================================================
// Plugin Configuration Parameters
// ============================================================================

// ============================================================================
// Plugin Factory
// ============================================================================

/// Build a plugin host from configs
pub fn build_plugin_host(
    configs: &[PluginConfig],
    sample_rate: u32,
    channels: usize,
) -> Result<PluginHost, String> {
    let mut host = PluginHost::new(channels, sample_rate);
    let mut current_channels = channels;

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
                    return Err(format!(
                        "Plugin '{}' expects {} input channels, but chain provides {}",
                        config.plugin_type,
                        plugin.input_channels(),
                        current_channels
                    ));
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
                return Err(format!(
                    "Failed to create plugin '{}': {}",
                    config.plugin_type, e
                ));
            }
        }
    }

    log::info!(
        "[Processing Thread] Plugin chain loaded: {} plugins, {}ch -> {}ch",
        configs.len(),
        channels,
        host.output_channels()
    );

    Ok(host)
}

/// Create a plugin from configuration
fn create_plugin(
    plugin_type: &str,
    parameters: &serde_json::Value,
    channels: usize,
    sample_rate: u32,
) -> Result<Box<dyn Plugin>, String> {
    use sotf_plugins::{
        BinauralDecoderPlugin, CompressorPlugin, CrossfeedPlugin, CrossfeedPluginParams, EqPlugin,
        ExpanderPlugin, GainPlugin, GatePlugin, InPlacePluginAdapter, LimiterPlugin,
        LoudnessCompensationPlugin, MatrixPlugin, MultibandCompressorPlugin,
        MultibandExpanderPlugin, UpmixerPlugin,
    };

    match plugin_type {
        "gain" => {
            let params: GainPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse gain plugin parameters: {}", e))?;

            let plugin = GainPlugin::from_params(channels, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "upmixer" => {
            // Upmixer is always 2->5 channels
            if channels != 2 {
                return Err(format!(
                    "Upmixer requires 2 input channels, got {}",
                    channels
                ));
            }

            let params: UpmixerPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse upmixer plugin parameters: {}", e))?;

            let plugin = UpmixerPlugin::from_params(params);
            Ok(Box::new(plugin))
        }

        "eq" | "parametric_eq" => {
            let params: EqPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse EQ plugin parameters: {}", e))?;

            let plugin = EqPlugin::from_params(channels, sample_rate, params)?;
            Ok(Box::new(plugin))
        }

        "compressor" => {
            let params: CompressorPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse compressor plugin parameters: {}", e))?;

            let plugin = CompressorPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "limiter" => {
            let params: LimiterPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse limiter plugin parameters: {}", e))?;

            let plugin = LimiterPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "loudness_monitor" => {
            let plugin = LoudnessMonitorPlugin::new(channels)
                .map_err(|e| format!("Failed to create loudness monitor: {}", e))?;
            Ok(Box::new(plugin))
        }

        "spectrum_analyzer" => {
            // Check for config in parameters
            let config: SpectrumConfig = if parameters.is_null() {
                SpectrumConfig::default()
            } else {
                serde_json::from_value(parameters.clone())
                    .unwrap_or_else(|_| SpectrumConfig::default())
            };

            let plugin = SpectrumAnalyzerPlugin::with_config(channels, config)
                .map_err(|e| format!("Failed to create spectrum analyzer: {}", e))?;
            Ok(Box::new(plugin))
        }

        "convolution" => {
            let params: ConvolutionPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse convolution plugin parameters: {}", e))?;

            let plugin = ConvolutionPlugin::from_params(channels, sample_rate, params)
                .map_err(|e| format!("Failed to create convolution plugin: {}", e))?;
            Ok(Box::new(plugin))
        }

        "gate" => {
            let params: GatePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse gate plugin parameters: {}", e))?;

            let plugin = GatePlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "expander" => {
            let params: ExpanderPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse expander plugin parameters: {}", e))?;

            let plugin = ExpanderPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "multiband_compressor" => {
            let params: MultibandCompressorPluginParams =
                serde_json::from_value(parameters.clone()).map_err(|e| {
                    format!(
                        "Failed to parse multiband compressor plugin parameters: {}",
                        e
                    )
                })?;

            let plugin = MultibandCompressorPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "multiband_expander" => {
            let params: MultibandExpanderPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| {
                    format!(
                        "Failed to parse multiband expander plugin parameters: {}",
                        e
                    )
                })?;

            let plugin = MultibandExpanderPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "delay" => {
            let params: DelayPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse delay plugin parameters: {}", e))?;

            let plugin = DelayPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "loudness_compensation" => {
            let params: LoudnessCompensationPluginParams =
                serde_json::from_value(parameters.clone()).map_err(|e| {
                    format!(
                        "Failed to parse loudness compensation plugin parameters: {}",
                        e
                    )
                })?;

            let plugin = LoudnessCompensationPlugin::from_params(channels, params)?;
            Ok(Box::new(plugin))
        }

        "fletcher_munson" => {
            use sotf_plugins::FletcherMunsonPlugin;

            let params: FletcherMunsonPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse Fletcher-Munson plugin parameters: {}", e))?;

            let mut plugin = FletcherMunsonPlugin::from_params(channels, params);
            plugin.initialize(sample_rate)?;
            Ok(Box::new(plugin))
        }

        "matrix" => {
            #[derive(Debug, Clone, serde::Deserialize)]
            struct MatrixPluginParams {
                // Dense mapping parameters (legacy)
                #[serde(default)]
                input_channels: Option<usize>,
                #[serde(default)]
                output_channels: Option<usize>,
                // Sparse mapping parameters
                #[serde(default)]
                input_channel_map: Option<Vec<usize>>,
                #[serde(default)]
                output_channel_map: Option<Vec<usize>>,
                // Matrix data
                matrix: Vec<f32>,
                // Channel states for Mute/Solo
                #[serde(default)]
                channel_states: Option<Vec<sotf_plugins::ChannelState>>,
            }

            let params: MatrixPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse matrix plugin parameters: {}", e))?;

            {
                let off_diag: Vec<_> = params
                    .matrix
                    .iter()
                    .enumerate()
                    .filter(|(idx, v)| {
                        let cols = params
                            .input_channels
                            .or(params.input_channel_map.as_ref().map(|m| m.len()))
                            .unwrap_or(1);
                        let row = idx / cols;
                        let col = idx % cols;
                        row != col && v.abs() > 1e-6
                    })
                    .map(|(idx, v)| format!("[{}]={:.3}", idx, v))
                    .collect();
                log::debug!(
                    "[create_plugin:matrix] deserialized matrix len={}, off-diagonal entries: {:?}",
                    params.matrix.len(),
                    off_diag,
                );
            }

            // Determine if using sparse or dense mapping
            let mut plugin = if let (Some(in_map), Some(out_map)) =
                (params.input_channel_map, params.output_channel_map)
            {
                // Sparse mapping
                MatrixPlugin::with_sparse_mapping(in_map, out_map, params.matrix)
                    .map_err(|e| format!("Failed to create sparse matrix plugin: {}", e))?
            } else if let (Some(in_ch), Some(out_ch)) =
                (params.input_channels, params.output_channels)
            {
                // Dense mapping (legacy)
                MatrixPlugin::with_matrix(in_ch, out_ch, params.matrix)
                    .map_err(|e| format!("Failed to create matrix plugin: {}", e))?
            } else {
                return Err(
                    "Matrix plugin requires either (input_channels, output_channels) \
                     or (input_channel_map, output_channel_map)"
                        .to_string(),
                );
            };

            if let Some(states) = params.channel_states {
                log::debug!(
                    "[Engine] Matrix Plugin created with channel_states: {:?}",
                    states
                );
                plugin = plugin.with_channel_states(states);
            } else {
                log::trace!("[Engine] Matrix Plugin created WITHOUT channel_states");
            }

            Ok(Box::new(plugin))
        }

        "binaural_decoder" => {
            use sotf_plugins::BinauralDecoderParams;

            let params: BinauralDecoderParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse binaural decoder parameters: {}", e))?;

            let plugin = BinauralDecoderPlugin::from_params(params);
            Ok(Box::new(plugin))
        }

        "crossover" => {
            let params: CrossoverPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse crossover plugin parameters: {}", e))?;

            let plugin = CrossoverPlugin::from_params(channels, &params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        // HAL plugins (macOS only, requires 'hal' feature)
        #[cfg(all(target_os = "macos", feature = "hal"))]
        "hal_input" => {
            use sotf_plugins::{HalInputPlugin, HalInputPluginParams};

            let params: HalInputPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse HAL input plugin parameters: {}", e))?;

            let plugin = HalInputPlugin::from_params(params)?;
            Ok(Box::new(plugin))
        }

        #[cfg(all(target_os = "macos", feature = "hal"))]
        "hal_output" => {
            use sotf_plugins::{HalOutputPlugin, HalOutputPluginParams};

            let params: HalOutputPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse HAL output plugin parameters: {}", e))?;

            let plugin = HalOutputPlugin::from_params(params)?;
            Ok(Box::new(plugin))
        }

        "channel_mute_solo" => {
            use sotf_plugins::{
                ChannelMuteSoloParams, ChannelMuteSoloPlugin, InPlacePluginAdapter,
            };

            let params: ChannelMuteSoloParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse channel_mute_solo parameters: {}", e))?;

            let plugin = ChannelMuteSoloPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "xtc" | "crosstalk_cancellation" => {
            use sotf_plugins::{XtcPlugin, XtcPluginParams};

            // XTC requires exactly 2 channels (stereo)
            if channels != 2 {
                return Err(format!(
                    "XTC plugin requires 2 input channels (stereo), got {}",
                    channels
                ));
            }

            let params: XtcPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse XTC plugin parameters: {}", e))?;

            let plugin = XtcPlugin::from_params(params, sample_rate)?;
            Ok(Box::new(plugin))
        }

        "denoiser" | "wiener_denoiser" => {
            use sotf_plugins::InPlacePluginAdapter;

            let params: DenoiserPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse denoiser plugin parameters: {}", e))?;

            let plugin = DenoiserPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "pnd" | "varispeed" => {
            use sotf_plugins::{PndPlugin, PndPluginParams};

            let params: PndPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse PND plugin parameters: {}", e))?;

            let plugin = PndPlugin::from_params(channels, params);
            Ok(Box::new(plugin))
        }

        "ab_compare" | "ab" => {
            use sotf_plugins::{ABComparePlugin, ABComparePluginParams};

            let params: ABComparePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse A/B compare plugin parameters: {}", e))?;

            let mut plugin = ABComparePlugin::from_params(channels, params)?;
            plugin.initialize(sample_rate)?;
            Ok(Box::new(plugin))
        }

        "resampler" => {
            use sotf_plugins::ResamplerPlugin;

            #[derive(serde::Deserialize)]
            struct ResamplerParams {
                input_sample_rate: u32,
                output_sample_rate: u32,
                #[serde(default = "default_chunk_size")]
                chunk_size: usize,
            }
            fn default_chunk_size() -> usize {
                1024
            }

            let params: ResamplerParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse resampler plugin parameters: {}", e))?;

            let plugin = ResamplerPlugin::new(
                channels,
                params.input_sample_rate,
                params.output_sample_rate,
                params.chunk_size,
            )
            .map_err(|e| format!("Failed to create resampler: {}", e))?;

            Ok(Box::new(plugin))
        }

        "band_split" => {
            use sotf_plugins::{BandSplitPlugin, BandSplitPluginParams};

            let params: BandSplitPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse band_split parameters: {}", e))?;

            let plugin = BandSplitPlugin::from_params(channels, &params)?;
            Ok(Box::new(plugin))
        }

        "band_merge" => {
            use sotf_plugins::{BandMergePlugin, BandMergePluginParams};

            let params: BandMergePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse band_merge parameters: {}", e))?;

            // `channels` is the current chain channel count (= output_channels * bands after a split).
            // BandMerge::from_params expects output_channels, so divide by bands.
            let output_channels = channels / params.bands;
            let plugin = BandMergePlugin::from_params(output_channels, &params)?;
            Ok(Box::new(plugin))
        }

        "downmix" => {
            use sotf_plugins::{DownmixPlugin, DownmixPluginParams};

            let params: DownmixPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse downmix parameters: {}", e))?;

            let plugin = DownmixPlugin::from_params(params);
            Ok(Box::new(plugin))
        }

        "mono_to_stereo" => {
            use sotf_plugins::{MonoToStereoPlugin, MonoToStereoPluginParams};

            let params: MonoToStereoPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse mono_to_stereo parameters: {}", e))?;

            let plugin = MonoToStereoPlugin::from_params(channels, params);
            Ok(Box::new(plugin))
        }

        "crossfeed" => {
            // Crossfeed requires exactly 2 channels (stereo)
            if channels != 2 {
                return Err(format!(
                    "Crossfeed plugin requires 2 input channels (stereo), got {}",
                    channels
                ));
            }

            let params: CrossfeedPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse crossfeed plugin parameters: {}", e))?;

            let plugin = CrossfeedPlugin::from_params(params)
                .map_err(|e| format!("Failed to create crossfeed plugin: {}", e))?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        other => Err(format!("Unknown plugin type: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::{PluginSettings, PluginType};

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
            _ => 2,
        }
    }

    #[test]
    fn test_create_plugin_all_types() {
        let sample_rate = 48000;

        for plugin_type in PluginType::all() {
            // Convolution requires an IR file on disk — skip factory test
            if plugin_type == PluginType::Convolution {
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

            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate as f64);
            let channels = input_channels_for(&plugin_type);

            match build_plugin_host(&[config.clone()], sample_rate, channels) {
                Ok(_) => {}
                Err(e) => panic!(
                    "build_plugin_host failed for '{}': {}",
                    config.plugin_type, e
                ),
            }
        }
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
            let skip_process = matches!(
                plugin_type,
                PluginType::Convolution
                    | PluginType::Upmixer
                    | PluginType::BinauralDecoder
                    | PluginType::Pnd
            );
            if skip_process {
                continue;
            }

            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate as f64);
            let in_channels = input_channels_for(&plugin_type);

            let mut host = build_plugin_host(&[config.clone()], sample_rate, in_channels)
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
}
