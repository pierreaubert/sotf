//! Shared plugin factory.
//!
//! Creates plugin instances from a type string and JSON parameters.
//! Used by the audio engine and by the A/B Compare plugin's sub-rack builder.

use crate::{
    ABComparePlugin, ABComparePluginParams, AaePlugin, AaePluginParams, AecPlugin, AecPluginParams,
    BandMergePlugin, BandMergePluginParams, BandSplitPlugin, BandSplitPluginParams,
    BeamformerPlugin, BeamformerPluginParams, BinauralDecoderParams, BinauralDecoderPlugin,
    ChannelMuteSoloParams, ChannelMuteSoloPlugin, CompressorPlugin, CompressorPluginParams,
    ConvolutionPlugin, ConvolutionPluginParams, CrossfeedPlugin, CrossfeedPluginParams,
    CrossoverPlugin, CrossoverPluginParams, DeEsserPlugin, DeEsserPluginParams, DeclickPlugin,
    DeclickPluginParams, DelayPlugin, DelayPluginParams, DenoiserPlugin, DenoiserPluginParams,
    DownmixPlugin, DownmixPluginParams, DynamicEqPlugin, DynamicEqPluginParams, EqPlugin,
    EqPluginParams, ExpanderPlugin, ExpanderPluginParams, FirDesignerPlugin,
    FirDesignerPluginParams, GainPlugin, GainPluginParams, GatePlugin, GatePluginParams,
    HissReducerPlugin, HissReducerPluginParams, InPlacePluginAdapter, LimiterPlugin,
    LimiterPluginParams, LinearPhaseEqPlugin, LinearPhaseEqPluginParams,
    LoudnessCompensationPlugin, LoudnessCompensationPluginParams, LoudnessMonitorPlugin,
    MatrixPlugin, MonoToStereoPlugin, MonoToStereoPluginParams, MultibandCompressorPlugin,
    MultibandCompressorPluginParams, MultibandExpanderPlugin, MultibandExpanderPluginParams,
    Plugin, PndPlugin, PndPluginParams, ResamplerPlugin, SaturationPlugin, SaturationPluginParams,
    SpectralCompressorPlugin, SpectralCompressorPluginParams, SpectrumAnalyzerPlugin,
    SpectrumConfig, SpeechDenoiserPlugin, SpeechDenoiserPluginParams, StereoImagerPlugin,
    StereoImagerPluginParams, TransientShaperPlugin, TransientShaperPluginParams, UpmixerPlugin,
    UpmixerPluginParams, XtcPlugin, XtcPluginParams,
};
use crate::{ExternalPlugin, PluginDescriptor, PluginFormat};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::{
    ExternalPluginSandboxPolicy, ExternalPluginSandboxTiming, ExternalPluginTrust,
    ExternalPluginWorkerCommand, IsolatedExternalPlugin, IsolatedExternalPluginConfig,
};
use std::path::{Path, PathBuf};

/// Plugin type strings accepted by [`create_plugin`].
pub const SUPPORTED_PLUGIN_TYPES: &[&str] = &[
    "gain",
    "eq",
    "parametric_eq",
    "compressor",
    "expander",
    "limiter",
    "gate",
    "delay",
    "convolution",
    "upmixer",
    "aae",
    "active_acoustic_enhancement",
    "downmix",
    "mono_to_stereo",
    "multiband_compressor",
    "multiband_expander",
    "de_esser",
    "dynamic_eq",
    "fir_designer",
    "linear_phase_eq",
    "spectral_compressor",
    "stereo_imager",
    "transient_shaper",
    "saturation",
    "loudness_compensation",
    "fletcher_munson",
    "crossfeed",
    "xtc",
    "crosstalk_cancellation",
    "denoiser",
    "wiener_denoiser",
    "speech_denoiser",
    "rnnoise",
    "rnnoise_denoiser",
    "hiss_reducer",
    "hiss",
    "declick",
    "transient_repair",
    "pnd",
    "varispeed",
    "binaural_decoder",
    "crossover",
    "matrix",
    "channel_mute_solo",
    "loudness_monitor",
    "spectrum_analyzer",
    "resampler",
    "band_split",
    "band_merge",
    "ab_compare",
    "ab",
    "aec",
    "beamformer",
    "ambisonics_decoder",
    "external",
    "external_plugin",
    #[cfg(all(target_os = "macos", feature = "hal"))]
    "hal_input",
    #[cfg(all(target_os = "macos", feature = "hal"))]
    "hal_output",
];

