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
pub mod spatial_spider;
pub mod theme;

mod ui_ab_compare;
mod ui_controller_view;
mod ui_dynamic_eq;
pub mod ui_eq;
mod ui_graph;
pub mod ui_layout_renderer;
mod ui_loudness;
mod ui_matrix;
mod ui_mb_compressor;
mod ui_mb_expander;
mod ui_multiband_common;
mod ui_mute_solo;
pub mod ui_plugin_shell;
mod ui_rack;
mod ui_simple;
mod ui_spectrum;
mod ui_upmixer;

pub use common::*;
use editing::PluginEditingManager;
pub use editing::get_param_count;
pub use gpui_audio_kit::{
    LevelMeterElement, MeterColors, ScaleType, TickConfig, db_to_position, render_tick_row,
};
use level_meters::LevelMeterManager;
pub use level_meters::{render_gr_meter, render_gradient_meter, render_lufs_with_true_peak};
pub use sotf_audio_player_midi::mapping::MidiOverlay;
pub use theme::*;

pub use gpui_audio_kit::{MeterData, SpectrumColors, SpectrumElement};
#[doc(hidden)]
pub use ui_ab_compare::ab_compare_view_state;
pub use ui_controller_view::render_controller_view;
pub use ui_dynamic_eq::render_dynamic_eq_plugin;
pub use ui_eq::render_eq_plugin;
pub use ui_loudness::render_loudness_monitor_plugin;
pub use ui_matrix::render_matrix_plugin;
#[doc(hidden)]
pub use ui_matrix::{checked_matrix_cell_index, matrix_settings_mut_by_instance_id};
pub use ui_mb_compressor::render_mb_compressor_plugin;
pub use ui_mb_expander::render_mb_expander_plugin;
pub use ui_mute_solo::render_mute_solo_plugin;
pub use ui_plugin_shell::render_plugin_shell;
pub use ui_rack::PluginDragInfo;
pub use ui_simple::render_simple_plugin_view;
pub use ui_spectrum::render_spectrum_analyzer_plugin;
pub use ui_upmixer::render_upmixer_plugin;

use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use crate::ui::PlayerView;
use custom_view_registry::{CustomViewRenderContext, GpuiViewRegistry};
use gpui::*;
use sotf_audio_player::{PluginGraph, PluginSettings};
use std::sync::OnceLock;

fn gpui_view_registry() -> &'static GpuiViewRegistry {
    static GPUI_VIEW_REGISTRY: OnceLock<GpuiViewRegistry> = OnceLock::new();
    GPUI_VIEW_REGISTRY.get_or_init(GpuiViewRegistry::new)
}

