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

const SPIN_MS_SIGNAL: u64 = 10;

/// Helper to send a message with backpressure handling and interruption support
fn send_or_interrupt<T>(
    tx: &SyncSender<T>,
    rx: &Receiver<ProcessingCommand>,
    mut msg: T,
) -> Result<Option<ProcessingCommand>, String> {
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
    /// Number of channels
    channels: usize,
    bypassed: bool,
    process_buffer: Vec<f32>,
}

impl ProcessingState {
    fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            host: PluginHost::new(channels, sample_rate),
            channels,
            bypassed: false,
            process_buffer: Vec::new(),
        }
    }

    /// Get the actual output channel count
    fn output_channels(&self) -> usize {
        self.host.output_channels()
    }

    /// Process a frame
    fn process_frame(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), String> {
        if self.bypassed {
            // Bypass - just copy
            output.copy_from_slice(input);
            return Ok(());
        }

        // Normal processing
        self.host.process(input, output)?;

        Ok(())
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

            // Swap host
            state.host = new_host;
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
        match decoder_rx.recv_timeout(std::time::Duration::from_millis(SPIN_MS_SIGNAL)) {
            Ok(DecoderMessage::Frame(frame)) => {
                let output_channels = state.output_channels();
                let output_samples = frame.num_frames * output_channels;

                let mut process_buffer = std::mem::take(&mut state.process_buffer);
                if process_buffer.len() != output_samples {
                    process_buffer.resize(output_samples, 0.0);
                }

                let start_time = std::time::Instant::now();
                match state.process_frame(&frame.data, &mut process_buffer) {
                    Ok(_) => {
                        let elapsed = start_time.elapsed();
                        if elapsed > std::time::Duration::from_millis(5) {
                            log::warn!(
                                "[Processing Thread] Slow processing: {:.2}ms for {} frames",
                                elapsed.as_secs_f64() * 1000.0,
                                frame.num_frames
                            );
                        }

                        let processed_frame = super::AudioFrame::new(
                            process_buffer.clone(),
                            frame.num_frames,
                            output_channels,
                            frame.sample_rate,
                        );

                        match send_or_interrupt(
                            &message_tx,
                            &command_rx,
                            ProcessingMessage::Frame(processed_frame),
                        ) {
                            Ok(Some(cmd)) => {
                                if handle_processing_command(cmd, &mut state, &response_tx) {
                                    break;
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                log::debug!("[Processing Thread] Send error: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("[Processing Thread] Processing error: {}", e);
                        event_tx.send(ThreadEvent::ProcessingError(e)).ok();
                    }
                }
                state.process_buffer = process_buffer;
            }
            Ok(DecoderMessage::EndOfStream) => {
                match send_or_interrupt(&message_tx, &command_rx, ProcessingMessage::EndOfStream) {
                    Ok(Some(cmd)) => {
                        if handle_processing_command(cmd, &mut state, &response_tx) {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
            Ok(DecoderMessage::Flush) => {
                match send_or_interrupt(&message_tx, &command_rx, ProcessingMessage::Flush) {
                    Ok(Some(cmd)) => {
                        if handle_processing_command(cmd, &mut state, &response_tx) {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
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
        BinauralDecoderPlugin, CompressorPlugin, EqPlugin, ExpanderPlugin, GainPlugin, GatePlugin,
        InPlacePluginAdapter, LimiterPlugin, LoudnessCompensationPlugin, MatrixPlugin,
        MultibandCompressorPlugin, MultibandExpanderPlugin, UpmixerPlugin,
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
            use sotf_plugins::{HalOutputPlugin, HalOutputPluginParams, InPlacePluginAdapter};

            let params: HalOutputPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse HAL output plugin parameters: {}", e))?;

            let plugin = HalOutputPlugin::from_params(channels, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
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

        other => Err(format!("Unknown plugin type: {}", other)),
    }
}
