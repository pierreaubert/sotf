//! Universal plugin factory for all SOTF audio plugins.

use sotf_host::plugin::{InPlacePluginAdapter, Plugin};
use sotf_host::ParametricPluginAdapter;

/// Create a plugin instance from a type name, channel count, sample rate, and JSON config.
///
/// The `sample_rate` is used for plugins that need it at construction time (EQ, XTC, Convolution).
/// Most plugins receive sample rate later via `initialize()`.
///
/// The `config_json` should be a JSON object matching the plugin's `Params` struct,
/// or `"{}"` / `"null"` for default parameters.
pub fn create_plugin(
    plugin_type: &str,
    channels: usize,
    sample_rate: u32,
    config_json: &str,
) -> Result<Box<dyn Plugin>, String> {
    match plugin_type {
        // ============================================================
        // Wave 1: Core effects
        // ============================================================
        "EQ" | "eq" => {
            let params: sotf_plugin_eq::EqPluginParams = parse_params(config_json)?;
            sotf_plugin_eq::EqPlugin::from_params(channels, sample_rate, params)
                .map(|p| p.into_boxed_plugin())
        }

        "Compressor" | "compressor" => {
            // Route to multiband compressor in single-band mode (num_bands=1).
            let mut params: sotf_plugin_multiband_compressor::MultibandCompressorPluginParams =
                parse_params(config_json)?;
            params.num_bands = 1;
            let plugin = sotf_plugin_multiband_compressor::MultibandCompressorPlugin::from_params(
                channels, params,
            );
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "Limiter" | "limiter" => {
            let params: sotf_plugin_limiter::LimiterPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_limiter::LimiterPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "Gate" | "gate" => {
            let params: sotf_plugin_gate::GatePluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_gate::GatePlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "Gain" | "gain" => {
            let params: sotf_plugin_gain::GainPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_gain::GainPlugin::from_params(channels, params)?;
            Ok(Box::new(ParametricPluginAdapter::new(plugin)))
        }

        "Delay" | "delay" => {
            let params: sotf_plugin_delay::DelayPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_delay::DelayPlugin::from_params(channels, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        // ============================================================
        // Wave 2: Extended effects
        // ============================================================
        "Expander" | "expander" => {
            // Route to multiband expander in single-band mode (num_bands=1).
            let mut params: sotf_plugin_multiband_expander::MultibandExpanderPluginParams =
                parse_params(config_json)?;
            params.num_bands = 1;
            let plugin = sotf_plugin_multiband_expander::MultibandExpanderPlugin::from_params(
                channels, params,
            );
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "DeEsser" | "de_esser" => {
            let params: sotf_plugin_de_esser::DeEsserPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_de_esser::DeEsserPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "DynamicEQ" | "dynamic_eq" => {
            let params: sotf_plugin_dynamic_eq::DynamicEqPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_dynamic_eq::DynamicEqPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "Crossfeed" | "crossfeed" => {
            let params: sotf_plugin_crossfeed::CrossfeedPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_crossfeed::CrossfeedPlugin::new(params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "MultibandCompressor" | "multiband_compressor" => {
            let params: sotf_plugin_multiband_compressor::MultibandCompressorPluginParams =
                parse_params(config_json)?;
            let plugin = sotf_plugin_multiband_compressor::MultibandCompressorPlugin::from_params(
                channels, params,
            );
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "MultibandExpander" | "multiband_expander" => {
            let params: sotf_plugin_multiband_expander::MultibandExpanderPluginParams =
                parse_params(config_json)?;
            let plugin = sotf_plugin_multiband_expander::MultibandExpanderPlugin::from_params(
                channels, params,
            );
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "Convolution" | "convolution" => {
            let params: sotf_plugin_convolution::ConvolutionPluginParams =
                parse_params(config_json)?;
            let plugin = sotf_plugin_convolution::ConvolutionPlugin::from_params(
                channels,
                sample_rate,
                params,
            )?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "FletcherMunson" | "fletcher_munson" => {
            // Backward compat: route to LoudnessCompensation in Auto mode (mode=2)
            let fm_params: sotf_plugin_loudness_compensation::FletcherMunsonCompat =
                parse_params(config_json)?;
            let lc_params = fm_params.into_loudness_compensation_params();
            let plugin =
                sotf_plugin_loudness_compensation::LoudnessCompensationPlugin::from_params(
                    channels, lc_params,
                )?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "LoudnessCompensation" | "loudness_compensation" => {
            let params: sotf_plugin_loudness_compensation::LoudnessCompensationPluginParams =
                parse_params(config_json)?;
            let plugin =
                sotf_plugin_loudness_compensation::LoudnessCompensationPlugin::from_params(
                    channels, params,
                )?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "ChannelMuteSolo" | "channel_mute_solo" => {
            let params: sotf_plugin_channel_mute_solo::ChannelMuteSoloParams =
                parse_params(config_json)?;
            let plugin =
                sotf_plugin_channel_mute_solo::ChannelMuteSoloPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "Downmix" | "downmix" => {
            let params: sotf_plugin_downmix::DownmixPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_downmix::DownmixPlugin::from_params(params);
            Ok(Box::new(plugin))
        }

        // ============================================================
        // Wave 3: Spatial / specialized
        // ============================================================
        "Upmixer" | "upmixer" => {
            let params: sotf_plugin_upmixer::UpmixerPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_upmixer::UpmixerPlugin::from_params(params);
            Ok(Box::new(plugin))
        }

        "AAE" | "aae" | "active_acoustic_enhancement" => {
            let params: sotf_plugin_aae::params::AaePluginParams = parse_params(config_json)?;
            let mut plugin = sotf_plugin_aae::AaePlugin::from_params(params);
            plugin.initialize(sample_rate)?;
            Ok(Box::new(plugin))
        }

        "XTC" | "xtc" => {
            let params: sotf_plugin_xtc::XtcPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_xtc::XtcPlugin::from_params(params, sample_rate)?;
            Ok(Box::new(plugin))
        }

        "Binaural" | "binaural" => {
            let params: sotf_plugin_binaural::BinauralDecoderParams = parse_params(config_json)?;
            let plugin = sotf_plugin_binaural::BinauralDecoderPlugin::from_params(params);
            Ok(Box::new(plugin))
        }

        "Matrix" | "matrix" => {
            let plugin = sotf_plugin_matrix::MatrixPlugin::new(channels, channels);
            Ok(Box::new(plugin))
        }

        "MonoToStereo" | "mono_to_stereo" => {
            let params: sotf_plugin_mono_to_stereo::MonoToStereoPluginParams =
                parse_params(config_json)?;
            let plugin =
                sotf_plugin_mono_to_stereo::MonoToStereoPlugin::from_params(channels, params);
            Ok(Box::new(plugin))
        }

        "PND" | "pnd" => {
            let params: sotf_plugin_pnd::PndPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_pnd::PndPlugin::from_params(channels, params);
            Ok(Box::new(plugin))
        }

        "Denoiser" | "denoiser" => {
            let params: sotf_plugin_denoiser::DenoiserPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_denoiser::DenoiserPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "SpeechDenoiser" | "speech_denoiser" | "RNNoise" | "rnnoise" => {
            let params: sotf_plugin_speech_denoiser::SpeechDenoiserPluginParams =
                parse_params(config_json)?;
            let plugin =
                sotf_plugin_speech_denoiser::SpeechDenoiserPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "HissReducer" | "hiss_reducer" | "Hiss" | "hiss" => {
            let params: sotf_plugin_hiss_reducer::HissReducerPluginParams =
                parse_params(config_json)?;
            let plugin = sotf_plugin_hiss_reducer::HissReducerPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "Declick" | "declick" | "TransientRepair" | "transient_repair" => {
            let params: sotf_plugin_declick::DeclickPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_declick::DeclickPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "ABCompare" | "ab_compare" => {
            let params: sotf_plugin_ab_compare::ABComparePluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_ab_compare::ABComparePlugin::from_params(channels, params)?;
            Ok(Box::new(plugin))
        }

        "Crossover" | "crossover" => {
            let params: sotf_plugin_crossover::CrossoverPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_crossover::CrossoverPlugin::from_params(channels, &params)?;
            Ok(Box::new(plugin))
        }

        "StereoImager" | "stereo_imager" => {
            let params: sotf_plugin_stereo_imager::StereoImagerPluginParams =
                parse_params(config_json)?;
            let plugin =
                sotf_plugin_stereo_imager::StereoImagerPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "TransientShaper" | "transient_shaper" => {
            let params: sotf_plugin_transient_shaper::TransientShaperPluginParams =
                parse_params(config_json)?;
            let plugin =
                sotf_plugin_transient_shaper::TransientShaperPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "Saturation" | "saturation" => {
            let params: sotf_plugin_saturation::SaturationPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_saturation::SaturationPlugin::from_params(channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "LinearPhaseEQ" | "linear_phase_eq" => {
            let params: sotf_plugin_linear_phase_eq::LinearPhaseEqPluginParams =
                parse_params(config_json)?;
            let plugin = sotf_plugin_linear_phase_eq::LinearPhaseEqPlugin::from_params(
                channels,
                sample_rate,
                params,
            )?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "SpectralCompressor" | "spectral_compressor" => {
            let params: sotf_plugin_spectral_compressor::SpectralCompressorPluginParams =
                parse_params(config_json)?;
            let plugin = sotf_plugin_spectral_compressor::SpectralCompressorPlugin::from_params(
                channels, params,
            );
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "Dither" | "dither" => {
            let plugin = sotf_plugin_dither::DitherPlugin::new(channels);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }

        "AmbisonicsDecoder" | "ambisonics_decoder" => {
            let config: sotf_plugin_ambisonics::AmbisonicsDecoderConfig =
                parse_params(config_json)?;
            let mut plugin = sotf_plugin_ambisonics::AmbisonicsDecoderPlugin::new(&config)?;
            plugin.initialize(sample_rate)?;
            Ok(Box::new(plugin))
        }

        "BandSplit" | "band_split" => {
            let params: sotf_plugin_band_split::BandSplitPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_band_split::BandSplitPlugin::from_params(channels, &params)?;
            Ok(Box::new(plugin))
        }

        "BandMerge" | "band_merge" => {
            let params: sotf_plugin_band_merge::BandMergePluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_band_merge::BandMergePlugin::from_params(channels, &params)?;
            Ok(Box::new(plugin))
        }

        "AEC" | "aec" => {
            let params: sotf_plugin_aec::AecPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_aec::AecPlugin::from_params(sample_rate, params);
            Ok(Box::new(plugin))
        }

        "Beamformer" | "beamformer" => {
            let params: sotf_plugin_beamformer::BeamformerPluginParams = parse_params(config_json)?;
            let plugin = sotf_plugin_beamformer::BeamformerPlugin::from_params(sample_rate, params);
            Ok(Box::new(plugin))
        }

        _ => Err(format!("Unknown plugin type: {plugin_type}")),
    }
}

/// List all available plugin types.
pub fn available_plugin_types() -> &'static [&'static str] {
    &[
        "EQ",
        "Compressor",
        "Limiter",
        "Gate",
        "Gain",
        "Delay",
        "Expander",
        "Crossfeed",
        "MultibandCompressor",
        "MultibandExpander",
        "Convolution",
        "FletcherMunson",
        "LoudnessCompensation",
        "ChannelMuteSolo",
        "Downmix",
        "Upmixer",
        "AAE",
        "XTC",
        "Binaural",
        "Matrix",
        "MonoToStereo",
        "PND",
        "Denoiser",
        "SpeechDenoiser",
        "HissReducer",
        "Declick",
        "ABCompare",
        "Crossover",
        "StereoImager",
        "TransientShaper",
        "DynamicEQ",
        "Saturation",
        "LinearPhaseEQ",
        "SpectralCompressor",
        "Dither",
        "AmbisonicsDecoder",
        "BandSplit",
        "BandMerge",
        "AEC",
        "Beamformer",
    ]
}

fn parse_params<T: serde::de::DeserializeOwned>(config_json: &str) -> Result<T, String> {
    let trimmed = config_json.trim();
    if trimmed.is_empty() || trimmed == "null" || trimmed == "{}" {
        // Try to deserialize from empty object for types with serde defaults
        serde_json::from_str::<T>("{}").map_err(|e| format!("Default params failed: {e}"))
    } else {
        serde_json::from_str::<T>(config_json)
            .map_err(|e| format!("Failed to parse plugin config: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::plugin::ProcessContext;

    #[test]
    fn test_create_wave1_plugins() {
        let wave1 = ["EQ", "Compressor", "Limiter", "Gate", "Gain", "Delay"];
        for plugin_type in &wave1 {
            let result = create_plugin(plugin_type, 2, 48000, "{}");
            assert!(
                result.is_ok(),
                "Failed to create {plugin_type}: {:?}",
                result.err()
            );
            let mut plugin = result.unwrap();
            assert!(plugin.initialize(48000).is_ok());
        }
    }

    #[test]
    fn test_create_wave1_lowercase() {
        let wave1 = ["eq", "compressor", "limiter", "gate", "gain", "delay"];
        for plugin_type in &wave1 {
            let result = create_plugin(plugin_type, 2, 48000, "{}");
            assert!(
                result.is_ok(),
                "Failed to create {plugin_type}: {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn test_unknown_plugin() {
        let result = create_plugin("NonExistent", 2, 48000, "{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_newly_wired_plugins() {
        for plugin_type in ["BandSplit", "BandMerge", "AEC", "Beamformer"] {
            let result = create_plugin(plugin_type, 2, 48000, "{}");
            assert!(
                result.is_ok(),
                "Failed to create {plugin_type}: {:?}",
                result.err()
            );
            let mut plugin = result.unwrap();
            assert!(plugin.initialize(48000).is_ok());
        }
    }

    #[test]
    fn test_eq_with_config() {
        let config = r#"{
            "filters": [
                {"filter_type": "peak", "freq": 1000.0, "q": 1.5, "db_gain": 3.0}
            ]
        }"#;
        let result = create_plugin("EQ", 2, 48000, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_silence() {
        let mut plugin = create_plugin("Gain", 2, 48000, "{}").unwrap();
        plugin.initialize(48000).unwrap();

        let input = vec![0.0f32; 256];
        let mut output = vec![0.0f32; 256];
        let ctx = ProcessContext::new(48000, 128);
        let result = plugin.process(&input, &mut output, &ctx);
        assert!(result.is_ok());
    }
}