#[doc(hidden)]
pub fn plugin_instance_id_for_render(
    input_mode: crate::app::InputMode,
    editing_node_id: Option<sotf_audio_player::GraphNodeId>,
    plugin_graph: &PluginGraph,
    plugin_idx: usize,
) -> Option<usize> {
    if input_mode == crate::app::InputMode::EditingPluginNode {
        editing_node_id
            .and_then(|node_id| plugin_graph.nodes.get(&node_id))
            .map(|node| node.plugin.id)
    } else {
        plugin_graph.get_plugin(plugin_idx).map(|plugin| plugin.id)
    }
}

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
    loudness: Option<std::sync::Arc<sotf_audio_player::LoudnessData>>,
    plugin_data: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    spectrum_tilt_select_open: bool,
    spectrum_reference_select_open: bool,
    plugin_graph: &PluginGraph,
    midi_overlay: Option<&MidiOverlay>,
    eq_chart_focus_handle: FocusHandle,
    plugin_exact_entry_focus_handle: FocusHandle,
    cx: &mut Context<PlayerView>,
    solved_editor_width: Option<f32>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let state = entity.read(cx);
    let text = PluginCommonTranslations::for_language(state.app.ui_state.language);
    let plugin_instance_id = plugin_instance_id_for_render(
        state.app.ui_state.input_mode,
        state.app.plugin_state.graph_state.editing_graph_node_uuid,
        plugin_graph,
        plugin_idx,
    );
    let auto_tab = state
        .app
        .plugin_ui
        .plugin_auto_tab
        .get(&plugin_idx)
        .copied()
        .unwrap_or(0);
    let sections = plugin_instance_id.map(|id| {
        (
            id,
            state
                .app
                .plugin_state
                .plugin_ui_state
                .plugin_sections
                .get(&id)
                .cloned()
                .unwrap_or_default(),
        )
    });
    let plugin_theme = plugin_theme_id_for_app_theme(
        state
            .app
            .plugin_state
            .rack_theme_state
            .resolved_id(plugin_idx),
        theme,
        state.app.ui_state.theme_id,
    )
    .theme();

    let window_width = state.app.ui_state.window_width;
    let combined_scale = crate::ui::compute_combined_scale(
        window_width,
        state.app.ui_state.window_height,
        state.app.ui_state.font_scale,
        state.app.ui_state.min_font_size_px,
        state.app.ui_state.max_font_size_px,
    );

    // The caller supplies the measured content box after meters and shell
    // padding. The initial compact fallback is replaced after layout.
    let available_width = solved_editor_width
        .filter(|width| width.is_finite())
        .unwrap_or(320.0 * combined_scale)
        .max(1.0);

    // Overlay the chassis theme onto the global app theme so custom views
    // and the layout renderer share the same chassis-aware colors. The owned
    // `chassis_theme` lives for the rest of the function so `ctx.theme` can
    // borrow it.
    let chassis_theme = plugin_theme.apply_to(theme);

    let registry = gpui_view_registry();
    let type_key = custom_view_registry::plugin_type_key(settings);

    let minimum_width = if settings.param_specs().len() <= 3 {
        160.0
    } else if settings.layout().is_some()
        && (registry.get(type_key).is_none()
            || matches!(
                settings,
                PluginSettings::Crossfeed { .. }
                    | PluginSettings::Denoiser { .. }
                    | PluginSettings::MultibandCompressor { .. }
                    | PluginSettings::MultibandExpander { .. }
            ))
    {
        // Declarative editors wrap their controls. Require room for one
        // control, not a complete row, so scaled narrow windows stay usable.
        200.0
    } else {
        320.0
    };
    let content = if available_width < minimum_width * combined_scale {
        use gpui::prelude::*;
        gpui::div()
            .w_full()
            .min_w_0()
            .child(gpui_ui_kit::Text::body(text.editor_too_narrow()))
            .into_any_element()
    } else if let Some(render_fn) = registry.get(type_key) {
        let ctx = CustomViewRenderContext {
            entity: entity.clone(),
            plugin_idx,
            plugin_instance_id,
            settings,
            available_width,
            layout_scale: combined_scale,
            is_editing,
            selected_param,
            selected_band_idx,
            theme: &chassis_theme,
            plugin_theme: &plugin_theme,
            loudness,
            plugin_data,
            spectrum_tilt_select_open,
            spectrum_reference_select_open,
            plugin_graph,
            midi_overlay,
            eq_chart_focus_handle,
            plugin_exact_entry_focus_handle,
        };
        render_fn(&ctx, cx)
    } else if settings.layout().is_some() {
        // Fallback: generic layout renderer for plugins with PluginLayout definitions.
        // Snapshot live audio data for plugins whose layout opts into the
        // spatial-spider visualization. The renderer ignores this when the
        // plugin's layout has no matching `VizSlot::Custom` entry.
        let spider_snapshot = {
            let app = &entity.read(cx).app;
            Some(
                crate::components::plugins::spatial_spider::SpatialSpiderSnapshot {
                    loudness: app.playback.loudness_info.clone(),
                    ui: app.plugin_ui.spatial_spider.clone(),
                },
            )
        };
        ui_layout_renderer::render_from_layout(
            &d,
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            sections,
            plugin_data.as_ref(),
            available_width,
            combined_scale,
            text,
            theme,
            &plugin_theme,
            spider_snapshot,
        )
    } else {
        gpui::div().into_any_element()
    };

    // This is the scrollport's direct child. Its height must include the full
    // editor; filling the viewport hides descendant overflow from scrolling.
    if let Some(plugin) = plugin_graph.get_plugin(plugin_idx) {
        gpui::div()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_stretch()
            .bg(chassis_theme.background)
            .child(render_app_plugin_shell(
                &d,
                entity,
                plugin_idx,
                &plugin.plugin_type(),
                plugin_graph.is_input_monitor(plugin_idx),
                plugin_graph.is_output_monitor(plugin_idx),
                plugin.enabled,
                text,
                &chassis_theme,
                content,
            ))
            .into_any_element()
    } else {
        gpui::div()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .bg(chassis_theme.background)
            .child(content)
            .into_any_element()
    }
}

pub(crate) fn render_app_plugin_shell(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    plugin_type: &sotf_audio_player::PluginType,
    is_input_monitor: bool,
    is_output_monitor: bool,
    enabled: bool,
    text: PluginCommonTranslations,
    theme: &Theme,
    content: impl IntoElement,
) -> AnyElement {
    let entity_for_bypass = entity.clone();
    ui_plugin_shell::render_plugin_shell(
        d,
        plugin_idx,
        plugin_type,
        is_input_monitor,
        is_output_monitor,
        enabled,
        text,
        theme,
        content,
        Some(Box::new(move |target_enabled, _window, cx| {
            if target_enabled != enabled {
                entity_for_bypass.update(cx, |state, _| {
                    state.app.toggle_plugin(plugin_idx);
                    state.app.update_level_meter_groups();
                });
            }
        })),
    )
    .into_any_element()
}
use crate::app::i18n::PluginCommonTranslations;
