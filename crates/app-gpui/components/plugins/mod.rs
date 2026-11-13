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

mod external_numeric;
mod ui_ab_compare;
mod ui_controller_view;
mod ui_dynamic_eq;
pub mod ui_eq;
pub mod ui_external;
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
mod ui_xtc;

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
    solved_editor_height: Option<f32>,
    rack_scroll_handle: Option<gpui::ScrollHandle>,
    matrix_scroll_handle: gpui::ScrollHandle,
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
    // EQ has no horizontal shell padding or side borders, unlike the other editors.
    let available_width = if matches!(settings, sotf_audio_player::PluginSettings::EQ { .. }) {
        available_width + 2.0 * d.card.0 * 16.0 * combined_scale + 2.0
    } else {
        available_width
    };
    let available_height = solved_editor_height
        .filter(|height| height.is_finite() && *height > 0.0)
        .unwrap_or(state.app.ui_state.window_height * 0.5);

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
    let analog_spectrum = matches!(settings, PluginSettings::AnalogEq { .. }).then(|| {
        let playback = &state.app.playback;
        (
            playback.spectrum_info.as_ref().map(|data| {
                crate::app::state::ui::SpectrumViewState::display_magnitudes(&data.magnitudes)
            }),
            f64::from(
                playback
                    .spectrum_output_sample_rate()
                    .filter(|rate| *rate > 0)
                    .unwrap_or(48_000),
            ),
        )
    });
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
            layout_choice_menu: state.app.plugin_ui.layout_choice_menu,
            layout_choice_highlight: state.app.plugin_ui.layout_choice_highlight,
            settings,
            available_width,
            available_height,
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
            rack_scroll_handle,
            matrix_scroll_handle,
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
            plugin_instance_id,
            state.app.plugin_ui.layout_choice_menu,
            state.app.plugin_ui.layout_choice_highlight,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            sections,
            plugin_data.as_ref(),
            available_width,
            available_height,
            combined_scale,
            text,
            theme,
            &plugin_theme,
            spider_snapshot,
        )
    } else {
        gpui::div().into_any_element()
    };

    let content = if let Some((spectrum, sample_rate)) = analog_spectrum {
        use gpui::prelude::*;
        div()
            .flex()
            .flex_col()
            .gap(d.gap)
            .min_w_0()
            .child(ui_eq::render_analog_eq_graph(
                settings,
                spectrum,
                sample_rate,
                available_width.max(1.0),
                (160.0 * combined_scale).min((available_height * 0.4).max(1.0)),
                &chassis_theme,
            ))
            .child(content)
            .into_any_element()
    } else {
        content
    };

    let content = if matches!(
        settings,
        PluginSettings::BandMerge { .. }
            | PluginSettings::Downmix { .. }
            | PluginSettings::Limiter { .. }
            | PluginSettings::Aec { .. }
            | PluginSettings::Beamformer { .. }
    ) {
        use gpui::prelude::*;
        if let Some(node) = plugin_instance_id.and_then(|id| {
            plugin_graph
                .nodes
                .values()
                .find(|node| node.plugin.id == id)
        }) {
            let (_status_key, label) = match settings {
                PluginSettings::BandMerge { bands, .. } => (
                    "band_merge",
                    format!(
                        "{} input channels → {} output channels · {bands} bands",
                        node.input_channels, node.output_channels
                    ),
                ),
                PluginSettings::Downmix {
                    itu_mode,
                    matrix_ltrt,
                    ..
                } => {
                    let output = if *matrix_ltrt { "Lt/Rt" } else { "Stereo" };
                    let policy = if *itu_mode {
                        "ITU BS.775 coefficients"
                    } else {
                        "Custom channel gains"
                    };
                    (
                        "downmix",
                        format!(
                            "{} input channels → {output} ({} channels) · {policy}",
                            node.input_channels, node.output_channels
                        ),
                    )
                }
                PluginSettings::Limiter {
                    threshold_db, mix, ..
                } => (
                    "limiter",
                    format!(
                        "Output ceiling: {threshold_db:.1} dBFS · Wet mix: {:.0}%",
                        mix * 100.0
                    ),
                ),
                PluginSettings::Aec { .. } => (
                    "aec",
                    // AEC's fixed two-input contract consumes microphone first,
                    // then far-end reference; routing is not signal detection.
                    "Input 1: microphone · Input 2: far-end reference · Output: mono".to_string(),
                ),
                PluginSettings::Beamformer { .. } => (
                    "beamformer",
                    format!(
                        "{:.0} microphones · {} input channels · Spacing {:.1} cm · Steer {:.1}°",
                        settings.param_value(0).unwrap_or(2.0),
                        node.input_channels,
                        settings.param_value(1).unwrap_or(5.0),
                        settings.param_value(2).unwrap_or(0.0),
                    ),
                ),
                _ => unreachable!("Status is limited to explicitly matched plugin types"),
            };
            let status =
                gpui_ui_kit::Text::caption(label.clone()).color(chassis_theme.text_secondary);
            #[cfg(feature = "dev-api")]
            let status = {
                use crate::app::dev_api::{DevElementState, DevTrackExt};
                status.dev_track_with_state(
                    format!("plugin.layout.{_status_key}.channel_status.{plugin_idx}"),
                    DevElementState::default().text(label),
                )
            };
            gpui::div()
                .flex()
                .flex_col()
                .min_w_0()
                .gap(d.gap)
                .child(status)
                .child(content)
                .into_any_element()
        } else {
            content
        }
    } else {
        content
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
                cx,
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
    cx: &gpui::App,
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
    let state = entity.read(cx);
    let settings = if let Some(id) = state.app.plugin_state.graph_state.editing_graph_node_uuid {
        state
            .app
            .plugin_state
            .routing_controller()
            .graph
            .nodes
            .get(&id)
            .map(|node| &node.plugin.settings)
    } else {
        state
            .app
            .plugin_state
            .graph
            .get_plugin(plugin_idx)
            .map(|plugin| &plugin.settings)
    };
    let instance_id = if let Some(id) = state.app.plugin_state.graph_state.editing_graph_node_uuid {
        state
            .app
            .plugin_state
            .routing_controller()
            .graph
            .nodes
            .get(&id)
            .map(|node| node.plugin.id)
    } else {
        state
            .app
            .plugin_state
            .graph
            .get_plugin(plugin_idx)
            .map(|plugin| plugin.id)
    };
    let menu = state.app.plugin_ui.layout_choice_menu;
    let highlight = state.app.plugin_ui.layout_choice_highlight;
    let header_control = if matches!(plugin_type, sotf_audio_player::PluginType::EQ) {
        settings.map(|settings| {
            ui_eq::render_eq_header_controls(d, entity.clone(), plugin_idx, settings, theme, cx)
        })
    } else if matches!(plugin_type, sotf_audio_player::PluginType::LinearPhaseEq) {
        settings.map(|settings| {
            ui_eq::render_linear_phase_eq_header_controls(
                d,
                entity.clone(),
                plugin_idx,
                settings,
                theme,
                cx,
            )
        })
    } else if matches!(plugin_type, sotf_audio_player::PluginType::DynamicEq) {
        settings.map(|settings| {
            ui_dynamic_eq::render_dynamic_eq_header_controls(
                d,
                entity.clone(),
                plugin_idx,
                settings,
                text,
                theme,
            )
        })
    } else if matches!(
        plugin_type,
        sotf_audio_player::PluginType::MultibandCompressor
    ) {
        settings.map(|settings| {
            ui_mb_compressor::render_header(
                d,
                entity.clone(),
                plugin_idx,
                settings,
                text,
                theme,
                cx,
            )
        })
    } else if matches!(
        plugin_type,
        sotf_audio_player::PluginType::MultibandExpander
    ) {
        settings.map(|settings| {
            ui_multiband_common::render_dense_multiband_header(
                d,
                entity.clone(),
                plugin_idx,
                settings,
                text,
                theme,
                cx,
                &[15],
            )
        })
    } else if matches!(plugin_type, sotf_audio_player::PluginType::SpectrumAnalyzer) {
        settings.map(|settings| {
            ui_spectrum::render_header(d, entity.clone(), plugin_idx, settings, theme, cx)
        })
    } else if matches!(plugin_type, sotf_audio_player::PluginType::Matrix) {
        settings.zip(instance_id).map(|(settings, instance_id)| {
            ui_matrix::render_header(d, entity.clone(), instance_id, settings, theme, cx)
        })
    } else if matches!(plugin_type, sotf_audio_player::PluginType::ChannelMuteSolo) {
        settings.map(|settings| {
            ui_mute_solo::render_header(d, entity.clone(), plugin_idx, settings, text, theme)
        })
    } else if matches!(plugin_type, sotf_audio_player::PluginType::XTC) {
        settings.map(|settings| {
            ui_xtc::render_header(d, entity.clone(), plugin_idx, settings, text, theme, cx)
        })
    } else if matches!(plugin_type, sotf_audio_player::PluginType::Compressor) {
        settings.map(|settings| {
            let params = settings.param_specs();
            let mut row = gpui::div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(d.grid);
            for index in [8, 7, 14] {
                let mut param = params[index];
                param.name = text.label(param.name);
                row = row.child(ui_layout_renderer::render_compact_choice_param(
                    d,
                    entity.clone(),
                    plugin_idx,
                    index,
                    &param,
                    settings.param_value(index).unwrap_or(0.0),
                    theme,
                ));
            }
            row.into_any_element()
        })
    } else if matches!(plugin_type, sotf_audio_player::PluginType::Upmixer) {
        use gpui_ui_kit::{Select, SelectOption, SelectSize};
        use sotf_plugins::param_specs::{find_by_key, index_of, upmixer::PARAMS};
        let state = entity.read(cx);
        let selected = state
            .app
            .plugin_state
            .graph
            .get_plugin(plugin_idx)
            .and_then(|plugin| {
                if let sotf_audio_player::PluginSettings::Upmixer { speaker_config, .. } =
                    &plugin.settings
                {
                    Some(speaker_config.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "5.1".into());
        let snapshot = spatial_spider::SpatialSpiderSnapshot {
            loudness: state.app.playback.loudness_info.clone(),
            ui: state.app.plugin_ui.spatial_spider.clone(),
        };
        let config = spatial_spider::resolve_speaker_config(&snapshot, Some(&selected));
        let spatial = spatial_spider::render::render_spatial_header_menu(
            d,
            entity.clone(),
            plugin_idx,
            &snapshot,
            config,
            text,
            theme,
        );
        let is_open = state.app.plugin_ui.upmixer_config_open;
        let labels = find_by_key(PARAMS, "speaker_config").choice_labels();
        let selected_index = labels
            .iter()
            .position(|label| *label == selected)
            .unwrap_or(0);
        let highlighted_index = state.app.plugin_ui.upmixer_config_highlighted_index;
        let entity_for_toggle = entity.clone();
        let entity_for_highlight = entity.clone();
        let entity_for_output = entity.clone();
        let select = Select::new(("upmixer-header-output", plugin_idx))
            .aria_label(text.label("Output"))
            .size(SelectSize::Sm)
            .selected(selected)
            .options(
                labels
                    .iter()
                    .map(|label| SelectOption::new(*label, *label))
                    .collect(),
            )
            .is_open(is_open)
            .highlighted_index(highlighted_index)
            .theme(theme.to_select_theme())
            .on_toggle(move |open, _, cx| {
                entity_for_toggle.update(cx, |state, cx| {
                    state.app.plugin_ui.upmixer_config_open = open;
                    state.app.plugin_ui.upmixer_config_highlighted_index =
                        open.then_some(selected_index);
                    cx.notify();
                });
            })
            .on_highlight(move |index, _, cx| {
                entity_for_highlight.update(cx, |state, cx| {
                    state.app.plugin_ui.upmixer_config_highlighted_index = index;
                    cx.notify();
                });
            })
            .on_change(move |value, _, cx| {
                if let Some(index) = labels.iter().position(|label| *label == value.as_ref()) {
                    entity_for_output.update(cx, |state, cx| {
                        state.app.set_plugin_param(
                            plugin_idx,
                            index_of(PARAMS, "speaker_config"),
                            index as f64,
                        );
                        state.app.plugin_ui.upmixer_config_open = false;
                        state.app.plugin_ui.upmixer_config_highlighted_index = None;
                        if let Some(config) =
                            sotf_plugins::speaker_config::get_speaker_config(value.as_ref())
                        {
                            let ui = &mut state.app.plugin_ui.spatial_spider;
                            if !config
                                .speakers
                                .iter()
                                .any(|s| !s.is_lfe && s.channel == ui.correlation_ref_channel)
                            {
                                ui.correlation_ref_channel = config
                                    .speakers
                                    .iter()
                                    .find(|s| !s.is_lfe)
                                    .map_or(0, |s| s.channel);
                                if matches!(
                                    ui.spider_mode,
                                    spatial_spider::SpiderMode::CorrelationFromRef { .. }
                                ) {
                                    ui.spider_mode =
                                        spatial_spider::SpiderMode::CorrelationFromRef {
                                            ref_channel: ui.correlation_ref_channel,
                                        };
                                }
                            }
                        }
                        state.app.update_level_meter_groups();
                        cx.notify();
                    });
                }
            });
        #[cfg(feature = "dev-api")]
        let select = {
            use crate::app::dev_api::DevTrackExt;
            select.dev_track(format!("plugin.shell.{plugin_idx}.output"))
        };
        Some(
            gpui::div()
                .flex()
                .items_center()
                .child(spatial)
                .child(
                    gpui::div()
                        .key_context("UpmixerOutputSelect")
                        .w(gpui::rems(7.0))
                        .flex_shrink_0()
                        .mr(d.gap)
                        .child(select),
                )
                .into_any_element(),
        )
    } else {
        settings.and_then(|settings| {
            ui_layout_renderer::render_editor_setup(
                d,
                entity.clone(),
                plugin_idx,
                instance_id,
                menu,
                highlight,
                settings,
                text,
                theme,
                true,
            )
        })
    };
    let setup = settings.and_then(|settings| {
        ui_layout_renderer::render_editor_setup(
            d,
            entity.clone(),
            plugin_idx,
            instance_id,
            menu,
            highlight,
            settings,
            text,
            theme,
            false,
        )
    });
    let setup = setup.map(|setup| {
        use gpui_ui_kit::{Accordion, AccordionItem, AccordionMode};
        let instance_id = instance_id.unwrap_or(plugin_idx);
        let expanded = state
            .app
            .plugin_state
            .plugin_ui_state
            .plugin_sections
            .get(&instance_id)
            .is_some_and(|sections| sections.iter().any(|id| id == "editor-setup"));
        let entity = entity.clone();
        let accordion = Accordion::new()
            .items(vec![
                AccordionItem::new("editor-setup", text.label("Advanced")).content(setup),
            ])
            .mode(AccordionMode::Multiple)
            .expanded(if expanded {
                vec![gpui::SharedString::from("editor-setup")]
            } else {
                Vec::new()
            })
            .bordered(false)
            .rounded(false)
            .theme(theme.to_accordion_theme())
            .on_change(move |id, open, _, cx| {
                entity.update(cx, |state, cx| {
                    let sections = state
                        .app
                        .plugin_state
                        .plugin_ui_state
                        .plugin_sections
                        .entry(instance_id)
                        .or_default();
                    sections.retain(|section| section != id.as_ref());
                    if open {
                        sections.push(id.to_string());
                    }
                    cx.notify();
                });
            });
        #[cfg(feature = "dev-api")]
        let accordion = {
            use crate::app::dev_api::DevTrackExt;
            accordion.header_wrapper(move |_, header| {
                header
                    .dev_track(format!("plugin.setup.advanced.header.{plugin_idx}"))
                    .into_any_element()
            })
        };
        accordion.into_any_element()
    });
    let content = gpui::div()
        .flex()
        .flex_col()
        .min_w_0()
        .w_full()
        .gap(d.grid)
        .child(content)
        .children(setup);
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
        header_control,
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
