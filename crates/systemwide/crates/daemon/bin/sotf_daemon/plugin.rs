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
    pt.wire_name()
}

/// Categorize plugins for the UI picker
pub(super) fn plugin_type_category(pt: &PluginType) -> &'static str {
    match pt {
        PluginType::EQ | PluginType::FletcherMunson | PluginType::LoudnessCompensation => {
            "EQ & Tone"
        }
        PluginType::Gain | PluginType::Dither => "Utility",
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
        PluginType::Crossover
        | PluginType::BandSplit
        | PluginType::BandMerge
        | PluginType::Crossfeed => "Utility",
        PluginType::Delay => "Effects",
        PluginType::Aec => "Restoration",
        PluginType::Beamformer => "Spatial & Routing",
        PluginType::AmbisonicsDecoder => "Spatial & Routing",
        PluginType::StereoImager => "Spatial & Routing",
        PluginType::DeEsser => "Dynamics",
        PluginType::TransientShaper => "Dynamics",
        PluginType::Saturation => "Effects",
        PluginType::DynamicEq => "Dynamics",
        PluginType::LinearPhaseEq => "EQ",
        PluginType::SpectralCompressor => "Dynamics",
        PluginType::External => "External",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concrete_external_plugin_has_daemon_metadata_without_a_generic_picker_entry() {
        assert_eq!(plugin_type_to_engine_str(&PluginType::External), "external");
        assert_eq!(plugin_type_category(&PluginType::External), "External");
        assert!(!PluginType::all().contains(&PluginType::External));
    }

    #[test]
    fn every_picker_plugin_uses_the_engine_wire_name() {
        for plugin_type in PluginType::all() {
            assert_eq!(
                plugin_type_to_engine_str(&plugin_type),
                plugin_type.wire_name()
            );
        }
        assert_eq!(plugin_type_category(&PluginType::Dither), "Utility");
    }
}
