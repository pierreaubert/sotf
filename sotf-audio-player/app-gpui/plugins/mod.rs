//! Plugin UI Components
//!
//! Each plugin type has its own rendering component to encapsulate
//! visualization and parameter display logic.
//!
//! Also includes logic modules for plugin-related App methods:
//! - `level_meters`: Level meter group management (mute/solo/dim)

pub mod actions;
pub mod common;
pub mod ticks;
pub mod theme;
pub mod level_meters;
pub mod editing;

mod binaural;
mod ui_compressor;
mod ui_convolution;
mod ui_eq;
mod ui_gain;
mod ui_gate;
mod ui_limiter;
mod ui_loudness;
mod ui_mute_solo;
mod ui_rack;
mod ui_spectrum;
mod ui_upmixer;

pub use common::*;
pub use level_meters::{LevelMeterElement, MeterColors, db_to_position, render_gradient_meter, render_gr_meter, render_peak_meter};
pub use editing::get_param_count;
pub use theme::*;
pub use ticks::{ScaleType, TickConfig, render_tick_row};

pub use binaural::render_binaural_plugin;
pub use ui_compressor::render_compressor_plugin;
pub use ui_convolution::render_convolution_plugin;
pub use ui_eq::render_eq_plugin;
pub use ui_gain::render_gain_plugin;
pub use ui_gate::render_gate_plugin;
pub use ui_limiter::render_limiter_plugin;
pub use ui_loudness::{render_loudness_compensation_plugin, render_loudness_monitor_plugin};
pub use ui_mute_solo::render_mute_solo_plugin;
pub use ui_rack::PluginDragInfo;
pub use ui_spectrum::{MeterData, SpectrumColors, SpectrumElement, render_spectrum_analyzer_plugin};
pub use ui_upmixer::render_upmixer_plugin;

use crate::app::AppState;
use crate::theme::Theme;
use gpui::*;
use sotf_audio_player::PluginSettings;

/// Render plugin-specific content based on plugin type
/// Uses Entity<AppState> for direct state updates
pub fn render_plugin_content(
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
    config_open: bool,
) -> AnyElement {
    match settings {
        PluginSettings::EQ { filters } => render_eq_plugin(
            entity.clone(),
            plugin_idx,
            ui_eq::EqRenderState {
                filters,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Gain { gain_db } => render_gain_plugin(
            entity.clone(),
            plugin_idx,
            ui_gain::GainRenderState {
                gain_db: *gain_db,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
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
        } => render_compressor_plugin(
            entity.clone(),
            plugin_idx,
            ui_compressor::CompressorRenderState {
                threshold_db: *threshold_db,
                ratio: *ratio,
                attack_ms: *attack_ms,
                release_ms: *release_ms,
                knee_db: *knee_db,
                makeup_gain_db: *makeup_gain_db,
                mix: *mix,
                auto_makeup: *auto_makeup,
                link_channels: *link_channels,
                sidechain_hpf_hz: *sidechain_hpf_hz,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Limiter {
            threshold_db,
            release_ms,
            mix,
        } => render_limiter_plugin(
            entity.clone(),
            plugin_idx,
            ui_limiter::LimiterRenderState {
                threshold_db: *threshold_db,
                release_ms: *release_ms,
                mix: *mix,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Gate {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
        } => render_gate_plugin(
            entity.clone(),
            plugin_idx,
            ui_gate::GateRenderState {
                threshold_db: *threshold_db,
                ratio: *ratio,
                attack_ms: *attack_ms,
                release_ms: *release_ms,
                mix: *mix,
                link_channels: *link_channels,
                sidechain_hpf_hz: *sidechain_hpf_hz,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
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
        } => render_upmixer_plugin(
            entity,
            plugin_idx,
            ui_upmixer::UpmixerRenderState {
                speaker_config,
                gain_front_direct: *gain_front_direct,
                gain_front_ambient: *gain_front_ambient,
                gain_rear_ambient: *gain_rear_ambient,
                lfe_cutoff_hz: *lfe_cutoff_hz,
                stereo_width: *stereo_width,
                bandpass_hz: *bandpass_hz,
                height_gain: *height_gain,
                lfe_gain: *lfe_gain,
                enable_subharmonic_synth: *enable_subharmonic_synth,
                subharmonic_gain: *subharmonic_gain,
                enable_hr_direct: *enable_hr_direct,
                hr_sharpen: *hr_sharpen,
                safety_cap_db: *safety_cap_db,
                decorrelation_mode: *decorrelation_mode,
                is_editing,
                selected_param,
                config_open,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::LoudnessCompensation {
            target_lufs,
            min_gain_db,
            max_gain_db,
        } => render_loudness_compensation_plugin(
            entity.clone(),
            plugin_idx,
            ui_loudness::LoudnessCompensationRenderState {
                target_lufs: *target_lufs,
                min_gain_db: *min_gain_db,
                max_gain_db: *max_gain_db,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::LoudnessMonitor => {
            render_loudness_monitor_plugin(entity.clone(), plugin_idx, is_editing, theme).into_any_element()
        }
        PluginSettings::BinauralDecoder {
            sofa_file,
            input_channels,
            enable_optimization,
            externalization,
            near_field_strength,
        } => render_binaural_plugin(
            entity.clone(),
            plugin_idx,
            binaural::BinauralRenderState {
                sofa_file,
                input_channels: *input_channels,
                enable_optimization: *enable_optimization,
                externalization: *externalization,
                near_field_strength: *near_field_strength,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Convolution {
            ir_file,
            mix,
            gain_db,
        } => render_convolution_plugin(
            entity.clone(),
            plugin_idx,
            ui_convolution::ConvolutionRenderState {
                ir_file,
                mix: *mix,
                gain_db: *gain_db,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
        } => render_spectrum_analyzer_plugin(
            entity.clone(),
            plugin_idx,
            ui_spectrum::SpectrumRenderState {
                num_bins: *num_bins,
                min_freq: *min_freq,
                max_freq: *max_freq,
                smoothing: *smoothing,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::ChannelMuteSolo {
            enabled,
            channel_states,
        } => render_mute_solo_plugin(
            entity.clone(),
            plugin_idx,
            ui_mute_solo::ChannelMuteSoloRenderState {
                enabled: *enabled,
                channel_states,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
    }
}
