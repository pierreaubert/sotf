// ============================================================================
// Processing Thread - Plugin Chain Execution
// ============================================================================
//
// Processes audio through the plugin chain with seamless hot-reload support.

use super::{
    DecoderMessage, PluginConfig, ProcessingCommand, ProcessingMessage, ProcessingResponse,
    ThreadEvent,
};
use crate::plugins::{
    AnalyzerPlugin, CompressorPluginParams, CrossoverPlugin, CrossoverPluginParams, DelayPlugin,
    DelayPluginParams, EqPluginParams, GainPluginParams, GatePluginParams, LimiterPluginParams,
    LoudnessCompensationPluginParams, Plugin, PluginHost, ProcessContext, UpmixerPluginParams,
};

use std::collections::HashMap;
use std::sync::{
    Arc,
    mpsc::{Receiver, Sender, SyncSender},
};

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
    /// Next plugin host (for hot-reload)
    next_host: Option<PluginHost>,
    /// Analyzer plugins (by ID)
    analyzers: HashMap<String, Box<dyn AnalyzerPlugin>>,
    /// Sample rate
    sample_rate: u32,
    /// Number of channels
    channels: usize,
    /// Bypass flag
    bypassed: bool,
    /// Crossfade position (0.0 = current, 1.0 = next)
    crossfade_pos: f32,
    /// Crossfade duration in frames
    crossfade_frames: usize,
    /// Current crossfade frame
    crossfade_current: usize,
}

impl ProcessingState {
    fn new(sample_rate: u32, channels: usize) -> Self {
        Self {
            host: PluginHost::new(channels, sample_rate),
            next_host: None,
            analyzers: HashMap::new(),
            sample_rate,
            channels,
            bypassed: false,
            crossfade_pos: 0.0,
            crossfade_frames: 4096, // ~85ms at 48kHz
            crossfade_current: 0,
        }
    }

    /// Start plugin chain update (hot-reload)
    fn start_reload(&mut self, new_host: PluginHost) {
        self.next_host = Some(new_host);
        self.crossfade_pos = 0.0;
        self.crossfade_current = 0;
        log::info!(
            "[Processing Thread] Starting plugin hot-reload (crossfade {} frames)",
            self.crossfade_frames
        );
    }

    /// Get the actual output channel count (accounting for pending hot-reload)
    /// If a hot-reload is pending with different channel count, returns the new channel count
    /// since the swap will happen immediately (no crossfade possible)
    fn output_channels(&self) -> usize {
        if let Some(next_host) = &self.next_host {
            // If next_host has different channel count, it will be swapped immediately
            if self.host.output_channels() != next_host.output_channels() {
                return next_host.output_channels();
            }
        }
        // Otherwise use current host's output or tracked channel count
        self.host.output_channels()
    }

