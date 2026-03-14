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

mod ui_ab_compare;
pub mod ui_auto_layout;
pub mod ui_layout_renderer;
mod ui_band_merge;
mod ui_band_split;
mod ui_binaural;
mod ui_compressor;
mod ui_convolution;
mod ui_crossfeed;
mod ui_denoiser;
mod ui_downmix;
pub mod ui_eq;
mod ui_expander;
mod ui_fletcher_munson;
mod ui_gain;
mod ui_gate;
mod ui_graph;
mod ui_limiter;
mod ui_loudness;
mod ui_matrix;
mod ui_mb_compressor;
mod ui_mb_expander;
mod ui_mono_to_stereo;
mod ui_mute_solo;
pub mod ui_plugin_shell;
mod ui_pnd;
mod ui_rack;
mod ui_simple;
mod ui_spectrum;
mod ui_upmixer;
mod ui_xtc;

pub use common::*;
pub use editing::get_param_count;
pub use level_meters::{
    LevelMeterElement, MeterColors, db_to_position, render_gr_meter, render_gradient_meter,
    render_lufs_with_true_peak, render_peak_meter,
};
pub use sotf_audio_player_midi::mapping::MidiOverlay;
pub use theme::*;
pub use ticks::{ScaleType, TickConfig, render_tick_row};

pub use ui_ab_compare::render_ab_compare_plugin;
pub use ui_auto_layout::{AutoLayoutInput, render_auto_layout, render_plugin_auto};
pub use ui_band_merge::render_band_merge_plugin;
pub use ui_band_split::render_band_split_plugin;
pub use ui_binaural::render_binaural_plugin;
pub use ui_compressor::render_compressor_plugin;
pub use ui_convolution::render_convolution_plugin;
pub use ui_crossfeed::render_crossfeed_plugin;
pub use ui_denoiser::render_denoiser_plugin;
pub use ui_downmix::render_downmix_plugin;
pub use ui_eq::render_eq_plugin;
pub use ui_expander::render_expander_plugin;
pub use ui_fletcher_munson::render_fletcher_munson_plugin;
pub use ui_gain::render_gain_plugin;
pub use ui_gate::render_gate_plugin;
pub use ui_limiter::render_limiter_plugin;
pub use ui_loudness::{render_loudness_compensation_plugin, render_loudness_monitor_plugin};
pub use ui_matrix::render_matrix_plugin;
pub use ui_mb_compressor::render_mb_compressor_plugin;
pub use ui_mb_expander::render_mb_expander_plugin;
pub use ui_mono_to_stereo::render_mono_to_stereo_plugin;
pub use ui_mute_solo::render_mute_solo_plugin;
pub use ui_plugin_shell::render_plugin_shell;
pub use ui_pnd::render_pnd_plugin;
pub use ui_rack::PluginDragInfo;
pub use ui_simple::render_simple_plugin_view;
pub use ui_spectrum::{
    MeterData, SpectrumColors, SpectrumElement, render_spectrum_analyzer_plugin,
};
pub use ui_upmixer::render_upmixer_plugin;
pub use ui_xtc::render_xtc_plugin;

use crate::app::AppState;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::*;
use sotf_audio_player::{PluginChain, PluginSettings};

