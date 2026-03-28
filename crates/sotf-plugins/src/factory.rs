//! Shared plugin factory.
//!
//! Creates plugin instances from a type string and JSON parameters.
//! Used by the audio engine and by the A/B Compare plugin's sub-rack builder.

use crate::{
    ABComparePlugin, ABComparePluginParams, AecPlugin, AecPluginParams, BandMergePlugin,
    BandMergePluginParams, BandSplitPlugin, BandSplitPluginParams, BeamformerPlugin,
    BeamformerPluginParams, BinauralDecoderPlugin, BinauralDecoderParams, ChannelMuteSoloParams,
    ChannelMuteSoloPlugin, CompressorPlugin, CompressorPluginParams, ConvolutionPlugin,
    ConvolutionPluginParams, CrossfeedPlugin, CrossfeedPluginParams, CrossoverPlugin,
    CrossoverPluginParams, DeEsserPlugin, DeEsserPluginParams, DelayPlugin, DelayPluginParams,
    DenoiserPlugin, DenoiserPluginParams, DownmixPlugin, DownmixPluginParams, DynamicEqPlugin,
    DynamicEqPluginParams, EqPlugin, EqPluginParams, ExpanderPlugin, ExpanderPluginParams,
    GainPlugin, GainPluginParams, GatePlugin, GatePluginParams, InPlacePluginAdapter,
    LimiterPlugin, LimiterPluginParams, LinearPhaseEqPlugin, LinearPhaseEqPluginParams,
    LoudnessCompensationPlugin, LoudnessCompensationPluginParams, LoudnessMonitorPlugin,
    MatrixPlugin, MonoToStereoPlugin, MonoToStereoPluginParams, MultibandCompressorPlugin,
    MultibandCompressorPluginParams, MultibandExpanderPlugin, MultibandExpanderPluginParams,
    PndPlugin, PndPluginParams, Plugin, ResamplerPlugin, SaturationPlugin, SaturationPluginParams,
    SpectralCompressorPlugin, SpectralCompressorPluginParams, SpectrumAnalyzerPlugin,
    SpectrumConfig, StereoImagerPlugin, StereoImagerPluginParams, TransientShaperPlugin,
    TransientShaperPluginParams, UpmixerPlugin, UpmixerPluginParams, XtcPlugin, XtcPluginParams,
};

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
            let plugin = DelayPlugin::from_params(channels, params);
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

        "downmix" => {
            let params: DownmixPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse downmix params: {e}"))?;
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
            let params: MultibandExpanderPluginParams =
                serde_json::from_value(parameters.clone())
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

        "linear_phase_eq" => {
            let params: LinearPhaseEqPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse linear-phase EQ params: {e}"))?;
            let plugin = LinearPhaseEqPlugin::from_params(channels, sample_rate, params)
                .map_err(|e| format!("Failed to create linear-phase EQ plugin: {e}"))?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "spectral_compressor" => {
            let params: SpectralCompressorPluginParams =
                serde_json::from_value(parameters.clone())
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
            let plugin = LoudnessCompensationPlugin::from_params(channels, fm.into_loudness_compensation_params())?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "crossfeed" => {
            if channels != 2 {
                return Err(format!("Crossfeed requires 2 input channels (stereo), got {channels}"));
            }
            let params: CrossfeedPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse crossfeed params: {e}"))?;
            let plugin = CrossfeedPlugin::new(params)
                .map_err(|e| format!("Failed to create crossfeed plugin: {e}"))?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "xtc" | "crosstalk_cancellation" => {
            if channels != 2 {
                return Err(format!("XTC requires 2 input channels (stereo), got {channels}"));
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

        "matrix" => {
            create_matrix_plugin(parameters, channels)
        }

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
                    .unwrap_or_else(|_| SpectrumConfig::default())
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
            fn default_chunk_size() -> usize { 1024 }

            let params: ResamplerParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Failed to parse resampler params: {e}"))?;
            let plugin = ResamplerPlugin::new(channels, params.input_sample_rate, params.output_sample_rate, params.chunk_size)
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

        #[cfg(feature = "iamf")]
        "ambisonics_decoder" => {
            let config: sotf_plugin_ambisonics::AmbisonicsDecoderConfig =
                serde_json::from_value(parameters.clone())
                    .map_err(|e| format!("Failed to parse ambisonics decoder params: {e}"))?;
            let mut plugin = sotf_plugin_ambisonics::AmbisonicsDecoderPlugin::new(&config)?;
            plugin.initialize(sample_rate)?;
            Ok(Box::new(plugin))
        }

        #[cfg(not(feature = "iamf"))]
        "ambisonics_decoder" => {
            Err("Ambisonics decoder requires the 'iamf' feature".to_string())
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
            log::info!("[factory:matrix] Resizing square matrix from {in_ch}x{out_ch} to {channels}x{channels}");
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
            log::info!("[factory] Resizing channel_states from {} to {needed}", states.len());
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
