use super::misc::parameter_descriptor_to_json;
use serde_json::Value;
use sotf_audio::plugins::PluginType;

pub(super) fn plugin_parameter_descriptors(settings: &sotf_audio::PluginSettings) -> Vec<Value> {
    settings
        .param_specs()
        .iter()
        .map(parameter_descriptor_to_json)
        .collect()
}

/// Map PluginType enum to the string the engine's create_plugin() expects
pub(super) fn plugin_type_to_engine_str(pt: &PluginType) -> &'static str {
    match pt {
        PluginType::EQ => "eq",
        PluginType::Gain => "gain",
        PluginType::Upmixer => "upmixer",
        PluginType::AAE => "aae",
        PluginType::Compressor => "compressor",
        PluginType::Limiter => "limiter",
        PluginType::Gate => "gate",
        PluginType::Expander => "expander",
        PluginType::MultibandCompressor => "multiband_compressor",
        PluginType::MultibandExpander => "multiband_expander",
        PluginType::LoudnessCompensation => "loudness_compensation",
        PluginType::FletcherMunson => "fletcher_munson",
        PluginType::BinauralDecoder => "binaural_decoder",
        PluginType::Convolution => "convolution",
        PluginType::LoudnessMonitor => "loudness_monitor",
        PluginType::SpectrumAnalyzer => "spectrum_analyzer",
        PluginType::ChannelMuteSolo => "channel_mute_solo",
        PluginType::Matrix => "matrix",
        PluginType::XTC => "xtc",
        PluginType::Denoiser => "denoiser",
        PluginType::Declick => "declick",
        PluginType::HissReducer => "hiss_reducer",
        PluginType::SpeechDenoiser => "speech_denoiser",
        PluginType::Pnd => "pnd",
        PluginType::ABCompare => "ab_compare",
        PluginType::Crossover => "crossover",
        PluginType::BandSplit => "band_split",
        PluginType::BandMerge => "band_merge",
        PluginType::Downmix => "downmix",
        PluginType::MonoToStereo => "mono_to_stereo",
        PluginType::Crossfeed => "crossfeed",
        PluginType::Delay => "delay",
        PluginType::Aec => "aec",
        PluginType::Beamformer => "beamformer",
        PluginType::AmbisonicsDecoder => "ambisonics_decoder",
        PluginType::StereoImager => "stereo_imager",
        PluginType::DeEsser => "de_esser",
        PluginType::TransientShaper => "transient_shaper",
        PluginType::Saturation => "saturation",
        PluginType::DynamicEq => "dynamic_eq",
        PluginType::FirDesigner => "fir_designer",
        PluginType::LinearPhaseEq => "linear_phase_eq",
        PluginType::SpectralCompressor => "spectral_compressor",
    }
}

/// Categorize plugins for the UI picker
pub(super) fn plugin_type_category(pt: &PluginType) -> &'static str {
    match pt {
        PluginType::EQ | PluginType::FletcherMunson | PluginType::LoudnessCompensation => {
            "EQ & Tone"
        }
        PluginType::Gain => "Utility",
        PluginType::Compressor | PluginType::Limiter | PluginType::Gate | PluginType::Expander => {
            "Dynamics"
        }
        PluginType::MultibandCompressor | PluginType::MultibandExpander => "Dynamics",
        PluginType::AAE
        | PluginType::Upmixer
        | PluginType::Downmix
        | PluginType::MonoToStereo
        | PluginType::Matrix
        | PluginType::ChannelMuteSolo => "Spatial & Routing",
        PluginType::BinauralDecoder | PluginType::XTC => "Spatial & Routing",
        PluginType::Convolution => "Effects",
        PluginType::Denoiser
        | PluginType::Declick
        | PluginType::HissReducer
        | PluginType::SpeechDenoiser
        | PluginType::Pnd => "Restoration",
        PluginType::LoudnessMonitor | PluginType::SpectrumAnalyzer => "Monitoring",
        PluginType::ABCompare => "Utility",
        PluginType::Crossover | PluginType::BandSplit | PluginType::BandMerge | PluginType::Crossfeed => "Utility",
        PluginType::Delay => "Effects",
        PluginType::Aec => "Restoration",
        PluginType::Beamformer => "Spatial & Routing",
        PluginType::AmbisonicsDecoder => "Spatial & Routing",
        PluginType::StereoImager => "Spatial & Routing",
        PluginType::DeEsser => "Dynamics",
        PluginType::TransientShaper => "Dynamics",
        PluginType::Saturation => "Effects",
        PluginType::DynamicEq => "Dynamics",
        PluginType::FirDesigner => "EQ & Tone",
        PluginType::LinearPhaseEq => "EQ",
        PluginType::SpectralCompressor => "Dynamics",
    }
}