pub fn is_supported_plugin_type(plugin_type: &str) -> bool {
    let lower = plugin_type.to_lowercase();
    SUPPORTED_PLUGIN_TYPES.contains(&lower.as_str())
}

/// Create a plugin instance from its type string and JSON parameters.
///
/// Supports all plugin types in the SOTF ecosystem. This is the single
/// authoritative factory -- both the audio engine and the A/B Compare
/// plugin's sub-rack builder delegate to this function.
pub fn create_plugin(
    plugin_type: &str,
    parameters: &serde_json::Value,
    channels: usize,
    sample_rate: u32,
) -> Result<Box<dyn Plugin>, String> {
    match plugin_type {
        "gain" => {
            let params: GainPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse gain params: {e}"))?;
            let plugin = GainPlugin::from_params(channels, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "eq" | "parametric_eq" => {
            let params: EqPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse EQ params: {e}"))?;
            let plugin = EqPlugin::from_params(channels, sample_rate, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "compressor" => {
            let params: CompressorPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse compressor params: {e}"))?;
            let plugin = CompressorPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "expander" => {
            let params: ExpanderPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse expander params: {e}"))?;
            let plugin = ExpanderPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "limiter" => {
            let params: LimiterPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse limiter params: {e}"))?;
            let plugin = LimiterPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "gate" => {
            let params: GatePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse gate params: {e}"))?;
            let plugin = GatePlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "delay" => {
            let params: DelayPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse delay params: {e}"))?;
            let plugin = DelayPlugin::from_params(channels, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "convolution" => {
            let params: ConvolutionPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse convolution params: {e}"))?;
            let plugin = ConvolutionPlugin::from_params(channels, sample_rate, params)
                .map_err(|e| format!("Failed to create convolution plugin: {e}"))?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "upmixer" => {
            if channels != 2 {
                return Err(format!("Upmixer requires 2 input channels, got {channels}"));
            }
            let params: UpmixerPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse upmixer params: {e}"))?;
            let plugin = UpmixerPlugin::from_params(params);
            Ok(Box::new(plugin))
        }

        "aae" | "active_acoustic_enhancement" => {
            if channels != 2 {
                return Err(format!("AAE requires 2 input channels, got {channels}"));
            }
            let params: AaePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse AAE params: {e}"))?;
            let mut plugin = AaePlugin::from_params(params);
            plugin.initialize(sample_rate)?;
            Ok(Box::new(plugin))
        }

        "downmix" => {
            let mut params: DownmixPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse downmix params: {e}"))?;
            if params.input_channels != channels {
                log::info!(
                    "[factory:downmix] Adapting input_channels from {} to current chain width {}",
                    params.input_channels,
                    channels
                );
                params.input_channels = channels;
            }
            let plugin = DownmixPlugin::from_params(params);
            Ok(Box::new(plugin))
        }

        "mono_to_stereo" => {
            let params: MonoToStereoPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse mono_to_stereo params: {e}"))?;
            let plugin = MonoToStereoPlugin::from_params(channels, params);
            Ok(Box::new(plugin))
        }

        "multiband_compressor" => {
            let params: MultibandCompressorPluginParams =
                serde_json::from_value(parameters.clone())
                    .map_err(|e| format!("Failed to parse multiband compressor params: {e}"))?;
            let plugin = MultibandCompressorPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "multiband_expander" => {
            let params: MultibandExpanderPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse multiband expander params: {e}"))?;
            let plugin = MultibandExpanderPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "de_esser" => {
            let params: DeEsserPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse de-esser params: {e}"))?;
            let plugin = DeEsserPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "dynamic_eq" => {
            let params: DynamicEqPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse dynamic EQ params: {e}"))?;
            let plugin = DynamicEqPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "fir_designer" => {
            let params: FirDesignerPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse FIR designer params: {e}"))?;
            let plugin = FirDesignerPlugin::from_params(channels, sample_rate, params)
                .map_err(|e| format!("Failed to create FIR designer plugin: {e}"))?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "linear_phase_eq" => {
            let params: LinearPhaseEqPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse linear-phase EQ params: {e}"))?;
            let plugin = LinearPhaseEqPlugin::from_params(channels, sample_rate, params)
                .map_err(|e| format!("Failed to create linear-phase EQ plugin: {e}"))?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "spectral_compressor" => {
            let params: SpectralCompressorPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse spectral compressor params: {e}"))?;
            let plugin = SpectralCompressorPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "stereo_imager" => {
            let params: StereoImagerPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse stereo imager params: {e}"))?;
            let plugin = StereoImagerPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "transient_shaper" => {
            let params: TransientShaperPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse transient shaper params: {e}"))?;
            let plugin = TransientShaperPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "saturation" => {
            let params: SaturationPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse saturation params: {e}"))?;
            let plugin = SaturationPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "loudness_compensation" => {
            let params: LoudnessCompensationPluginParams =
                serde_json::from_value(parameters.clone())
                    .map_err(|e| format!("Failed to parse loudness compensation params: {e}"))?;
            let plugin = LoudnessCompensationPlugin::from_params(channels, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "fletcher_munson" => {
            use crate::plugin_loudness_compensation::FletcherMunsonCompat;
            let fm: FletcherMunsonCompat = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse Fletcher-Munson params: {e}"))?;
            let plugin = LoudnessCompensationPlugin::from_params(
                channels,
                fm.into_loudness_compensation_params(),
            )?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "crossfeed" => {
            if channels != 2 {
                return Err(format!(
                    "Crossfeed requires 2 input channels (stereo), got {channels}"
                ));
            }
            let params: CrossfeedPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse crossfeed params: {e}"))?;
            let plugin = CrossfeedPlugin::new(params)
                .map_err(|e| format!("Failed to create crossfeed plugin: {e}"))?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "xtc" | "crosstalk_cancellation" => {
            if channels != 2 {
                return Err(format!(
                    "XTC requires 2 input channels (stereo), got {channels}"
                ));
            }
            let params: XtcPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse XTC params: {e}"))?;
            let plugin = XtcPlugin::from_params(params, sample_rate)?;
            Ok(Box::new(plugin))
        }

        "denoiser" | "wiener_denoiser" => {
            let params: DenoiserPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse denoiser params: {e}"))?;
            let plugin = DenoiserPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "speech_denoiser" | "rnnoise" | "rnnoise_denoiser" => {
            let params: SpeechDenoiserPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse speech denoiser params: {e}"))?;
            let plugin = SpeechDenoiserPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "hiss_reducer" | "hiss" => {
            let params: HissReducerPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse hiss reducer params: {e}"))?;
            let plugin = HissReducerPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "declick" | "transient_repair" => {
            let params: DeclickPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse declick params: {e}"))?;
            let plugin = DeclickPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "pnd" | "varispeed" => {
            let params: PndPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse PND params: {e}"))?;
            let plugin = PndPlugin::from_params(channels, params);
            Ok(Box::new(plugin))
        }

        "binaural_decoder" => {
            let params: BinauralDecoderParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse binaural decoder params: {e}"))?;
            let plugin = BinauralDecoderPlugin::from_params(params);
            Ok(Box::new(plugin))
        }

        "crossover" => {
            let params: CrossoverPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse crossover params: {e}"))?;
            let plugin = CrossoverPlugin::from_params(channels, &params)?;
            Ok(Box::new(plugin))
        }

        "matrix" => create_matrix_plugin(parameters, channels),

        "channel_mute_solo" => {
            let params: ChannelMuteSoloParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse channel_mute_solo params: {e}"))?;
            let plugin = ChannelMuteSoloPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "loudness_monitor" => {
            let plugin = LoudnessMonitorPlugin::new(channels)
                .map_err(|e| format!("Failed to create loudness monitor: {e}"))?;
            Ok(Box::new(plugin))
        }

        "spectrum_analyzer" => {
            let config: SpectrumConfig = if parameters.is_null() {
                SpectrumConfig::default()
            } else {
                serde_json::from_value(parameters.clone())
                    .map_err(|e| format!("Failed to parse spectrum analyzer params: {e}"))?
            };
            let plugin = SpectrumAnalyzerPlugin::with_config(channels, config)
                .map_err(|e| format!("Failed to create spectrum analyzer: {e}"))?;
            Ok(Box::new(plugin))
        }

        "resampler" => {
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
                .map_err(|e| format!("Failed to parse resampler params: {e}"))?;
            let plugin = ResamplerPlugin::new(
                channels,
                params.input_sample_rate,
                params.output_sample_rate,
                params.chunk_size,
            )
            .map_err(|e| format!("Failed to create resampler: {e}"))?;
            Ok(Box::new(plugin))
        }

        "band_split" => {
            let params: BandSplitPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse band_split params: {e}"))?;
            let plugin = BandSplitPlugin::from_params(channels, &params)?;
            Ok(Box::new(plugin))
        }

        "band_merge" => {
            let params: BandMergePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse band_merge params: {e}"))?;
            if params.bands == 0 {
                return Err("BandMerge requires at least 1 band".to_string());
            }
            let output_channels = channels / params.bands;
            let plugin = BandMergePlugin::from_params(output_channels, &params)?;
            Ok(Box::new(plugin))
        }

        "ab_compare" | "ab" => {
            let params: ABComparePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse A/B compare params: {e}"))?;
            let mut plugin = ABComparePlugin::from_params(channels, params)?;
            // Inject the shared factory so sub-racks support all plugin types
            plugin.set_plugin_factory(create_plugin);
            plugin.initialize(sample_rate)?;
            Ok(Box::new(plugin))
        }

        "aec" => {
            let params: AecPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse AEC params: {e}"))?;
            let plugin = AecPlugin::from_params(sample_rate, params);
            Ok(Box::new(plugin))
        }

        "beamformer" => {
            let params: BeamformerPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse beamformer params: {e}"))?;
            let plugin = BeamformerPlugin::from_params(sample_rate, params);
            Ok(Box::new(plugin))
        }

        "ambisonics_decoder" => {
            let config: sotf_plugin_ambisonics::AmbisonicsDecoderConfig =
                serde_json::from_value(parameters.clone())
                    .map_err(|e| format!("Failed to parse ambisonics decoder params: {e}"))?;
            let mut plugin = sotf_plugin_ambisonics::AmbisonicsDecoderPlugin::new(&config)?;
            plugin.initialize(sample_rate)?;
            Ok(Box::new(plugin))
        }

        "external" | "external_plugin" => {
            let descriptor = parse_external_plugin_descriptor(parameters)
                .map_err(|e| format!("Failed to parse external plugin descriptor: {e}"))?;
            if descriptor.audio_inputs != 0 && descriptor.audio_inputs != channels {
                return Err(format!(
                    "External plugin '{}' requires {} input channels, got {channels}",
                    descriptor.name, descriptor.audio_inputs
                ));
            }

            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            {
                let plugin_trust = external_plugin_trust(parameters)?;
                if external_plugin_isolation_requested(parameters, plugin_trust) {
                    let plugin = IsolatedExternalPlugin::new(
                        descriptor,
                        sample_rate,
                        parse_isolated_external_plugin_config(parameters, plugin_trust)?,
                    )
                    .map_err(|e| format!("Failed to create isolated external plugin: {e}"))?;
                    return Ok(Box::new(plugin));
                }
            }

            let plugin = ExternalPlugin::new(&descriptor, sample_rate)
                .map_err(|e| format!("Failed to load external plugin: {e}"))?;
            Ok(Box::new(plugin))
        }

        #[cfg(all(target_os = "macos", feature = "hal"))]
        "hal_input" => {
            use crate::{HalInputPlugin, HalInputPluginParams};
            let params: HalInputPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse HAL input params: {e}"))?;
            let plugin = HalInputPlugin::from_params(params)?;
            Ok(Box::new(plugin))
        }

        #[cfg(all(target_os = "macos", feature = "hal"))]
        "hal_output" => {
            use crate::{HalOutputPlugin, HalOutputPluginParams};
            let params: HalOutputPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse HAL output params: {e}"))?;
            let plugin = HalOutputPlugin::from_params(params)?;
            Ok(Box::new(plugin))
        }

        other => Err(format!("Unknown plugin type: {other}")),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn external_plugin_isolation_requested(
    parameters: &serde_json::Value,
    trust: ExternalPluginTrust,
) -> bool {
    if let Some(isolated) = parameters.get("isolated").and_then(serde_json::Value::as_bool) {
        return isolated;
    }

    if let Some(isolation) = parameters.get("isolation").and_then(serde_json::Value::as_str) {
        let isolation = isolation.to_ascii_lowercase();
        if isolation == "disabled" || isolation == "off" || isolation == "false" {
            return false;
        }
        if matches!(
            isolation.as_str(),
            "process" | "subprocess" | "out_of_process" | "isolated" | "always"
        ) {
            return true;
        }
    }

    // Default to isolated execution for external plugins so unknown code runs in a
    // dedicated worker process unless explicitly disabled. Signed plugins still
    // inherit a relaxed sandbox profile via `plugin_trust`.
    let _ = trust;
    true
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn external_plugin_trust(parameters: &serde_json::Value) -> Result<ExternalPluginTrust, String> {
    if let Some(trust) = parameters
        .get("plugin_trust")
        .or_else(|| parameters.get("trust"))
    {
        let trust = trust
            .as_str()
            .ok_or_else(|| "`plugin_trust` must be a string".to_string())?;
        trust.parse::<ExternalPluginTrust>()
    } else {
        Ok(ExternalPluginTrust::Unknown)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn parse_isolated_external_plugin_config(
    parameters: &serde_json::Value,
    trust: ExternalPluginTrust,
) -> Result<IsolatedExternalPluginConfig, String> {
    let mut config = IsolatedExternalPluginConfig {
        sandbox_policy: ExternalPluginSandboxPolicy::for_trust(trust),
        ..IsolatedExternalPluginConfig::default()
    };

    if let Some(max_block_frames) = parameters.get("max_block_frames") {
        let max_block_frames = max_block_frames
            .as_u64()
            .ok_or_else(|| "`max_block_frames` must be an integer".to_string())?;
        config.max_block_frames = u32::try_from(max_block_frames)
            .map_err(|_| "`max_block_frames` is too large".to_string())?;
    }

    if let Some(deadline_micros) = parameters.get("deadline_micros") {
        let deadline_micros = deadline_micros
            .as_u64()
            .ok_or_else(|| "`deadline_micros` must be an integer".to_string())?;
        config.deadline = std::time::Duration::from_micros(deadline_micros);
    }

    if let Some(start_worker) = parameters.get("start_worker") {
        config.start_worker = start_worker
            .as_bool()
            .ok_or_else(|| "`start_worker` must be a boolean".to_string())?;
    }

    if let Some(worker_path) = parameters
        .get("worker_path")
        .or_else(|| parameters.get("worker_binary"))
    {
        let worker_path = worker_path
            .as_str()
            .ok_or_else(|| "`worker_path` must be a string".to_string())?;
        if worker_path.is_empty() {
            return Err("`worker_path` must not be empty".to_string());
        }
        config.worker_command = ExternalPluginWorkerCommand::new(worker_path);
    }

    if let Some(worker_args) = parameters.get("worker_args") {
        let worker_args = worker_args
            .as_array()
            .ok_or_else(|| "`worker_args` must be an array".to_string())?;
        let mut args = Vec::with_capacity(worker_args.len());
        for arg in worker_args {
            let arg = arg
                .as_str()
                .ok_or_else(|| "`worker_args` entries must be strings".to_string())?;
            args.push(arg.to_string());
        }
        config.worker_command = config.worker_command.clone().args(args);
    }

    if let Some(worker_env) = parameters.get("worker_env") {
        let worker_env = worker_env
            .as_object()
            .ok_or_else(|| "`worker_env` must be an object".to_string())?;
        for (key, value) in worker_env {
            let value = value
                .as_str()
                .ok_or_else(|| "`worker_env` values must be strings".to_string())?;
            config.worker_command = config.worker_command.clone().env(key, value);
        }
    }

    if let Some(sandbox_timing) = parameters.get("sandbox_timing") {
        let sandbox_timing = sandbox_timing
            .as_str()
            .ok_or_else(|| "`sandbox_timing` must be a string".to_string())?;
        config.sandbox_policy.timing = sandbox_timing.parse::<ExternalPluginSandboxTiming>()?;
    }

    if let Some(sandbox_required) = parameters.get("sandbox_required") {
        config.sandbox_policy.require_platform_sandbox = sandbox_required
            .as_bool()
            .ok_or_else(|| "`sandbox_required` must be a boolean".to_string())?;
    }

    if let Some(allow_network) = parameters.get("sandbox_allow_network") {
        config.sandbox_policy.allow_network = allow_network
            .as_bool()
            .ok_or_else(|| "`sandbox_allow_network` must be a boolean".to_string())?;
    }

    if let Some(allow_child_processes) = parameters.get("sandbox_allow_child_processes") {
        config.sandbox_policy.allow_child_processes = allow_child_processes
            .as_bool()
            .ok_or_else(|| "`sandbox_allow_child_processes` must be a boolean".to_string())?;
    }

    parse_sandbox_paths(
        parameters,
        "sandbox_read_paths",
        &mut config.sandbox_policy.extra_read_paths,
    )?;
    parse_sandbox_paths(
        parameters,
        "sandbox_write_paths",
        &mut config.sandbox_policy.extra_write_paths,
    )?;

    Ok(config)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn parse_sandbox_paths(
    parameters: &serde_json::Value,
    key: &str,
    paths: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let Some(value) = parameters.get(key) else {
        return Ok(());
    };

    let entries = value
        .as_array()
        .ok_or_else(|| format!("`{key}` must be an array"))?;
    paths.reserve(entries.len());
    for entry in entries {
        let path = entry
            .as_str()
            .ok_or_else(|| format!("`{key}` entries must be strings"))?;
        if path.is_empty() {
            return Err(format!("`{key}` entries must not be empty"));
        }
        paths.push(PathBuf::from(path));
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct ExternalPluginDescriptorSeed {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    vendor: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    path: Option<PathBuf>,
    #[serde(default)]
    audio_inputs: Option<usize>,
    #[serde(default)]
    audio_outputs: Option<usize>,
    #[serde(default)]
    is_instrument: Option<bool>,
    #[serde(default)]
    categories: Option<Vec<String>>,
    #[serde(default)]
    format: Option<String>,
}

fn parse_external_plugin_descriptor(
    parameters: &serde_json::Value,
) -> Result<PluginDescriptor, String> {
    let seed = if let Some(descriptor) = parameters.get("descriptor") {
        if !descriptor.is_object() {
            return Err("`descriptor` must be an object".to_string());
        }
        serde_json::from_value(descriptor.clone())
            .map_err(|e| format!("Failed to parse external plugin descriptor: {e}"))?
    } else if let Some(path) = parameters.as_str() {
        ExternalPluginDescriptorSeed {
            id: None,
            name: None,
            vendor: None,
            version: None,
            path: Some(path.into()),
            audio_inputs: None,
            audio_outputs: None,
            is_instrument: None,
            categories: None,
            format: None,
        }
    } else {
        serde_json::from_value(parameters.clone())
            .map_err(|e| format!("Failed to parse external plugin parameters: {e}"))?
    };

    let path = seed
        .path
        .ok_or_else(|| "External plugin descriptor is missing required `path`".to_string())?;
    let path = path.canonicalize().unwrap_or(path);
    let format = parse_external_format(seed.format, &path)?;
    let fallback_name = fallback_name_from_path(&path)?;
    let name = seed.name.unwrap_or_else(|| fallback_name.clone());
    let id = seed
        .id
        .unwrap_or_else(|| format!("{}.{}", format.extension(), fallback_name));
    let vendor = seed.vendor.unwrap_or_else(|| "Unknown".to_string());
    let version = seed.version.unwrap_or_else(|| "Unknown".to_string());
    let audio_inputs = seed.audio_inputs.unwrap_or(2);
    let audio_outputs = seed.audio_outputs.unwrap_or(2);
    let categories = seed.categories.unwrap_or_default();

    Ok(PluginDescriptor {
        id,
        name,
        vendor,
        version,
        format,
        path,
        audio_inputs,
        audio_outputs: audio_outputs.max(1),
        is_instrument: seed.is_instrument.unwrap_or(false),
        categories,
    })
}

fn parse_external_format(format: Option<String>, path: &Path) -> Result<PluginFormat, String> {
    if let Some(format) = format {
        parse_external_format_name(&format)
            .ok_or_else(|| format!("Unknown external plugin format '{format}'"))
    } else {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| {
                "Unable to infer external plugin format from missing extension".to_string()
            })?;
        parse_external_format_name(extension).ok_or_else(|| {
            format!("Unable to infer external plugin format from extension '{extension}'")
        })
    }
}

fn parse_external_format_name(format: &str) -> Option<PluginFormat> {
    match format.to_ascii_lowercase().as_str() {
        "clap" => Some(PluginFormat::Clap),
        "vst3" => Some(PluginFormat::Vst3),
        "component" | "audiounit" | "au" => Some(PluginFormat::AudioUnit),
        _ => None,
    }
}

fn fallback_name_from_path(path: &Path) -> Result<String, String> {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| "External plugin path has no file name".to_string())
}

/// Create a matrix plugin (complex logic with auto-resize).
fn create_matrix_plugin(
    parameters: &serde_json::Value,
    channels: usize,
) -> Result<Box<dyn Plugin>, String> {
    #[derive(Debug, Clone, serde::Deserialize)]
    struct MatrixPluginParams {
        #[serde(default)]
        input_channels: Option<usize>,
        #[serde(default)]
        output_channels: Option<usize>,
        #[serde(default)]
        input_channel_map: Option<Vec<usize>>,
        #[serde(default)]
        output_channel_map: Option<Vec<usize>>,
        matrix: Vec<f32>,
        #[serde(default)]
        channel_states: Option<Vec<crate::ChannelState>>,
    }

    let params: MatrixPluginParams = serde_json::from_value(parameters.clone())
        .map_err(|e| format!("Failed to parse matrix params: {e}"))?;

    let mut plugin = if let (Some(in_map), Some(out_map)) =
        (params.input_channel_map, params.output_channel_map)
    {
        MatrixPlugin::with_sparse_mapping(in_map, out_map, params.matrix)
            .map_err(|e| format!("Failed to create sparse matrix plugin: {e}"))?
    } else if let (Some(in_ch), Some(out_ch)) = (params.input_channels, params.output_channels) {
        if in_ch == out_ch && in_ch != channels {
            log::info!(
                "[factory:matrix] Resizing square matrix from {in_ch}x{out_ch} to {channels}x{channels}"
            );
            let mut matrix = params.matrix;
            resize_matrix(&mut matrix, in_ch, out_ch, channels, channels);
            MatrixPlugin::with_matrix(channels, channels, matrix)
                .map_err(|e| format!("Failed to create resized matrix plugin: {e}"))?
        } else {
            MatrixPlugin::with_matrix(in_ch, out_ch, params.matrix)
                .map_err(|e| format!("Failed to create matrix plugin: {e}"))?
        }
    } else {
        return Err(
            "Matrix plugin requires either (input_channels, output_channels) \
             or (input_channel_map, output_channel_map)"
                .to_string(),
        );
    };

    if let Some(mut states) = params.channel_states {
        let needed = plugin.output_channels();
        if states.len() != needed {
            log::info!(
                "[factory] Resizing channel_states from {} to {needed}",
                states.len()
            );
            states.resize(needed, crate::ChannelState::default());
        }
        plugin = plugin.with_channel_states(states);
    }

    Ok(Box::new(plugin))
}

/// Resize a matrix to new dimensions, preserving existing values and filling
/// new diagonal entries with 1.0.
fn resize_matrix(
    matrix: &mut Vec<f32>,
    old_in: usize,
    old_out: usize,
    new_in: usize,
    new_out: usize,
) {
    let mut new_matrix = vec![0.0; new_in * new_out];
    for out in 0..old_out.min(new_out) {
        for inp in 0..old_in.min(new_in) {
            new_matrix[out * new_in + inp] = matrix[out * old_in + inp];
        }
    }
    for i in old_in.min(old_out)..new_in.min(new_out) {
        new_matrix[i * new_in + i] = 1.0;
    }
    *matrix = new_matrix;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn supported_plugin_type_list_covers_factory_aliases() {
        assert!(is_supported_plugin_type("gain"));
        assert!(is_supported_plugin_type("EQ"));
        assert!(is_supported_plugin_type("rnnoise"));
        assert!(is_supported_plugin_type("active_acoustic_enhancement"));
        assert!(is_supported_plugin_type("external"));
        assert!(is_supported_plugin_type("external_plugin"));
        assert!(!is_supported_plugin_type("definitely_missing"));
    }

    #[test]
    fn create_external_plugin_from_path() {
        let dir = tempdir().unwrap();
        let plugin_path = dir.path().join("external-test-plugin.clap");
        std::fs::write(&plugin_path, b"stub plugin").unwrap();
        let params = serde_json::json!({
            "path": plugin_path.to_string_lossy(),
            "audio_inputs": 2,
            "audio_outputs": 2,
            "name": "External Test",
            "format": "clap",
            "plugin_trust": "signed",
        });

        let plugin = create_plugin("external", &params, 2, 48_000).unwrap();
        assert_eq!(plugin.input_channels(), 2);
    }

    #[test]
    fn create_external_plugin_from_path_string() {
        let dir = tempdir().unwrap();
        let plugin_path = dir.path().join("external-test-plugin-string.clap");
        std::fs::write(&plugin_path, b"stub plugin").unwrap();

        let plugin = create_plugin(
            "external",
            &serde_json::json!({
                "path": plugin_path.to_string_lossy(),
                "audio_inputs": 2,
                "audio_outputs": 2,
                "name": "External Test",
                "format": "clap",
                "plugin_trust": "signed",
            }),
            2,
            48_000,
        )
        .unwrap();
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn create_external_plugin_from_embedded_descriptor() {
        let dir = tempdir().unwrap();
        let plugin_path = dir.path().join("external-test-plugin.clap");
        std::fs::write(&plugin_path, b"stub plugin").unwrap();
        let descriptor = PluginDescriptor {
            id: "test.external".into(),
            name: "Embedded External Test".into(),
            vendor: "Test".into(),
            version: "0.1.0".into(),
            format: PluginFormat::Clap,
            path: plugin_path.clone(),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec!["testing".into()],
        };

        let plugin = create_plugin(
            "external_plugin",
            &serde_json::json!({"descriptor": descriptor, "plugin_trust": "signed"}),
            2,
            48_000,
        )
        .unwrap();
        assert_eq!(plugin.output_channels(), 2);
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[test]
    fn create_external_plugin_defaults_to_isolated_when_trust_unknown() {
        let dir = tempdir().unwrap();
        let plugin_path = dir.path().join("external-test-plugin-isolated.clap");
        std::fs::write(&plugin_path, b"stub plugin").unwrap();
        let params = serde_json::json!({
            "path": plugin_path.to_string_lossy(),
            "audio_inputs": 2,
            "audio_outputs": 2,
            "name": "External Isolated Test",
            "format": "clap",
            "plugin_trust": "unknown",
            "start_worker": false,
            "deadline_micros": 0,
        });

        let mut plugin = create_plugin("external", &params, 2, 48_000).unwrap();
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);

        let input = vec![0.25, -0.5, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];
        let frames = plugin
            .process(
                &input,
                &mut output,
                &sotf_host::ProcessContext::new(48_000, 2),
            )
            .unwrap();
        assert_eq!(frames, 2);
        assert_eq!(output, input);
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[test]
    fn create_external_plugin_can_opt_into_process_isolation() {
        let config = parse_isolated_external_plugin_config(&serde_json::json!({
            "worker_path": "/usr/bin/sotf-test-worker",
            "worker_args": ["--once", "--idle-sleep-micros", "250"],
            "worker_env": {
                "SOTF_TEST_WORKER": "1"
            },
            "start_worker": false,
            "plugin_trust": "signed",
            "deadline_micros": 250,
            "max_block_frames": 1024,
        }),
            ExternalPluginTrust::Signed)
        .unwrap();

        assert_eq!(
            config.worker_command.program(),
            Path::new("/usr/bin/sotf-test-worker")
        );
        assert_eq!(
            config.worker_command.command_args(),
            &[
                "--once".to_string(),
                "--idle-sleep-micros".to_string(),
                "250".to_string()
            ]
        );
        assert_eq!(
            config.worker_command.command_env(),
            &[("SOTF_TEST_WORKER".to_string(), "1".to_string())]
        );
        assert!(!config.start_worker);
        assert_eq!(config.deadline, std::time::Duration::from_micros(250));
        assert_eq!(config.max_block_frames, 1024);
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[test]
    fn isolated_external_plugin_config_maps_trust_to_sandbox_timing() {
        let signed = parse_isolated_external_plugin_config(&serde_json::json!({
            "plugin_trust": "signed",
            "sandbox_read_paths": ["/Library/Audio/Plug-Ins"],
            "sandbox_write_paths": ["/tmp/sotf-plugin-cache"],
        }),
            ExternalPluginTrust::Signed)
        .unwrap();
        assert_eq!(
            signed.sandbox_policy.timing,
            ExternalPluginSandboxTiming::AfterPluginLoad
        );
        assert!(!signed.sandbox_policy.require_platform_sandbox);
        assert_eq!(
            signed.sandbox_policy.extra_read_paths,
            vec![PathBuf::from("/Library/Audio/Plug-Ins")]
        );

        let untrusted = parse_isolated_external_plugin_config(&serde_json::json!({
            "plugin_trust": "untrusted"
        }),
            ExternalPluginTrust::Untrusted)
        .unwrap();
        assert_eq!(
            untrusted.sandbox_policy.timing,
            ExternalPluginSandboxTiming::BeforePluginLoad
        );
        assert_eq!(
            untrusted.sandbox_policy.require_platform_sandbox,
            cfg!(target_os = "linux")
        );
    }

    #[test]
    fn create_external_plugin_reports_invalid_parameters() {
        let err = match create_plugin(
            "external",
            &serde_json::json!({"audio_inputs": 2}),
            2,
            48_000,
        ) {
            Ok(_) => panic!("external plugin creation should fail"),
            Err(err) => err,
        };
        assert!(
            err.contains("External plugin descriptor is missing required `path`")
                || err.contains("path")
        );
    }
}
