//! Plugin UI Components
//!
//! Each plugin type has its own rendering component to encapsulate
//! visualization and parameter display logic.
//!
//! Also includes logic modules for plugin-related App methods:
//! - `level_meters`: Level meter group management (mute/solo/dim)

mod binaural;
mod common;
mod compressor;
mod convolution;
mod eq;
mod gain;
mod gate;
mod level_meters;
mod limiter;
mod loudness;
mod mute_solo;
mod spectrum;
pub mod theme;
mod ticks;
mod upmixer;

pub use common::*;
pub use theme::*;
pub use ticks::{ScaleType, TickConfig, render_tick_row};
pub use compressor::render_compressor_plugin;
pub use eq::render_eq_plugin;
pub use gain::render_gain_plugin;
pub use gate::render_gate_plugin;
pub use limiter::render_limiter_plugin;
pub use loudness::{render_loudness_compensation_plugin, render_loudness_monitor_plugin};
pub use upmixer::render_upmixer_plugin;
pub use binaural::render_binaural_plugin;
pub use convolution::render_convolution_plugin;
pub use spectrum::render_spectrum_analyzer_plugin;
pub use mute_solo::render_mute_solo_plugin;

use crate::theme::Theme;
use gpui::*;
use sotf_audio_player::PluginSettings;

/// Render plugin-specific content based on plugin type
pub fn render_plugin_content(
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> AnyElement {
    match settings {
        PluginSettings::EQ { filters } => {
            render_eq_plugin(filters, is_editing, selected_param, theme).into_any_element()
        }
        PluginSettings::Gain { gain_db } => {
            render_gain_plugin(*gain_db, is_editing, selected_param, theme).into_any_element()
        }
        PluginSettings::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_gain_db,
            mix,
            auto_makeup,
            link_channels,
            sidechain_hpf_hz,
        } => {
            render_compressor_plugin(
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *knee_db,
                *makeup_gain_db,
                *mix,
                *auto_makeup,
                *link_channels,
                *sidechain_hpf_hz,
                is_editing,
                selected_param,
                theme,
            ).into_any_element()
        }
        PluginSettings::Limiter {
            threshold_db,
            release_ms,
            mix,
        } => {
            render_limiter_plugin(
                *threshold_db,
                *release_ms,
                *mix,
                is_editing,
                selected_param,
                theme,
            ).into_any_element()
        }
        PluginSettings::Gate {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
        } => {
            render_gate_plugin(
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *mix,
                *link_channels,
                *sidechain_hpf_hz,
                is_editing,
                selected_param,
                theme,
            ).into_any_element()
        }
        PluginSettings::Upmixer {
            speaker_config,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            lfe_cutoff_hz,
            stereo_width,
            bandpass_hz,
            height_gain,
            lfe_gain,
            enable_subharmonic_synth,
            subharmonic_gain,
            enable_hr_direct,
            hr_sharpen,
            safety_cap_db,
            decorrelation_mode,
        } => {
            render_upmixer_plugin(
                speaker_config,
                *gain_front_direct,
                *gain_front_ambient,
                *gain_rear_ambient,
                *lfe_cutoff_hz,
                *stereo_width,
                *bandpass_hz,
                *height_gain,
                *lfe_gain,
                *enable_subharmonic_synth,
                *subharmonic_gain,
                *enable_hr_direct,
                *hr_sharpen,
                *safety_cap_db,
                *decorrelation_mode,
                is_editing,
                selected_param,
                theme,
            ).into_any_element()
        }
        PluginSettings::LoudnessCompensation {
            target_lufs,
            min_gain_db,
            max_gain_db,
        } => {
            render_loudness_compensation_plugin(
                *target_lufs,
                *min_gain_db,
                *max_gain_db,
                is_editing,
                selected_param,
                theme,
            ).into_any_element()
        }
        PluginSettings::LoudnessMonitor => {
            render_loudness_monitor_plugin(is_editing, theme).into_any_element()
        }
        PluginSettings::BinauralDecoder {
            sofa_file,
            input_channels,
            enable_optimization,
            externalization,
            near_field_strength,
        } => {
            render_binaural_plugin(
                sofa_file,
                *input_channels,
                *enable_optimization,
                *externalization,
                *near_field_strength,
                is_editing,
                selected_param,
                theme,
            ).into_any_element()
        }
        PluginSettings::Convolution {
            ir_file,
            mix,
            gain_db,
        } => {
            render_convolution_plugin(
                ir_file,
                *mix,
                *gain_db,
                is_editing,
                selected_param,
                theme,
            ).into_any_element()
        }
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
        } => {
            render_spectrum_analyzer_plugin(
                *num_bins,
                *min_freq,
                *max_freq,
                *smoothing,
                is_editing,
                selected_param,
                theme,
            ).into_any_element()
        }
        PluginSettings::ChannelMuteSolo {
            enabled,
            channel_states,
        } => {
            render_mute_solo_plugin(
                *enabled,
                channel_states,
                is_editing,
                selected_param,
                theme,
            ).into_any_element()
        }
    }
}
