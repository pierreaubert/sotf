//! Plugin UI Components
//!
//! Each plugin type has its own rendering component to encapsulate
//! visualization and parameter display logic.
//!
//! Also includes logic modules for plugin-related App methods:
//! - `level_meters`: Level meter group management (mute/solo/dim)
//! - `graph`: 2D canvas-based plugin graph view

pub mod actions;
pub mod common;
pub mod editing;
pub mod level_meters;
pub mod theme;
pub mod ticks;

mod ui_binaural;
mod ui_compressor;
mod ui_convolution;
mod ui_eq;
mod ui_gain;
mod ui_gate;
mod ui_graph;
mod ui_limiter;
mod ui_loudness;
mod ui_mute_solo;
mod ui_rack;
mod ui_spectrum;
mod ui_upmixer;

pub use common::*;
pub use editing::get_param_count;
pub use level_meters::{
    LevelMeterElement, MeterColors, db_to_position, render_gr_meter, render_gradient_meter,
    render_lufs_with_true_peak, render_peak_meter,
};
pub use theme::*;
pub use ticks::{ScaleType, TickConfig, render_tick_row};

pub use ui_binaural::render_binaural_plugin;
pub use ui_compressor::render_compressor_plugin;
pub use ui_convolution::render_convolution_plugin;
pub use ui_eq::render_eq_plugin;
pub use ui_gain::render_gain_plugin;
pub use ui_gate::render_gate_plugin;
pub use ui_limiter::render_limiter_plugin;
pub use ui_loudness::{render_loudness_compensation_plugin, render_loudness_monitor_plugin};
pub use ui_mute_solo::render_mute_solo_plugin;
pub use ui_rack::PluginDragInfo;
pub use ui_spectrum::{
    MeterData, SpectrumColors, SpectrumElement, render_spectrum_analyzer_plugin,
};
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
    selected_band_idx: usize,
    loudness: Option<sotf_audio_player::LoudnessData>,
    plugin_data: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
) -> AnyElement {
    match settings {
        PluginSettings::EQ { filters } => {
            let selected_band_idx = selected_band_idx.min(filters.len().saturating_sub(1));
            render_eq_plugin(
                entity.clone(),
                plugin_idx,
                ui_eq::EqRenderState {
                    filters,
                    is_editing,
                    selected_param,
                    selected_band_idx,
                },
                theme,
            )
            .into_any_element()
        }
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
                data: plugin_data.as_ref().and_then(|d| d.downcast_ref()),
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
                data: plugin_data.as_ref().and_then(|d| d.downcast_ref()),
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Upmixer {
            speaker_config,
            // Gains
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            height_gain,
            stereo_width,
            center_spread,
            surround_direct_bleed,
            rear_late_reflection,
            // LFE
            lfe_cutoff_hz,
            lfe_gain,
            bandpass_hz,
            // Sub-harmonic
            enable_subharmonic_synth,
            subharmonic_gain,
            subharmonic_freq_hz,
            subharmonic_attack_ms,
            subharmonic_release_ms,
            // Decorrelation
            decorrelation_mode,
            decorrelation_lfo_rate_hz,
            velvet_noise_duration_ms,
            velvet_noise_density,
            // Height
            enable_hr_direct,
            hr_sharpen,
            height_hf_cap_hz,
            height_transient_reduction,
            height_direct_leak,
            // Ambient
            ambient_boost,
            safety_cap_db,
            rear_ambient_boost,
            // Dialogue
            dialogue_weight,
            voice_freq_min_hz,
            voice_freq_max_hz,
            ..
        } => render_upmixer_plugin(
            entity,
            plugin_idx,
            ui_upmixer::UpmixerRenderState {
                speaker_config,
                // Gains
                gain_front_direct: *gain_front_direct,
                gain_front_ambient: *gain_front_ambient,
                gain_rear_ambient: *gain_rear_ambient,
                height_gain: *height_gain,
                stereo_width: *stereo_width,
                center_spread: *center_spread,
                surround_direct_bleed: *surround_direct_bleed,
                rear_late_reflection: *rear_late_reflection,
                // LFE
                lfe_cutoff_hz: *lfe_cutoff_hz,
                lfe_gain: *lfe_gain,
                bandpass_hz: *bandpass_hz,
                // Sub-harmonic
                enable_subharmonic_synth: *enable_subharmonic_synth,
                subharmonic_gain: *subharmonic_gain,
                subharmonic_freq_hz: *subharmonic_freq_hz,
                subharmonic_attack_ms: *subharmonic_attack_ms,
                subharmonic_release_ms: *subharmonic_release_ms,
                // Decorrelation
                decorrelation_mode: *decorrelation_mode,
                decorrelation_lfo_rate_hz: *decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms: *velvet_noise_duration_ms,
                velvet_noise_density: *velvet_noise_density,
                // Height
                enable_hr_direct: *enable_hr_direct,
                hr_sharpen: *hr_sharpen,
                height_hf_cap_hz: *height_hf_cap_hz,
                height_transient_reduction: *height_transient_reduction,
                height_direct_leak: *height_direct_leak,
                // Ambient
                ambient_boost: *ambient_boost,
                safety_cap_db: *safety_cap_db,
                rear_ambient_boost: *rear_ambient_boost,
                // Dialogue
                dialogue_weight: *dialogue_weight,
                voice_freq_min_hz: *voice_freq_min_hz,
                voice_freq_max_hz: *voice_freq_max_hz,
                // UI state
                is_editing,
                selected_param,
                config_open,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::LoudnessCompensation {
            low_freq,
            low_gain,
            high_freq,
            high_gain,
        } => render_loudness_compensation_plugin(
            entity.clone(),
            plugin_idx,
            ui_loudness::LoudnessCompensationRenderState {
                low_freq: *low_freq,
                low_gain: *low_gain,
                high_freq: *high_freq,
                high_gain: *high_gain,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::LoudnessMonitor => {
            render_loudness_monitor_plugin(loudness, plugin_idx, is_editing, theme)
                .into_any_element()
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
            ui_binaural::BinauralRenderState {
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
                data: plugin_data.as_ref().and_then(|d| d.downcast_ref()),
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
        PluginSettings::Matrix {
            input_channels,
            output_channels,
            matrix,
        } => {
            // TODO: Implement dedicated Matrix UI
            // For now, render a placeholder
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_primary)
                        .child(format!("Matrix: {}→{} channels", input_channels, output_channels)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(format!("{} coefficients", matrix.len())),
                )
                .into_any_element()
        }
    }
}
