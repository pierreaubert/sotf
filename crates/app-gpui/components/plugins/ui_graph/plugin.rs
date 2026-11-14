use super::consts::MAX_WORKFLOW_PORTS;
use super::consts::NODE_MENU_BYPASS;
use super::consts::NODE_MENU_EDIT;
use super::consts::NODE_MENU_REMOVE;
use super::consts::NODE_MENU_SOLO;
use crate::i18n::PluginGraphTranslations;
use crate::theme::Theme;
use gpui::Rgba;
use gpui_ui_kit::MenuItem;
use sotf_audio_player::PluginType;

/// Get a color for a plugin type
pub(super) fn plugin_color(plugin_type: &PluginType, theme: &Theme) -> Rgba {
    super::super::ui_plugin_shell::plugin_accent_color(plugin_type, theme)
}

/// Get input and output channel counts for a plugin type
/// Maximum port counts for a plugin type (None = fixed, matches default counts)
pub(super) fn plugin_max_ports(plugin_type: &PluginType) -> (Option<usize>, Option<usize>) {
    match plugin_type {
        PluginType::Upmixer | PluginType::AAE => (Some(2), Some(MAX_WORKFLOW_PORTS)),
        PluginType::BinauralDecoder | PluginType::Downmix => (Some(MAX_WORKFLOW_PORTS), Some(2)),
        PluginType::MonoToStereo => (Some(1), Some(2)),
        PluginType::Crossover
        | PluginType::BandSplit
        | PluginType::BandMerge
        | PluginType::Matrix => (Some(MAX_WORKFLOW_PORTS), Some(MAX_WORKFLOW_PORTS)),
        PluginType::AmbisonicsDecoder => (Some(4), Some(MAX_WORKFLOW_PORTS)),
        PluginType::XTC
        | PluginType::Crossfeed
        | PluginType::StereoImager
        | PluginType::Aec
        | PluginType::Beamformer => {
            let (i, o) = plugin_channel_counts(plugin_type);
            (Some(i), Some(o))
        }
        _ => {
            // Most processors and analyzers are pass-through with respect to
            // channel layout. Let graph bulk-connect grow both sides after an
            // upmixer or matrix expands the chain.
            (Some(MAX_WORKFLOW_PORTS), Some(MAX_WORKFLOW_PORTS))
        }
    }
}

pub(super) fn plugin_channel_counts(plugin_type: &PluginType) -> (usize, usize) {
    match plugin_type {
        // Most plugins are 2-in, 2-out (stereo passthrough)
        // Upmixer/AAE: stereo in, multi-channel out
        PluginType::Upmixer | PluginType::AAE => (2, 5),
        // Binaural decoder: multi-channel in, stereo out
        PluginType::BinauralDecoder => (5, 2),
        // Convolution: stereo in/out
        // Matrix: variable in/out (default to 2x2)
        // A/B Compare: stereo in/out
        // Crossover: stereo in, stereo/frequency-band out depending on mode.
        PluginType::Crossover | PluginType::BandSplit => (2, 4),
        // Band Split: 2 in, 4 out (2 bands x 2 channels)
        // Band Merge: 4 in, 2 out (2 bands x 2 channels merged back)
        PluginType::BandMerge => (4, 2),
        // Downmix: multi-channel in, stereo out
        PluginType::Downmix => (6, 2),
        // Mono to Stereo: mono in, stereo out
        PluginType::MonoToStereo => (1, 2),
        // Crossfeed: stereo in/out
        // AEC: 2 in (mic + ref), 1 out
        PluginType::Aec | PluginType::Beamformer => (2, 1),
        // Beamformer: M in, 1 out (default 2 mics)
        // Ambisonics decoder: 4ch (FOA) in, multi-channel out
        PluginType::AmbisonicsDecoder => (4, 6),
        // Stereo imager: stereo only
        // De-esser: stereo in/out
        // Transient shaper: in-place processing
        // Saturation: in-place processing
        // Dynamic EQ: in-place processing
        // FIR EQ: in-place processing
        // FIR Designer: in-place processing
        // Spectral compressor: in-place processing
        // Concrete external graph nodes carry descriptor-specific counts in
        // PluginGraphNode; this fallback is only used by type-only palettes.
        PluginType::EQ
        | PluginType::Gain
        | PluginType::Dither
        | PluginType::Compressor
        | PluginType::Limiter
        | PluginType::Gate
        | PluginType::Expander
        | PluginType::MultibandCompressor
        | PluginType::MultibandExpander
        | PluginType::LoudnessCompensation
        | PluginType::FletcherMunson
        | PluginType::LoudnessMonitor
        | PluginType::SpectrumAnalyzer
        | PluginType::ChannelMuteSolo
        | PluginType::XTC
        | PluginType::Denoiser
        | PluginType::Declick
        | PluginType::HissReducer
        | PluginType::SpeechDenoiser
        | PluginType::Pnd
        | PluginType::AnalogEq
        | PluginType::AnalogLimiter
        | PluginType::AnalogCompressor
        | PluginType::Convolution
        | PluginType::Matrix
        | PluginType::ABCompare
        | PluginType::Crossfeed
        | PluginType::Delay
        | PluginType::StereoImager
        | PluginType::DeEsser
        | PluginType::TransientShaper
        | PluginType::Saturation
        | PluginType::DynamicEq
        | PluginType::LinearPhaseEq
        | PluginType::SpectralCompressor
        | PluginType::External => (2, 2),
    }
}

#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
pub(super) fn plugin_node_menu_items(text: PluginGraphTranslations) -> Vec<MenuItem> {
    vec![
        MenuItem::new(NODE_MENU_EDIT, text.edit_parameters),
        MenuItem::separator(),
        MenuItem::new(NODE_MENU_BYPASS, text.bypass_activate),
        MenuItem::new(NODE_MENU_SOLO, text.solo),
        MenuItem::separator(),
        MenuItem::new(NODE_MENU_REMOVE, text.remove),
    ]
}