    /// Recreate all analyzers with new channel count
    fn recreate_analyzers_for_channels(&mut self, new_channels: usize) -> Result<(), String> {
        use crate::plugins::{LoudnessMonitorPlugin, SpectrumAnalyzerPlugin};

        // Collect analyzer IDs and types before recreating
        let analyzer_info: Vec<(String, bool)> = self
            .analyzers
            .keys()
            .map(|id| {
                let is_spectrum = id.contains("spectrum");
                (id.clone(), is_spectrum)
            })
            .collect();

        if analyzer_info.is_empty() {
            return Ok(());
        }

        log::info!(
            "[Processing Thread] Recreating {} analyzers for {} channels",
            analyzer_info.len(),
            new_channels
        );

        // Recreate each analyzer with new channel count
        for (id, is_spectrum) in analyzer_info {
            self.analyzers.remove(&id);

            if is_spectrum {
                match SpectrumAnalyzerPlugin::new(new_channels) {
                    Ok(mut plugin) => {
                        plugin.initialize(self.sample_rate)?;
                        self.analyzers.insert(id.clone(), Box::new(plugin));
                        log::debug!(
                            "[Processing Thread] Recreated spectrum analyzer '{}' with {} channels",
                            id,
                            new_channels
                        );
                    }
                    Err(e) => {
                        log::warn!(
                            "[Processing Thread] Failed to recreate spectrum analyzer '{}': {}",
                            id,
                            e
                        );
                    }
                }
            } else {
                match LoudnessMonitorPlugin::new(new_channels) {
                    Ok(mut plugin) => {
                        plugin.initialize(self.sample_rate)?;
                        self.analyzers.insert(id.clone(), Box::new(plugin));
                        log::debug!(
                            "[Processing Thread] Recreated loudness analyzer '{}' with {} channels",
                            id,
                            new_channels
                        );
                    }
                    Err(e) => {
                        log::warn!(
                            "[Processing Thread] Failed to recreate loudness analyzer '{}': {}",
                            id,
                            e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Add an analyzer plugin
    fn add_analyzer(
        &mut self,
        id: String,
        mut analyzer: Box<dyn AnalyzerPlugin>,
    ) -> Result<(), String> {
        // Initialize the analyzer with current sample rate
        analyzer.initialize(self.sample_rate)?;

        log::debug!("[Processing Thread] Added analyzer: {}", id);
        self.analyzers.insert(id, analyzer);
        Ok(())
    }

    /// Remove an analyzer plugin
    fn remove_analyzer(&mut self, id: &str) -> Option<Box<dyn AnalyzerPlugin>> {
        log::debug!("[Processing Thread] Removed analyzer: {}", id);
        self.analyzers.remove(id)
    }

    /// Get analyzer data
    fn get_analyzer_data(&self, id: &str) -> Result<Arc<dyn std::any::Any + Send + Sync>, String> {
        use crate::plugins::{LoudnessData, SpectrumData};

        let analyzer = self
            .analyzers
            .get(id)
            .ok_or_else(|| format!("Analyzer '{}' not found", id))?;

        // Get data from the analyzer (returns Box<dyn Any + Send>)
        let data = analyzer.get_data();

        // Try downcasting to known types and re-wrapping in Arc
        if let Some(loudness) = data.downcast_ref::<LoudnessData>() {
            return Ok(Arc::new(loudness.clone()) as Arc<dyn std::any::Any + Send + Sync>);
        }

        if let Some(spectrum) = data.downcast_ref::<SpectrumData>() {
            return Ok(Arc::new(spectrum.clone()) as Arc<dyn std::any::Any + Send + Sync>);
        }

        Err(format!("Unknown analyzer data type for '{}'", id))
    }

    /// Process a frame with seamless crossfade
    fn process_frame(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), String> {
        if self.bypassed {
            // Bypass - just copy
            output.copy_from_slice(input);
            return Ok(());
        }

        if let Some(next_host) = &mut self.next_host {
            // Check if channel counts differ - crossfade only works for same channel count
            if self.host.output_channels() != next_host.output_channels() {
                log::info!(
                    "[Processing Thread] Channel count change detected ({}→{}), immediate swap (no crossfade)",
                    self.host.output_channels(),
                    next_host.output_channels()
                );

                // Immediate swap - no crossfade possible when channel count changes
                let old_channels = self.channels;
                std::mem::swap(&mut self.host, next_host);
                self.channels = self.host.output_channels();
                self.next_host = None;
                self.crossfade_pos = 0.0;
                self.crossfade_current = 0;

                log::info!(
                    "[Processing Thread] Updated output channels: {}",
                    self.channels
                );

                // Recreate analyzers for new channel count
                if old_channels != self.channels {
                    if let Err(e) = self.recreate_analyzers_for_channels(self.channels) {
                        log::warn!("[Processing Thread] Failed to recreate analyzers: {}", e);
                    }
                }

                // Process with new host
                self.host.process(input, output)?;
            } else {
                // Crossfading between old and new plugin chains (same channel count)
                let num_frames = input.len() / self.host.input_channels();
                let output_samples = num_frames * self.host.output_channels();
                let mut output_old = vec![0.0; output_samples];
                let mut output_new = vec![0.0; output_samples];

                // Process with both hosts
                self.host.process(input, &mut output_old)?;
                next_host.process(input, &mut output_new)?;

                // Calculate crossfade coefficient
                self.crossfade_current += num_frames;
                self.crossfade_pos =
                    (self.crossfade_current as f32 / self.crossfade_frames as f32).min(1.0);

                // Apply crossfade
                for i in 0..output.len() {
                    output[i] = output_old[i] * (1.0 - self.crossfade_pos)
                        + output_new[i] * self.crossfade_pos;
                }

                // Check if crossfade complete
                if self.crossfade_pos >= 1.0 {
                    log::debug!("[Processing Thread] Hot-reload complete");
                    // Swap in the new host
                    let old_channels = self.channels;
                    std::mem::swap(&mut self.host, next_host);
                    self.channels = self.host.output_channels();
                    self.next_host = None;
                    self.crossfade_pos = 0.0;
                    self.crossfade_current = 0;

                    // Recreate analyzers if channel count changed (shouldn't happen in crossfade path, but be safe)
                    if old_channels != self.channels {
                        if let Err(e) = self.recreate_analyzers_for_channels(self.channels) {
                            log::warn!("[Processing Thread] Failed to recreate analyzers: {}", e);
                        }
                    }
                }
            }
        } else {
            // Normal processing
            self.host.process(input, output)?;
        }

        // Feed audio to analyzer plugins
        if !self.analyzers.is_empty() {
            // Use actual host output channels, not cached self.channels
            let output_channels = self.host.output_channels();
            let num_frames = output.len() / output_channels;
            let context = ProcessContext {
                sample_rate: self.sample_rate,
                num_frames,
            };

            for (id, analyzer) in self.analyzers.iter_mut() {
                if let Err(e) = analyzer.process(output, &context) {
                    log::debug!("[Processing Thread] Analyzer '{}' error: {}", id, e);
                }
            }
        }

        Ok(())
    }
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
) -> Result<(), String> {
    let mut state = ProcessingState::new(sample_rate, channels);

    log::info!(
        "[Processing Thread] Started - {}Hz, {} channels",
        sample_rate,
        channels
    );

    loop {
        // Check for commands (non-blocking)
        if let Ok(command) = command_rx.try_recv() {
            match command {
                ProcessingCommand::UpdatePlugins(configs) => {
                    // Create new plugin host
                    match build_plugin_host(&configs, sample_rate, channels) {
                        Ok(new_host) => {
                            let output_channels = new_host.output_channels();
                            state.start_reload(new_host);
                            response_tx
                                .send(ProcessingResponse::PluginChainUpdated { output_channels })
                                .ok();
                        }
                        Err(e) => {
                            log::debug!("[Processing Thread] Failed to build plugin chain: {}", e);
                            response_tx.send(ProcessingResponse::Error(e)).ok();
                        }
                    }
                }
                ProcessingCommand::SetParameter {
                    plugin_index,
                    param_id,
                    value,
                } => {
                    // Update parameter on current host
                    // TODO: Implement parameter setting
                    log::info!(
                        "[Processing Thread] Set parameter: plugin {} param {} = {}",
                        plugin_index,
                        param_id,
                        value
                    );
                    response_tx.send(ProcessingResponse::Ok).ok();
                }
                ProcessingCommand::Bypass(bypass) => {
                    state.bypassed = bypass;
                    log::debug!("[Processing Thread] Bypass: {}", bypass);
                    response_tx.send(ProcessingResponse::Ok).ok();
                }
                ProcessingCommand::AddLoudnessAnalyzer { id, channels } => {
                    use crate::plugins::LoudnessMonitorPlugin;
                    match LoudnessMonitorPlugin::new(channels) {
                        Ok(plugin) => match state.add_analyzer(id.clone(), Box::new(plugin)) {
                            Ok(_) => {
                                log::debug!("[Processing Thread] Added loudness analyzer: {}", id);
                                response_tx.send(ProcessingResponse::Ok).ok();
                            }
                            Err(e) => {
                                log::info!(
                                    "[Processing Thread] Failed to add loudness analyzer: {}",
                                    e
                                );
                                response_tx.send(ProcessingResponse::Error(e)).ok();
                            }
                        },
                        Err(e) => {
                            log::info!(
                                "[Processing Thread] Failed to create loudness analyzer: {}",
                                e
                            );
                            response_tx.send(ProcessingResponse::Error(e)).ok();
                        }
                    }
                }
                ProcessingCommand::AddSpectrumAnalyzer { id, channels } => {
                    use crate::plugins::SpectrumAnalyzerPlugin;
                    match SpectrumAnalyzerPlugin::new(channels) {
                        Ok(plugin) => match state.add_analyzer(id.clone(), Box::new(plugin)) {
                            Ok(_) => {
                                log::debug!("[Processing Thread] Added spectrum analyzer: {}", id);
                                response_tx.send(ProcessingResponse::Ok).ok();
                            }
                            Err(e) => {
                                log::info!(
                                    "[Processing Thread] Failed to add spectrum analyzer: {}",
                                    e
                                );
                                response_tx.send(ProcessingResponse::Error(e)).ok();
                            }
                        },
                        Err(e) => {
                            log::info!(
                                "[Processing Thread] Failed to create spectrum analyzer: {}",
                                e
                            );
                            response_tx.send(ProcessingResponse::Error(e)).ok();
                        }
                    }
                }
                ProcessingCommand::RemoveAnalyzer(id) => match state.remove_analyzer(&id) {
                    Some(_) => {
                        log::debug!("[Processing Thread] Removed analyzer: {}", id);
                        response_tx.send(ProcessingResponse::Ok).ok();
                    }
                    None => {
                        let err = format!("Analyzer '{}' not found", id);
                        log::debug!("[Processing Thread] {}", err);
                        response_tx.send(ProcessingResponse::Error(err)).ok();
                    }
                },
                ProcessingCommand::GetAnalyzerData(analyzer_id) => {
                    // log::debug!("[Processing Thread] Get analyzer data: {}", analyzer_id);
                    match state.get_analyzer_data(&analyzer_id) {
                        Ok(data) => {
                            response_tx
                                .send(ProcessingResponse::AnalyzerData(data))
                                .ok();
                        }
                        Err(e) => {
                            response_tx.send(ProcessingResponse::Error(e)).ok();
                        }
                    }
                }
                ProcessingCommand::Stop => {
                    // Stop processing - just clear state
                    log::debug!("[Processing Thread] Stopped");
                }
                ProcessingCommand::Shutdown => {
                    log::debug!("[Processing Thread] Shutting down");
                    break;
                }
            }
        }

        // Process audio from decoder
        match decoder_rx.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(DecoderMessage::Frame(frame)) => {
                // Process frame
                // IMPORTANT: Output buffer size must match plugin chain output channels, not input!
                // Use output_channels() which accounts for pending hot-reload with channel changes
                let output_channels = state.output_channels();
                let output_samples = frame.num_frames * output_channels;
                let mut output = vec![0.0; output_samples];

                match state.process_frame(&frame.data, &mut output) {
                    Ok(_) => {
                        // Send processed frame with correct output channel count
                        let processed_frame = super::AudioFrame::new(
                            output,
                            frame.num_frames,
                            output_channels,
                            frame.sample_rate,
                        );
                        message_tx
                            .send(ProcessingMessage::Frame(processed_frame))
                            .ok();
                    }
                    Err(e) => {
                        log::debug!("[Processing Thread] Processing error: {}", e);
                        event_tx.send(ThreadEvent::ProcessingError(e)).ok();
                    }
                }
            }
            Ok(DecoderMessage::EndOfStream) => {
                message_tx.send(ProcessingMessage::EndOfStream).ok();
            }
            Ok(DecoderMessage::Flush) => {
                message_tx.send(ProcessingMessage::Flush).ok();
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No message, continue
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                log::debug!("[Processing Thread] Decoder queue disconnected");
                break;
            }
        }
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
fn build_plugin_host(
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
    use crate::plugins::{
        BinauralDecoderPlugin, CompressorPlugin, EqPlugin, GainPlugin, GatePlugin,
        InPlacePluginAdapter, LimiterPlugin, LoudnessCompensationPlugin, MatrixPlugin,
        UpmixerPlugin,
    };

    match plugin_type {
        "gain" => {
            let params: GainPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse gain plugin parameters: {}", e))?;

            let plugin = GainPlugin::from_params(channels, params);
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

        "gate" => {
            let params: GatePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse gate plugin parameters: {}", e))?;

            let plugin = GatePlugin::from_params(channels, params);
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

            let plugin = LoudnessCompensationPlugin::from_params(channels, params);
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
            }

            let params: MatrixPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse matrix plugin parameters: {}", e))?;

            // Determine if using sparse or dense mapping
            let plugin = if let (Some(in_map), Some(out_map)) =
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

            Ok(Box::new(plugin))
        }

        "binaural_decoder" => {
            use crate::plugins::BinauralDecoderParams;

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

        // HAL plugins (macOS only)
        #[cfg(target_os = "macos")]
        "hal_input" => {
            use crate::plugins::{HalInputPlugin, HalInputPluginParams};

            let params: HalInputPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse HAL input plugin parameters: {}", e))?;

            let plugin = HalInputPlugin::from_params(params)?;
            Ok(Box::new(plugin))
        }

        #[cfg(target_os = "macos")]
        "hal_output" => {
            use crate::plugins::{HalOutputPlugin, HalOutputPluginParams, InPlacePluginAdapter};

            let params: HalOutputPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse HAL output plugin parameters: {}", e))?;

            let plugin = HalOutputPlugin::from_params(channels, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        other => Err(format!("Unknown plugin type: {}", other)),
    }
}
