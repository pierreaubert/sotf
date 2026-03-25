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
pub mod custom_view_registry;
pub mod editing;
pub mod level_meters;
pub mod theme;
pub mod ticks;

mod ui_compressor;
mod ui_downmix;
pub mod ui_eq;
mod ui_gain;
mod ui_gate;
mod ui_graph;
pub mod ui_layout_renderer;
mod ui_limiter;
mod ui_loudness;
mod ui_matrix;
mod ui_mb_compressor;
mod ui_mb_expander;
mod ui_mono_to_stereo;
mod ui_mute_solo;
pub mod ui_plugin_shell;
mod ui_rack;
mod ui_simple;
mod ui_spectrum;
mod ui_upmixer;

pub use common::*;
pub use editing::get_param_count;
pub use level_meters::{
    LevelMeterElement, MeterColors, db_to_position, render_gr_meter, render_gradient_meter,
    render_lufs_with_true_peak, render_peak_meter,
};
pub use sotf_audio_player_midi::mapping::MidiOverlay;
pub use theme::*;
pub use ticks::{ScaleType, TickConfig, render_tick_row};

pub use ui_compressor::render_compressor_plugin;
pub use ui_downmix::render_downmix_plugin;
pub use ui_eq::render_eq_plugin;
pub use ui_gain::render_gain_plugin;
pub use ui_gate::render_gate_plugin;
pub use ui_limiter::render_limiter_plugin;
pub use ui_loudness::render_loudness_monitor_plugin;
pub use ui_matrix::render_matrix_plugin;
pub use ui_mb_compressor::render_mb_compressor_plugin;
pub use ui_mb_expander::render_mb_expander_plugin;
pub use ui_mono_to_stereo::render_mono_to_stereo_plugin;
pub use ui_mute_solo::render_mute_solo_plugin;
pub use ui_plugin_shell::render_plugin_shell;
pub use ui_rack::PluginDragInfo;
pub use ui_simple::render_simple_plugin_view;
pub use ui_spectrum::{
    MeterData, SpectrumColors, SpectrumElement, render_spectrum_analyzer_plugin,
};
pub use ui_upmixer::render_upmixer_plugin;

use crate::app::AppState;
use crate::theme::Theme;
use crate::ui::PlayerView;
use custom_view_registry::{CustomViewRenderContext, GpuiViewRegistry};
use gpui::*;
use sotf_audio_player::{PluginChain, PluginSettings};

/// Render plugin-specific content based on plugin type.
///
/// Uses the `GpuiViewRegistry` for plugins with custom UIs (EQ, Spectrum, etc.)
/// and falls back to the declarative `PluginLayout` renderer for everything else.
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

    // Compute available width for the plugin content area.
    let available_width = {
        let window_width = state.app.ui_state.window_width;
        let is_standalone = state.app.ui_state.current_screen == crate::app::Screen::Studio;
        let content_width = if is_standalone {
            window_width
        } else {
            let layout_state = state.layout.read(cx);
            let rack_ratio = if layout_state.rack_panel_collapsed {
                0.0
            } else {
                layout_state.rack_h_ratio
            };
            rack_ratio * window_width
        };
        let output_meter_width = if state.app.output_meter_collapsed {
            0.0
        } else {
            state.app.output_meter_width
        };
        (content_width - output_meter_width - 44.0).max(300.0)
    };

    // Check if this plugin has a registered custom view
    let registry = GpuiViewRegistry::new();
    let type_key = custom_view_registry::plugin_type_key(settings);

    if let Some(render_fn) = registry.get(type_key) {
        let ctx = CustomViewRenderContext {
            entity: entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            selected_band_idx,
            theme,
            loudness,
            plugin_data,
            spectrum_tilt_select_open,
            spectrum_reference_select_open,
            plugin_chain,
            midi_overlay,
        };
        return render_fn(&ctx, cx);
    }

    // Fallback: generic layout renderer for plugins with PluginLayout definitions
    ui_layout_renderer::render_from_layout(
        entity.clone(),
        plugin_idx,
        settings,
        is_editing,
        selected_param,
        auto_tab,
        plugin_data.as_ref(),
        available_width,
        theme,
    )
}
