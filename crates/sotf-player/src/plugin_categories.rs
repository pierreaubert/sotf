//! Canonical plugin categorisation shared by all SOTF player UIs.
//!
//! Both the GPUI rack/detail panels and the TUI's plugin picker render
//! the available plugins grouped by category. Keeping the grouping in
//! one place avoids drift between frontends.

use sotf_audio::plugins::PluginType;

/// One category as displayed in the plugin picker.
pub struct PluginCategory {
    pub name: &'static str,
    pub plugins: &'static [PluginType],
}

/// Canonical category list. Order is preserved when rendered.
///
/// `FletcherMunson` is intentionally omitted: it is superseded by
/// `LoudnessCompensation`. Existing rack entries still load via the
/// backward-compat path in `sotf_plugins::factory`.
pub const CATEGORIES: &[PluginCategory] = &[
    PluginCategory {
        name: "Dynamics",
        plugins: &[
            PluginType::Compressor,
            PluginType::Limiter,
            PluginType::Gate,
            PluginType::Expander,
            PluginType::MultibandCompressor,
            PluginType::MultibandExpander,
            PluginType::TransientShaper,
            PluginType::DeEsser,
            PluginType::Saturation,
            PluginType::DynamicEq,
            PluginType::SpectralCompressor,
        ],
    },
    PluginCategory {
        name: "EQ & Tone",
        plugins: &[
            PluginType::EQ,
            PluginType::LinearPhaseEq,
            PluginType::Gain,
            PluginType::Delay,
            PluginType::LoudnessCompensation,
        ],
    },
    PluginCategory {
        name: "Denoising",
        plugins: &[
            PluginType::Denoiser,
            PluginType::Declick,
            PluginType::HissReducer,
            PluginType::SpeechDenoiser,
            PluginType::Aec,
            PluginType::Pnd,
        ],
    },
    PluginCategory {
        name: "Spatial",
        plugins: &[
            PluginType::Upmixer,
            PluginType::Matrix,
            PluginType::BinauralDecoder,
            PluginType::Convolution,
            PluginType::XTC,
            PluginType::Crossfeed,
            PluginType::Beamformer,
            PluginType::AmbisonicsDecoder,
            PluginType::AAE,
            PluginType::StereoImager,
            PluginType::Downmix,
            PluginType::MonoToStereo,
        ],
    },
    PluginCategory {
        name: "Analysis",
        plugins: &[
            PluginType::LoudnessMonitor,
            PluginType::SpectrumAnalyzer,
            PluginType::ABCompare,
        ],
    },
    PluginCategory {
        name: "Routing",
        plugins: &[
            PluginType::ChannelMuteSolo,
            PluginType::Crossover,
            PluginType::BandSplit,
            PluginType::BandMerge,
        ],
    },
];

/// Find the category name for a plugin, if it appears in `CATEGORIES`.
pub fn category_of(plugin_type: &PluginType) -> Option<&'static str> {
    CATEGORIES
        .iter()
        .find(|c| c.plugins.iter().any(|p| p == plugin_type))
        .map(|c| c.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_plugin_appears_in_two_categories() {
        let mut seen: Vec<&PluginType> = Vec::new();
        for cat in CATEGORIES {
            for p in cat.plugins {
                assert!(
                    !seen.contains(&p),
                    "{p:?} appears in more than one category"
                );
                seen.push(p);
            }
        }
    }

    #[test]
    fn all_listed_plugins_are_known() {
        // Just exercises the constants.
        for cat in CATEGORIES {
            assert!(!cat.name.is_empty());
            assert!(!cat.plugins.is_empty(), "{} is empty", cat.name);
        }
    }

    #[test]
    fn all_app_facing_plugins_appear_in_a_picker_category() {
        for plugin_type in PluginType::all() {
            if plugin_type == PluginType::FletcherMunson {
                continue;
            }
            assert!(
                category_of(&plugin_type).is_some(),
                "{plugin_type:?} is in PluginType::all() but missing from picker categories"
            );
        }
    }

    #[test]
    fn crossover_stays_reachable_from_routing_category() {
        assert_eq!(category_of(&PluginType::Crossover), Some("Routing"));
    }
}