/// Render plugin-specific content based on plugin type.
///
/// Uses the declarative `PluginLayout` renderer for most plugins, with
/// custom renderers only for EQ, SpectrumAnalyzer, Matrix, ChannelMuteSolo,
/// MultibandCompressor, MultibandExpander, and LoudnessMonitor.
pub fn render_plugin_content(
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
    _config_open: bool,
    selected_band_idx: usize,
    loudness: Option<sotf_audio_player::LoudnessData>,
    plugin_data: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    spectrum_tilt_select_open: bool,
    spectrum_reference_select_open: bool,
    plugin_chain: &PluginChain,
    midi_overlay: Option<&MidiOverlay>,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let state = entity.read(cx);
    let auto_tab = state
        .app
        .plugin_auto_tab
        .get(&plugin_idx)
        .copied()
        .unwrap_or(0);

    // Compute available width for the plugin content area from window/layout state.
    // Rack panel width = rack_h_ratio * window_width (horizontal mode).
    // Plugin content = rack_width - output_meter_width - padding/dividers.
    let available_width = {
        let window_width = state.app.ui_state.window_width;
        let layout_state = state.layout.read(cx);
        let rack_ratio = if layout_state.rack_panel_collapsed {
            0.0
        } else {
            layout_state.rack_h_ratio
        };
        let rack_width = rack_ratio * window_width;
        let output_meter_width = if state.app.output_meter_collapsed {
            0.0
        } else {
            state.app.output_meter_width
        };
        // Subtract output meter, dividers (~12px), and padding (~32px)
        (rack_width - output_meter_width - 44.0).max(300.0)
    };

    match settings {
        // ====================================================================
        // Custom renderers — plugins with unique UI requirements
        // ====================================================================

        PluginSettings::EQ {
            channels,
            filters,
            channel_filters,
            per_channel_mode,
            ..
        } => {
            let selected_band_idx = selected_band_idx.min(filters.len().saturating_sub(1));
            render_eq_plugin(
                entity.clone(),
                plugin_idx,
                ui_eq::EqRenderState {
                    channels: *channels,
                    filters,
                    channel_filters,
                    per_channel_mode: *per_channel_mode,
                    is_editing,
                    selected_param,
                    selected_band_idx,
                    midi_overlay,
                },
                theme,
                cx,
            )
            .into_any_element()
        }

        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
            tilt_correction,
            tilt_reference,
        } => render_spectrum_analyzer_plugin(
            entity.clone(),
            plugin_idx,
            ui_spectrum::SpectrumRenderState {
                num_bins: *num_bins,
                min_freq: *min_freq,
                max_freq: *max_freq,
                smoothing: *smoothing,
                tilt_correction: *tilt_correction,
                tilt_reference: *tilt_reference,
                tilt_select_open: spectrum_tilt_select_open,
                reference_select_open: spectrum_reference_select_open,
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
            channel_states,
        } => {
            let speaker_config = plugin_chain.speaker_config_at_index(plugin_idx);
            render_matrix_plugin(
                entity.clone(),
                plugin_idx,
                ui_matrix::MatrixRenderState {
                    input_channels: *input_channels,
                    output_channels: *output_channels,
                    matrix,
                    channel_states,
                    speaker_config,
                    is_editing,
                    selected_param,
                    selected_cell: None,
                },
                theme,
            )
            .into_any_element()
        }

        PluginSettings::LoudnessMonitor => {
            render_loudness_monitor_plugin(loudness, plugin_idx, is_editing, theme)
                .into_any_element()
        }

        // MultibandCompressor/Expander: band selection UI requires custom rendering
        PluginSettings::MultibandCompressor {
            num_bands,
            crossover_preset,
            crossover_freq_1,
            crossover_freq_2,
            crossover_freq_3,
            crossover_freq_4,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            mix,
            link_channels,
            bands,
            ..
        } => {
            let selected_band_idx = selected_band_idx.min(bands.len());
            let (dt, dr, da, drl, dk, dm, dam, dact, ds, db) = if selected_band_idx > 0 {
                let b = &bands[selected_band_idx - 1];
                (
                    b.threshold_db.map(|v| v as f64).unwrap_or(*threshold_db),
                    b.ratio.map(|v| v as f64).unwrap_or(*ratio),
                    b.attack_ms.map(|v| v as f64).unwrap_or(*attack_ms),
                    b.release_ms.map(|v| v as f64).unwrap_or(*release_ms),
                    b.knee_db.map(|v| v as f64).unwrap_or(*knee_db),
                    b.makeup_gain_db as f64,
                    b.auto_makeup,
                    b.active,
                    b.solo,
                    b.bypass,
                )
            } else {
                (*threshold_db, *ratio, *attack_ms, *release_ms, *knee_db, 0.0, false, true, false, false)
            };
            render_mb_compressor_plugin(
                entity.clone(),
                plugin_idx,
                ui_mb_compressor::MbCompressorRenderState {
                    num_bands: *num_bands, crossover_preset: *crossover_preset,
                    crossover_freq_1: *crossover_freq_1, crossover_freq_2: *crossover_freq_2,
                    crossover_freq_3: *crossover_freq_3, crossover_freq_4: *crossover_freq_4,
                    threshold_db: dt, ratio: dr, attack_ms: da, release_ms: drl,
                    knee_db: dk, makeup_gain_db: dm, auto_makeup: dam,
                    active: dact, solo: ds, bypass: db, mix: *mix,
                    link_channels: *link_channels, is_editing, selected_param,
                    selected_band_idx,
                },
                theme,
            )
            .into_any_element()
        }
        PluginSettings::MultibandExpander {
            num_bands,
            crossover_preset,
            crossover_freq_1,
            crossover_freq_2,
            crossover_freq_3,
            crossover_freq_4,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            range_db,
            knee_db,
            hysteresis_db,
            hold_ms,
            mix,
            link_channels,
            bands,
            ..
        } => {
            let selected_band_idx = selected_band_idx.min(bands.len());
            let (dt, dr, da, drl, drng, dk, dh, dhold, dam, dact, ds, db) = if selected_band_idx > 0 {
                let b = &bands[selected_band_idx - 1];
                (
                    b.threshold_db.map(|v| v as f64).unwrap_or(*threshold_db),
                    b.ratio.map(|v| v as f64).unwrap_or(*ratio),
                    b.attack_ms.map(|v| v as f64).unwrap_or(*attack_ms),
                    b.release_ms.map(|v| v as f64).unwrap_or(*release_ms),
                    b.range_db.map(|v| v as f64).unwrap_or(*range_db),
                    b.knee_db.map(|v| v as f64).unwrap_or(*knee_db),
                    b.hysteresis_db.map(|v| v as f64).unwrap_or(*hysteresis_db),
                    b.hold_ms.map(|v| v as f64).unwrap_or(*hold_ms),
                    b.auto_makeup, b.active, b.solo, b.bypass,
                )
            } else {
                (*threshold_db, *ratio, *attack_ms, *release_ms, *range_db, *knee_db,
                 *hysteresis_db, *hold_ms, false, true, false, false)
            };
            render_mb_expander_plugin(
                entity.clone(),
                plugin_idx,
                ui_mb_expander::MbExpanderRenderState {
                    num_bands: *num_bands, crossover_preset: *crossover_preset,
                    crossover_freq_1: *crossover_freq_1, crossover_freq_2: *crossover_freq_2,
                    crossover_freq_3: *crossover_freq_3, crossover_freq_4: *crossover_freq_4,
                    threshold_db: dt, ratio: dr, attack_ms: da, release_ms: drl,
                    range_db: drng, knee_db: dk, hysteresis_db: dh, hold_ms: dhold,
                    auto_makeup: dam, active: dact, solo: ds, bypass: db, mix: *mix,
                    link_channels: *link_channels, is_editing, selected_param,
                    selected_band_idx,
                },
                theme,
            )
            .into_any_element()
        }

        // ====================================================================
        // Generic layout renderer — all plugins with PluginLayout definitions
        // ====================================================================
        _ => ui_layout_renderer::render_from_layout(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            plugin_data.as_ref(),
            available_width,
            theme,
        ),
    }
}
