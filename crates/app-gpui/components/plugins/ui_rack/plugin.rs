// intentional-file: fixed pixel values here are graph and plugin control geometry.
use super::super::actions::ToggleUpmixerConfig;
use super::super::level_meters::render_gradient_meter;
use super::super::render_plugin_content;
use super::super::ui_plugin_shell::{plugin_accent_color as plugin_color, plugin_icon};
use super::plugin_drag_info::PluginDragInfo;
use super::short::short_name;
use super::short::short_name_with_permanent;
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::{DialogTranslations, PluginCommonTranslations, PluginRackTranslations};
use crate::app::state::plugin::{PluginUiView, available_controllers};
use crate::app::state::{DividerDragState, DividerType};
use crate::app::types::{PluginUpdateType, Screen};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;
use crate::components::plugins::theme::{PluginThemeId, plugin_theme_id_for_app_theme};
use crate::components::themed_tooltip as make_tooltip;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui::{MouseMoveEvent, MouseUpEvent};
use gpui_audio_kit::db_to_position;
use gpui_ui_kit::accessibility::{AccessibilityExt, AccessibilityNode, AriaProps, AriaRole};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, CollapseDirection, IconButton, IconButtonSize,
    IconButtonVariant, PaneDivider, PaneDividerTheme, Select, SelectOption, SelectSize, Toggle,
    ToggleSize, ToggleStyle,
};
use sotf_audio_player::{PluginType, UpmixerOutputSettings};
use sotf_plugins::param_specs::{find_by_key as pk, index_of, upmixer::PARAMS as UP};

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

/// Brief description for each plugin type (shown in add-plugin menu tooltips).
pub(crate) fn plugin_description(
    plugin_type: &PluginType,
    text: PluginCommonTranslations,
) -> &'static str {
    text.description(plugin_type)
}

fn output_meter_width_bounds(state: &crate::app::AppState) -> (f32, f32) {
    let (_, output_channels) = state.app.plugin_state.graph.compute_channel_flow();
    let num_output_groups = state.app.level_meters.groups.len();
    let scale = crate::ui::compute_combined_scale(
        state.app.ui_state.window_width,
        state.app.ui_state.window_height,
        state.app.ui_state.font_scale,
        state.app.ui_state.min_font_size_px,
        state.app.ui_state.max_font_size_px,
    );
    let meter_bar_width = 16.0 * scale;
    let bar_gap = 1.0;
    let group_padding = 4.0 * scale;
    let legend_width = 16.0 * scale;
    let input_width = 2.0 * meter_bar_width + bar_gap + group_padding;
    let output_width = output_channels as f32 * (meter_bar_width + bar_gap)
        + num_output_groups as f32 * group_padding;
    let minimum = input_width + legend_width + output_width + 8.0 * scale;
    (minimum, minimum * 2.0)
}

fn plugin_theme_select_value(theme: PluginThemeId) -> &'static str {
    match theme {
        PluginThemeId::Graphite => "graphite",
        PluginThemeId::StudioCream => "studio_cream",
        PluginThemeId::Brutalist => "brutalist",
    }
}

fn plugin_theme_from_select_value(value: &str) -> PluginThemeId {
    match value {
        "studio_cream" => PluginThemeId::StudioCream,
        "brutalist" => PluginThemeId::Brutalist,
        _ => PluginThemeId::Graphite,
    }
}

impl PlayerView {
    pub(crate) fn render_plugins_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let text = PluginRackTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let dismiss_hint_label =
            DialogTranslations::for_language(self.state.read(cx).app.ui_state.language)
                .about
                .close;
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let current_hint = self.state.read(cx).app.tutorial.current_hint.clone();

        div()
            .id("plugins-screen")
            .key_context("PluginRack")
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .bg(theme.background)
            .on_action(cx.listener(Self::toggle_upmixer_config))
            // Plugin parameter actions - needed for knob/slider interaction
            .on_action(cx.listener(Self::on_update_plugin_param))
            .on_action(cx.listener(Self::on_select_plugin_param))
            .on_action(cx.listener(Self::on_reset_plugin_param))
            .on_action(cx.listener(Self::on_start_knob_drag))
            // Global mouse move handler for knob/slider and divider dragging
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                let (knob_drag, divider_drag) = {
                    let state_read = view.state.read(cx);
                    (
                        state_read.app.drag.knob_drag,
                        state_read.app.layout.dragging_divider.clone(),
                    )
                };

                if let Some(drag) = knob_drag {
                    let mouse_y: f32 = event.position.y.into();
                    let delta_y = drag.start_y - mouse_y; // Inverted: up = positive (increase)
                    // Scale: 150px drag = full range. Shift provides a
                    // predictable fine-adjust mode for precise edits.
                    let range = drag.max - drag.min;
                    let drag_distance = if event.modifiers.shift { 1500.0 } else { 150.0 };
                    let value_delta = (delta_y as f64 / drag_distance) * range;
                    let new_value = (drag.start_value + value_delta).clamp(drag.min, drag.max);

                    // Update the parameter value via the plugin editing system
                    // (set_plugin_param already sets pending_plugin_update)
                    view.state.update(cx, |state, _cx| {
                        state
                            .app
                            .set_plugin_param(drag.plugin_idx, drag.param_idx, new_value);
                    });
                    cx.notify();
                } else if let Some(drag) = divider_drag {
                    view.state.update(cx, |state, cx| {
                        match drag.divider_type {
                            DividerType::InputMeter => {
                                // Dragging right increases input meter width
                                let mouse_x: f32 = event.position.x.into();
                                let delta_x = mouse_x - drag.start_x;
                                let new_width = (drag.start_width + delta_x).clamp(60.0, 200.0);
                                state.app.layout.input_meter_width = new_width;
                            }
                            DividerType::OutputMeter => {
                                // Dragging left increases output meter width
                                let mouse_x: f32 = event.position.x.into();
                                let delta_x = mouse_x - drag.start_x;
                                let (minimum, maximum) = output_meter_width_bounds(state);
                                let new_width =
                                    (drag.start_width - delta_x).clamp(minimum, maximum);
                                state.app.layout.output_meter_width = new_width;
                            }
                            DividerType::RackDetail => {
                                let window_height: f32 = window.bounds().size.height.into();
                                if window_height > 0.0 {
                                    let mouse_y: f32 = event.position.y.into();
                                    let delta_y = mouse_y - drag.start_x;
                                    let new_ratio = (drag.start_width + delta_y / window_height)
                                        .clamp(0.12, 0.65);
                                    state.layout.update(cx, |layout, _| {
                                        layout.rack_detail_ratio = new_ratio;
                                    });
                                }
                            }
                        }
                    });
                    cx.notify();
                }
            }))
            // Global mouse up handler to stop knob/slider and divider dragging
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, cx| {
                        if state.app.drag.knob_drag.is_some() {
                            state.app.drag.knob_drag = None;
                        }
                        let had_divider_drag = state.app.layout.dragging_divider.take().is_some();
                        if had_divider_drag {
                            let layout = state.layout.read(cx).clone();
                            if let Err(e) = state.app.save_config(&layout) {
                                log::warn!("Failed to save panel layout: {}", e);
                            }
                        }
                    });
                }),
            )
            // Contextual hint banner (dismissible and persisted as seen).
            // The Studio first-visit variant surfaces the otherwise hidden
            // rack keyboard model once; per-action hints reuse the same strip.
            .when_some(
                current_hint.filter(|h| {
                    matches!(
                        h.hint_id,
                        crate::components::dialogs::tutorial::HintId::StudioFirstVisit
                            | crate::components::dialogs::tutorial::HintId::FirstPluginAdded
                    )
                }),
                |el, hint| {
                    el.child(
                        div()
                            .id("hint-banner")
                            .cursor_pointer()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, cx| {
                                        state.app.dismiss_hint();
                                        let layout = state.layout.read(cx);
                                        if let Err(error) = state.app.save_config(layout) {
                                            log::error!("Failed to save config: {error}");
                                        }
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(crate::components::dialogs::tutorial::render_hint_banner(
                                &hint,
                                &theme,
                                d,
                                dismiss_hint_label,
                                cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    cx.stop_propagation();
                                    view.state.update(cx, |state, cx| {
                                        state.app.dismiss_hint();
                                        let layout = state.layout.read(cx);
                                        if let Err(error) = state.app.save_config(layout) {
                                            log::error!("Failed to save config: {error}");
                                        }
                                    });
                                    cx.notify();
                                }),
                            )),
                    )
                },
            )
            // When the plugin graph is non-linear (parallel branches, e.g.
            // after a multi-driver Room EQ "Apply as Graph"), the rack
            // can't represent the topology — `plugins()` returns an empty
            // Vec because `plugins_linear()` rejects branching graphs.
            // Show a redirect card instead of a misleading empty rack.
            .when(
                !self.state.read(cx).app.plugin_state.is_rack_available(),
                |d| d.child(self.render_non_linear_chain_notice(cx)),
            )
            .when(
                self.state.read(cx).app.plugin_state.is_rack_available(),
                |d| {
                    let (is_collapsed, rack_detail_ratio) = {
                        let state = self.state.read(cx);
                        let layout = state.layout.read(cx);
                        (
                            state.app.layout.rack_detail_collapsed
                                || layout.rack_detail_ratio <= 0.05,
                            layout.rack_detail_ratio.clamp(0.12, 0.65),
                        )
                    };
                    d
                        // Plugin Rack Strip (top) - only show if not collapsed
                        .when(!is_collapsed, |d| {
                            d.child(
                                div()
                                    .id("plugin-rack-viewport")
                                    .h(relative(rack_detail_ratio))
                                    .flex_shrink_0()
                                    // The rack cards keep a stable touch target, but the
                                    // user can drag the divider low enough that the strip is
                                    // shorter than one card row. Keep that state usable by
                                    // allowing the strip itself to scroll vertically instead
                                    // of clipping its controls behind the detail divider.
                                    .overflow_scroll()
                                    .child(self.render_plugin_rack(cx)),
                            )
                        })
                        // Horizontal divider between rack and detail panel
                        .child({
                            let divider_theme = PaneDividerTheme {
                                background: theme.background,
                                background_hover: theme.surface_hover,
                                background_collapsed: theme.surface,
                                foreground: theme.text_muted,
                                foreground_hover: theme.text_secondary,
                                border: theme.border,
                                tint: Rgba {
                                    a: 0.42,
                                    ..theme.accent
                                },
                                tint_hover: theme.accent,
                            };
                            let state_for_toggle = self.state.clone();
                            let state_for_drag = self.state.clone();
                            PaneDivider::horizontal("rack-detail-divider", CollapseDirection::Up)
                                .label(text.signal_chain)
                                .theme(divider_theme)
                                .thickness(px(4.0))
                                .collapsed(is_collapsed)
                                .on_toggle(move |collapsed, _window, cx| {
                                    state_for_toggle.update(cx, |s, cx| {
                                        s.app.layout.rack_detail_collapsed = collapsed;
                                        s.layout.update(cx, |layout, _| {
                                            layout.rack_detail_ratio =
                                                if collapsed { 0.0 } else { 0.22 };
                                            if let Err(e) = s.app.save_config(layout) {
                                                log::debug!("Config save failed: {e}");
                                            }
                                        });
                                    });
                                })
                                .on_drag_start(move |pos, _window, cx| {
                                    state_for_drag.update(cx, |s, cx| {
                                        let start_width =
                                            s.layout.read(cx).rack_detail_ratio.clamp(0.12, 0.65);
                                        s.app.layout.rack_detail_collapsed = false;
                                        s.app.layout.dragging_divider = Some(DividerDragState {
                                            divider_type: DividerType::RackDetail,
                                            start_x: pos,
                                            start_width,
                                        });
                                    });
                                })
                        })
                        // Parameter Panel (bottom, fills remaining space)
                        .child(self.render_plugin_detail_panel(cx))
                },
            )
    }

    /// Render the empty-state notice shown in Studio when the plugin graph
    /// has a non-linear topology that the rack strip can't render.
    pub(super) fn render_non_linear_chain_notice(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use gpui_ui_kit::{
            Button, ButtonVariant, Card, StackSpacing, Text, TextSize, TextWeight, VStack,
        };
        let d = Ds::from_cx(cx);
        let text = PluginRackTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let theme = self.state.read(cx).app.ui_state.theme.clone();

        div()
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .p(d.card)
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new(text.graph_routing_title)
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new(text.graph_routing_description)
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("open_graph_view", text.open_graph_view)
                                    .variant(ButtonVariant::Primary)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(cx.listener(|view, _, _, cx| {
                                        view.state.update(cx, |state, _| {
                                            state.app.ui_state.current_screen = Screen::PluginGraph;
                                        });
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }

    /// Render the horizontal plugin rack strip
    pub(super) fn render_plugin_rack(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let text = PluginRackTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let (
            plugins_data,
            selected_idx,
            theme,
            preset_open,
            preset_list,
            last_loaded_preset,
            has_pending_update,
            chain_input_channels,
            chain_output_channels,
            chain_sample_rate,
            chain_buffer_frames,
        ) = {
            let state = self.state.read(cx);
            let graph = &state.app.plugin_state.graph;
            let plugins: Vec<_> = graph
                .plugins()
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        p.plugin_type().clone(),
                        p.enabled,
                        // Prefer the plugin's custom user-facing name (e.g.
                        // "Room EQ", "Broadband EQ") so exported room-EQ
                        // instances are distinguishable in the rack.
                        p.display_name(),
                        p.is_permanent(),
                        graph.is_input_monitor(i),
                        graph.is_output_monitor(i),
                    )
                })
                .collect();
            let preset_open = state.app.plugin_state.preset_state.plugin_preset_open;
            let preset_list = state
                .app
                .plugin_state
                .preset_state
                .plugin_preset_list
                .clone();
            let last_loaded_preset = state.app.plugin_state.last_loaded_preset.clone();
            let has_pending_update = state
                .app
                .plugin_state
                .update_state
                .pending_plugin_update
                .is_some();

            let (chain_input_channels, chain_output_channels) = graph.compute_channel_flow();

            let chain_sample_rate = state.app.audio_device_state.hal_config.sample_rate;
            let chain_buffer_frames = state.app.audio_device_state.hal_config.buffer_frames;

            (
                plugins,
                state.app.plugin_state.selected_plugin_index,
                state.app.ui_state.theme.clone(),
                preset_open,
                preset_list,
                last_loaded_preset,
                has_pending_update,
                chain_input_channels,
                chain_output_channels,
                chain_sample_rate,
                chain_buffer_frames,
            )
        };

        // Build the chain summary string: "5 plugins · 2ch → 6ch · 48 kHz · ~21 ms".
        // Latency is the buffer-frame round-trip estimate (one full buffer at
        // the configured sample rate).
        let chain_latency_ms =
            (chain_buffer_frames as f64 / chain_sample_rate.max(1) as f64) * 1000.0;
        let chain_plugin_count = format!("{} plugins", plugins_data.len());
        let chain_channels = format!("{chain_input_channels}ch → {chain_output_channels}ch");
        let chain_clock = format!(
            "{} | ~{:.1} ms",
            crate::app::state::audio_device::format_sample_rate(chain_sample_rate),
            chain_latency_ms,
        );

        // Pre-compute static data for plugin modules
        let modules_info: Vec<_> = plugins_data
            .iter()
            .enumerate()
            .map(
                |(idx, (pt, enabled, name, permanent, is_input_mon, is_output_mon))| {
                    (
                        idx,
                        plugin_color(pt, &theme),
                        plugin_icon(pt, *is_input_mon, *is_output_mon),
                        name.clone(),
                        *enabled,
                        selected_idx == idx,
                        pt.clone(),     // Include plugin type for short_name
                        *permanent,     // Include permanent flag
                        *is_input_mon,  // Input monitor flag
                        *is_output_mon, // Output monitor flag
                    )
                },
            )
            .collect();

        let is_empty = plugins_data.is_empty();
        let d = Ds::from_cx(cx);
        let show_add_plugin_menu = self.state.read(cx).app.plugin_ui.show_add_plugin_menu;
        let add_slot_id = ElementId::from("plugin-add-slot");
        let add_slot_accessibility = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: add_slot_id.clone(),
            label: text.add_plugin_to_start.into(),
            props: add_slot_accessibility,
        });

        // Split: main plugins, then "+", then Matrix + output monitor
        // The "+" always appears just before the Matrix plugin.
        let trailing_start = modules_info
            .iter()
            .position(|(_, _, _, _, _, _, pt, _, _, _)| *pt == PluginType::Matrix)
            .unwrap_or(modules_info.len());
        let (main_modules, tail_modules) = modules_info.split_at(trailing_start);

        div()
            .relative()
            .flex()
            .flex_col()
            .bg(theme.background_secondary)
            .border_color(theme.border)
            // Plugin modules strip — Ozone-style rack
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .w_full()
                    .min_h(rems(7.0))
                    .bg(theme.background_secondary)
                    .child(
                        div()
                            .id("plugin-rack")
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(d.section)
                            .px(d.card)
                            .py(d.pad_x)
                            .overflow_x_scroll()
                    // Plugin modules - Ozone-style cards with left button column
                    .children(main_modules.iter().map(
                        |(idx, color, icon, _name, enabled, is_selected, plugin_type, is_permanent, is_input_mon, is_output_mon)| {
                            let idx = *idx;
                            let color = *color;
                            let icon = *icon;
                            let enabled = *enabled;
                            let is_selected = *is_selected;
                            let is_permanent = *is_permanent;
                            let is_input_mon = *is_input_mon;
                            let is_output_mon = *is_output_mon;
                            let theme_c = theme.clone();
                            let drag_info = PluginDragInfo {
                                source_index: idx,
                                name: short_name_with_permanent(plugin_type, is_input_mon, is_output_mon, is_permanent).to_string(),
                                color,
                                icon,
                                surface: theme_c.surface,
                                text_on_accent: theme_c.text_on_accent,
                            };
                            let drop_highlight = theme_c.feedback.drag_over_highlight;
                            let drop_border = theme_c.feedback.drag_over_border;
                            let accent_color = theme_c.accent;

                            // Module card with drop target
                            div()
                                .id(("plugin-module", idx))
                                .group("plugin-module")
                                .w(rems(8.0))
                                .h(rems(6.5))
                                .flex()
                                .flex_row()
                                .rounded(d.r_md)
                                .border_1()
                                .border_color(if is_selected {
                                    color
                                } else {
                                    theme_c.border
                                })
                                .bg(if is_selected {
                                    Theme::opacity_20pct(color)
                                } else if is_permanent {
                                    theme_c.background_secondary
                                } else {
                                    theme_c.surface
                                })
                                .when(!enabled, |d| d.opacity(0.5))
                                .shadow_sm()
                                .when(!is_permanent, |d| d.cursor_grab())
                                .hover(|s| s.border_color(color))
                                // Drag-over feedback
                                .drag_over::<PluginDragInfo>({
                                    move |style, _, _, _| {
                                        style
                                            .bg(drop_highlight)
                                            .border_color(drop_border)
                                            .border_l_4()
                                    }
                                })
                                // Drop to reorder
                                .on_drop(cx.listener(
                                    move |view, info: &PluginDragInfo, _window, cx| {
                                        let source = info.source_index;
                                        let target = idx;
                                        if source != target {
                                            view.state.update(cx, |state, _cx| {
                                                let chain_len =
                                                    state.app.plugin_state.graph.len();
                                                let source_is_monitor = state
                                                    .app
                                                    .plugin_state.graph
                                                    .get_plugin(source)
                                                    .map(|p| matches!(p.plugin_type(), PluginType::LoudnessMonitor))
                                                    .unwrap_or(false);
                                                if source_is_monitor && target != 0 && target != chain_len - 1 {
                                                    state.app.ui_state.toast_message = Some(
                                                        crate::app::ToastMessage::info(
                                                            "Level monitors must remain at a chain boundary",
                                                        ),
                                                    );
                                                    return;
                                                }
                                                match state.app.plugin_state.graph.move_plugin(source, target) {
                                                    Ok(()) => {
                                                        state.app.plugin_state.selected_plugin_index = target;
                                                        state.app.plugin_state.clear_confirmations();
                                                        state.app.plugin_state.update_state.pending_plugin_update =
                                                            Some(PluginUpdateType::Structural);
                                                        state.app.update_level_meter_groups();
                                                    }
                                                    Err(error) => {
                                                        state.app.ui_state.toast_message =
                                                            Some(crate::app::ToastMessage::error(error));
                                                    }
                                                }
                                            });
                                            cx.notify();
                                        }
                                    },
                                ))
                                // Start drag
                                .when(!is_permanent, |d| {
                                    d.on_drag(drag_info, |info, _position, _window, cx| {
                                        cx.new(|_| info.clone())
                                    })
                                })
                                // Click to select
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(
                                        move |view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.plugin_state.selected_plugin_index = idx;
                                                state.app.plugin_state.clear_confirmations();
                                            });
                                            cx.notify();
                                        },
                                    ),
                                )
                                // Left button column: A (active), S (solo), P (presets), X (remove/lock)
                                .child({
                                    let is_soloed = {
                                        let state = self.state.read(cx);
                                        state.app.plugin_state.chain_state.soloed_plugin_index == Some(idx)
                                    };
                                    let state_for_active = self.state.clone();
                                    let state_for_solo = self.state.clone();
                                    let state_for_presets = self.state.clone();
                                    let state_for_remove = self.state.clone();
                                    let active_theme = theme_c.clone();
                                    let solo_theme = theme_c.clone();
                                    let preset_theme = theme_c.clone();
                                    let active_tooltip_theme = theme_c.clone();
                                    let solo_tooltip_theme = theme_c.clone();
                                    let preset_tooltip_theme = theme_c.clone();
                                    let active_tooltip = if enabled {
                                        "Bypass plugin"
                                    } else {
                                        "Activate plugin"
                                    };
                                    let solo_tooltip = if is_soloed {
                                        "Disable plugin solo"
                                    } else {
                                        "Solo plugin"
                                    };
                                    let preset_tooltip = text.plugin_presets;

                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_between()
                                        .py(d.pad_y_half)
                                        .px(d.grid)
                                        .gap(d.grid)
                                        .h_full()
                                        .border_r_1()
                                        .border_color(theme_c.border)
                                        // A (Active/Bypass) button
                                        .child(
                                            div()
                                                .id(("plugin-active-tooltip", idx))
                                                .tooltip(move |_window, cx| {
                                                    make_tooltip(
                                                        active_tooltip,
                                                        &active_tooltip_theme,
                                                        cx,
                                                    )
                                                })
                        .child(dev_track!(
                            IconButton::with_child(
                                                        ("plugin-active", idx),
                                                        div()
                                                            .text_size(d.text_xs)
                                                            .font_weight(FontWeight::BOLD)
                                                            .child(if enabled { "A" } else { "B" }),
                                                    )
                                                    .variant(if enabled {
                                                        IconButtonVariant::Filled
                                                    } else {
                                                        IconButtonVariant::Outline
                                                    })
                                                    .size(IconButtonSize::Sm)
                                                    .theme(active_theme.to_icon_button_theme())
                                                    .aria_label(active_tooltip)
                                .on_click_event(move |_event, _window, cx| {
                                    state_for_active.update(cx, |state, _| {
                                        state.app.toggle_plugin(idx);
                                        state.app.update_level_meter_groups();
                                    });
                                }),
                            format!("rack.plugin.{idx}.toggle")
                        )),
                                        )
                                        // S (Solo) button — functional
                                        .child(
                                            div()
                                                .id(("plugin-solo-tooltip", idx))
                                                .tooltip(move |_window, cx| {
                                                    make_tooltip(
                                                        solo_tooltip,
                                                        &solo_tooltip_theme,
                                                        cx,
                                                    )
                                                })
                                                .child(
                                                    IconButton::with_child(
                                                        ("plugin-solo", idx),
                                                        div()
                                                            .text_size(d.text_xs)
                                                            .font_weight(FontWeight::BOLD)
                                                            .child("S"),
                                                    )
                                                    .variant(if is_soloed {
                                                        IconButtonVariant::Filled
                                                    } else {
                                                        IconButtonVariant::Outline
                                                    })
                                                    .size(IconButtonSize::Sm)
                                                    .theme(solo_theme.to_icon_button_theme())
                                                    .aria_label(solo_tooltip)
                                                    .on_click_event(move |_event, _window, cx| {
                                                        state_for_solo.update(cx, |state, _| {
                                                            state.app.toggle_plugin_solo(idx);
                                                            state.app.update_level_meter_groups();
                                                        });
                                                    }),
                                                ),
                                        )
                                        // P (Presets) button
                                        .child(
                                            div()
                                                .id(("plugin-presets-tooltip", idx))
                                                .tooltip(move |_window, cx| {
                                                    make_tooltip(
                                                        preset_tooltip,
                                                        &preset_tooltip_theme,
                                                        cx,
                                                    )
                                                })
                                                .child(
                                                    IconButton::with_child(
                                                        ("plugin-presets", idx),
                                                        div()
                                                            .text_size(d.text_xs)
                                                            .font_weight(FontWeight::BOLD)
                                                            .child("P"),
                                                    )
                                                    .variant(if preset_open == Some(idx) {
                                                        IconButtonVariant::Filled
                                                    } else {
                                                        IconButtonVariant::Outline
                                                    })
                                                    .size(IconButtonSize::Sm)
                                                    .theme(preset_theme.to_icon_button_theme())
                                                    .aria_label(preset_tooltip)
                                                    .on_click_event(move |_event, _window, cx| {
                                                        state_for_presets.update(cx, |state, _| {
                                                                let ps = &mut state.app.plugin_state;
                                                                if ps.preset_state.plugin_preset_open == Some(idx) {
                                                                    // Close
                                                                    ps.preset_state.plugin_preset_open = None;
                                                                    ps.preset_state.plugin_preset_save_mode = false;
                                                                    ps.preset_state.plugin_preset_input.clear();
                                                                    ps.preset_state.confirm_delete_preset = None;
                                                                } else {
                                                                    // Open and populate list
                                                                    if let Some(plugin) = ps.graph.get_plugin(idx) {
                                                                        let pt = plugin.plugin_type();
                                                                        ps.preset_state.plugin_preset_list = sotf_audio_player::PluginController::list_plugin_presets(&pt);
                                                                    }
                                                                    ps.preset_state.plugin_preset_open = Some(idx);
                                                                    ps.preset_state.plugin_preset_save_mode = false;
                                                                    ps.preset_state.plugin_preset_input.clear();
                                                                }
                                                        });
                                                    }),
                                                ),
                                        )
                                        // X (Remove) or lock icon for permanent
                                        .when(!is_permanent, |el| {
                                            let confirming = self.state.read(cx).app.plugin_state.preset_state.confirm_remove_plugin == Some(idx);
                                            let remove_state = state_for_remove.clone();
                                            let remove_theme = theme_c.clone();
                                            let remove_tooltip_theme = theme_c.clone();
                                            let remove_tooltip = if confirming {
                                                "Confirm plugin removal"
                                            } else {
                                                text.remove_plugin
                                            };
                                            el.child(
                                                div()
                                                    .id(("plugin-close-tooltip", idx))
                                                    .tooltip(move |_window, cx| {
                                                        make_tooltip(
                                                            remove_tooltip,
                                                            &remove_tooltip_theme,
                                                            cx,
                                                        )
                                                    })
                                                    .child(
                                                        IconButton::with_child(
                                                            ("plugin-close", idx),
                                                            div()
                                                                .text_size(d.text_xs)
                                                                .font_weight(FontWeight::BOLD)
                                                                .child(if confirming { "?" } else { "X" }),
                                                        )
                                                        .variant(if confirming {
                                                            IconButtonVariant::Filled
                                                        } else {
                                                            IconButtonVariant::Outline
                                                        })
                                                        .size(IconButtonSize::Sm)
                                                        .theme(remove_theme.to_icon_button_theme())
                                                        .aria_label(remove_tooltip)
                                                        .on_click_event(move |_event, _window, cx| {
                                                            remove_state.update(cx, |state, _| {
                                                                if state.app.plugin_state.preset_state.confirm_remove_plugin == Some(idx) {
                                                                    state.app.plugin_state.preset_state.confirm_remove_plugin = None;
                                                                    state.app.remove_plugin(idx);
                                                                    state.app.update_level_meter_groups();
                                                                } else {
                                                                    state.app.plugin_state.preset_state.confirm_remove_plugin = Some(idx);
                                                                }
                                                            });
                                                        }),
                                                    ),
                                            )
                                        })
                                        // Lock icon for permanent plugins
                                        .when(is_permanent, |d| {
                                            d.child(
                                                div()
                                                    .w(rems(1.125))
                                                    .h(rems(1.125))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(theme_c.text_muted)
                                                    .child(
                                                        Icon::new(IconName::Lock)
                                                            .small()
                                                            .color(theme_c.text_muted),
                                                    ),
                                            )
                                        })
                                })
                                // Right content area: name on top, icon preview center, drag handle bottom
                                // OR preset panel when P is active
                                .child({
                                    let right_panel = div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .h_full()
                                        .overflow_hidden();

                                    if preset_open == Some(idx) {
                                        // Preset panel
                                        let theme_p = theme_c.clone();
                                        right_panel
                                            .bg(theme_p.surface)
                                            .px(d.grid)
                                            .py(d.half_grid)
                                            // intentional: 1px divider between preset items
                                            .gap(px(1.0))
                                            .children(preset_list.iter().enumerate().map(
                                                |(pi, name)| {
                                                    let theme_i = theme_p.clone();
                                                    let name_load = name.clone();
                                                    let name_del = name.clone();
                                                    div()
                                                        .id(("preset-item", pi))
                                                        .flex()
                                                        .items_center()
                                                        .gap(d.half_grid)
                                                        .px(d.grid)
                                                        // intentional: 1px compact preset-item inset — do not scale
                                                        .py(px(1.0))
                                                        .rounded(d.r_sm)
                                                        .cursor_pointer()
                                                        .text_size(d.text_xs)
                                                        .text_color(theme_i.text_primary)
                                                        .hover(|s| s.bg(theme_i.background_secondary))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view, _e: &MouseUpEvent, _, cx| {
                                                                    cx.stop_propagation();
                                                                    view.state.update(cx, |state, _cx| {
                                                                        let ps = &mut state.app.plugin_state;
                                                                        match ps.load_plugin_preset(idx, &name_load) {
                                                                            Ok(_) => {
                                                                                ps.update_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                                                                            }
                                                                            Err(e) => {
                                                                                log::error!("Failed to load preset: {e}");
                                                                            }
                                                                        }
                                                                        ps.preset_state.plugin_preset_open = None;
                                                                    });
                                                                    cx.notify();
                                                                },
                                                            ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .overflow_hidden()
                                                                .text_ellipsis()
                                                                .child(name.clone()),
                                                        )
                                                        .child({
                                                            let confirming_del = self.state.read(cx).app.plugin_state.preset_state.confirm_delete_preset.as_ref()
                                                                .is_some_and(|(pi_idx, pn)| *pi_idx == idx && pn == name);
                                                            let name_confirm = name.clone();
                                                            div()
                                                                .id(("preset-del", pi))
                                                                .text_size(d.text_xs)
                                                                .cursor_pointer()
                                                                .when(confirming_del, |d| d.text_color(theme_i.error).font_weight(FontWeight::BOLD))
                                                                .when(!confirming_del, |d| d.text_color(theme_i.text_muted).hover(|s| s.text_color(theme_i.error)))
                                                                .on_mouse_up(
                                                                    MouseButton::Left,
                                                                    cx.listener(
                                                                        move |view, _e: &MouseUpEvent, _, cx| {
                                                                            cx.stop_propagation();
                                                                            view.state.update(cx, |state, _cx| {
                                                                                let ps = &mut state.app.plugin_state;
                                                                                let is_confirmed = ps.preset_state.confirm_delete_preset.as_ref()
                                                                                    .is_some_and(|(pi_idx, pn)| *pi_idx == idx && *pn == name_del);
                                                                                if is_confirmed {
                                                                                    // Second click: confirmed
                                                                                    ps.preset_state.confirm_delete_preset = None;
                                                                                    if let Some(plugin) = ps.graph.get_plugin(idx) {
                                                                                        let pt = plugin.plugin_type().clone();
                                                                                        let _ = sotf_audio_player::PluginController::delete_plugin_preset(&pt, &name_del);
                                                                                        ps.preset_state.plugin_preset_list = sotf_audio_player::PluginController::list_plugin_presets(&pt);
                                                                                    }
                                                                                } else {
                                                                                    // First click: ask for confirmation
                                                                                    ps.preset_state.confirm_delete_preset = Some((idx, name_confirm.clone()));
                                                                                }
                                                                            });
                                                                            cx.notify();
                                                                        },
                                                                    ),
                                                                )
                                                                .child(if confirming_del { "?" } else { "X" })
                                                        })
                                                }
                                            ))
                                            .child({
                                                let save_mode = self.state.read(cx).app.plugin_state.preset_state.plugin_preset_save_mode;
                                                let theme_s = theme_p.clone();
                                                if save_mode {
                                                    // Text input + confirm button
                                                    let preset_name = self.state.read(cx).app.plugin_state.preset_state.plugin_preset_input.clone();
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(d.half_grid)
                                                        .mt(d.half_grid)
                                                        .child({
                                                            let state_for_text = self.state.clone();
                                                            div()
                                                                .flex_1()
                                                                .child(
                                                                    gpui_ui_kit::Input::new("preset-name-input")
                                                                        .value(gpui::SharedString::from(preset_name))
                                        .placeholder(text.preset_name_placeholder)
                                                                        .size(gpui_ui_kit::InputSize::Xs)
                                                                        .bg_color(theme_s.background_secondary)
                                                                        .on_text_change(move |text, _window, cx| {
                                                                            state_for_text.update(cx, |state, _cx| {
                            state.app.plugin_state.preset_state.plugin_preset_input = text.to_string();
                                                                            });
                                                                        }),
                                                                )
                                                        })
                                                        .child(
                                                            div()
                                                                .id(("preset-confirm", idx))
                                                                .px(d.grid)
                                                                .py(d.half_grid)
                                                                .rounded(d.r_sm)
                                                                .cursor_pointer()
                                                                .text_size(d.text_xs)
                                                                .text_color(accent_color)
                                                                .border_1()
                                                                .border_color(accent_color)
                                                                .hover(|s| s.bg(accent_color).text_color(theme_s.text_on_accent))
                                                                .on_mouse_up(
                                                                    MouseButton::Left,
                                                                    cx.listener(move |view, _e: &MouseUpEvent, _, cx| {
                                                                        cx.stop_propagation();
                                                                        view.state.update(cx, |state, _cx| {
                                                                            let ps = &mut state.app.plugin_state;
                                                                            let name = ps.preset_state.plugin_preset_input.trim().to_string();
                                                                            if !name.is_empty() {
                                                                                match ps.save_plugin_preset(idx, &name) {
                                                                                    Ok(_) => {
                                                                                        if let Some(plugin) = ps.graph.get_plugin(idx) {
                                                                                            let pt = plugin.plugin_type().clone();
                                                                                            ps.preset_state.plugin_preset_list = sotf_audio_player::PluginController::list_plugin_presets(&pt);
                                                                                        }
                                                                                    }
                                                                                    Err(e) => {
                                                                                        log::error!("Failed to save preset: {e}");
                                                                                    }
                                                                                }
                                                                                ps.preset_state.plugin_preset_save_mode = false;
                                                                                ps.preset_state.plugin_preset_input.clear();
                                                                            }
                                                                        });
                                                                        cx.notify();
                                                                    }),
                                                                )
                                                .child(text.ok),
                                                        )
                                                        .into_any_element()
                                                } else {
                                                    // "+ Save" button that enters save mode
                                                    div()
                                                        .id(("preset-save-btn", idx))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .px(d.grid)
                                                        .py(d.half_grid)
                                                        .mt(d.half_grid)
                                                        .rounded(d.r_sm)
                                                        .cursor_pointer()
                                                        .text_size(d.text_xs)
                                                        .text_color(accent_color)
                                                        .border_1()
                                                        .border_color(accent_color)
                                                        .hover(|s| s.bg(accent_color).text_color(theme_s.text_on_accent))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(move |view, _e: &MouseUpEvent, _, cx| {
                                                                cx.stop_propagation();
                                                                view.state.update(cx, |state, _cx| {
                                                                    let ps = &mut state.app.plugin_state;
                                                                    let n = ps.preset_state.plugin_preset_list.len() + 1;
                                                                    ps.preset_state.plugin_preset_input = format!("Preset {n}");
                                                                    ps.preset_state.plugin_preset_save_mode = true;
                                                                });
                                                                cx.notify();
                                                            }),
                                                        )
                                    .child(text.save_new)
                                                        .into_any_element()
                                                }
                                            })
                                    } else {
                                        // Normal view
                                        right_panel
                                            .child(
                                                div()
                                                    .px(d.pad_y)
                                                    .pt(d.pad_y_half)
                                                    .text_size(d.text_xs)
                                                    .text_color(if is_selected { color } else { theme_c.text_primary })
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .overflow_hidden()
                                                    .text_ellipsis()
                                                    .child(short_name_with_permanent(plugin_type, is_input_mon, is_output_mon, is_permanent)),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        Icon::new(icon)
                                                            .size(IconSize::Xxl)
                                                            .color(color),
                                                    ),
                                            )
                                            .when(!is_permanent, |el| {
                                                el.child(
                                                    div()
                                                        .flex()
                                                        .justify_end()
                                                        .px(d.grid)
                                                        .pb(d.half_grid)
                                                        .text_color(theme_c.text_muted)
                                                        .child(
                                                            Icon::new(IconName::GripVertical)
                                                                .small()
                                                                .color(theme_c.text_muted),
                                                        ),
                                                )
                                            })
                                    }
                                })
                        },
                    ))
                    // "+" Add plugin slot (visually distinct from solid plugin boxes)
                    .child({
                        let theme_add = theme.clone();
                        let drop_highlight = theme.feedback.drag_over_highlight;
                        let drop_border = theme.feedback.drag_over_border;

                dev_track!(div()
                    .id(add_slot_id.clone())
                            .w(rems(8.0))
                            .h(rems(6.5))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .border_1()
                            .border_dashed()
                            .border_color(Theme::opacity_8pct(theme_add.text_muted))
                            .cursor_pointer()
                            .text_size(d.text_lg)
                            .text_color(theme_add.text_muted)
                            .hover(|s| s.border_color(theme_add.accent).text_color(theme_add.accent))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.plugin_ui.show_add_plugin_menu = !state.app.plugin_ui.show_add_plugin_menu;
                                    });
                                    cx.notify();
                                }),
                            )
                            // Drop zone at the end
                            .drag_over::<PluginDragInfo>({
                                move |style, _, _, _| {
                                    style.bg(drop_highlight).border_color(drop_border)
                                }
                            })
                            .on_drop(cx.listener(
                                move |view, info: &PluginDragInfo, _window, cx| {
                                    let source = info.source_index;
                                    view.state.update(cx, |state, _cx| {
                                        // Clamp target to last non-permanent plugin index
                                        let target = state
                                            .app
                                            .plugin_state
                                            .graph
                                            .user_plugin_insert_index()
                                            .saturating_sub(1);
                                        if source == target {
                                            return;
                                        }
                                        let source_is_monitor = state
                                            .app
                                            .plugin_state
                                            .graph
                                            .get_plugin(source)
                                            .map(|p| matches!(p.plugin_type(), PluginType::LoudnessMonitor))
                                            .unwrap_or(false);
                                        let chain_len =
                                            state.app.plugin_state.graph.len();
                                        if source_is_monitor && target != chain_len - 1 {
                                            state.app.ui_state.toast_message = Some(
                                                crate::app::ToastMessage::info(
                                                    "Level monitors must remain at a chain boundary",
                                                ),
                                            );
                                            return;
                                        }
                                        match state.app.plugin_state.graph.move_plugin(source, target) {
                                            Ok(()) => {
                                                state.app.plugin_state.selected_plugin_index = target;
                                                state.app.plugin_state.clear_confirmations();
                                                state.app.plugin_state.update_state.pending_plugin_update =
                                                    Some(PluginUpdateType::Structural);
                                                state.app.update_level_meter_groups();
                                            }
                                            Err(error) => {
                                                state.app.ui_state.toast_message =
                                                    Some(crate::app::ToastMessage::error(error));
                                            }
                                        }
                                    });
                                    cx.notify();
                                },
                            ))
                    .child(
                        Icon::new(IconName::Plus)
                            .small()
                            .color(theme_add.text_secondary),
                    ), "rack.add.open")
            })
                    // Trailing permanent plugins (output matrix/monitor) after "+"
                    .children(tail_modules.iter().map(
                        |(idx, color, icon, _name, enabled, is_selected, plugin_type, is_permanent, is_input_mon, is_output_mon)| {
                            let idx = *idx;
                            let color = *color;
                            let icon = *icon;
                            let enabled = *enabled;
                            let is_selected = *is_selected;
                            let is_permanent = *is_permanent;
                            let is_input_mon = *is_input_mon;
                            let is_output_mon = *is_output_mon;
                            let theme_c = theme.clone();
                            div()
                                .id(("plugin-module", idx))
                                .group("plugin-module")
                                .w(rems(8.0))
                                .h(rems(6.5))
                                .flex()
                                .flex_row()
                                .rounded(d.r_md)
                                .border_1()
                                .border_color(if is_selected { color } else { theme_c.border })
                                .bg(if is_selected {
                                    Theme::opacity_20pct(color)
                                } else {
                                    theme_c.background_secondary
                                })
                                .when(!enabled, |d| d.opacity(0.5))
                                .shadow_sm()
                                .hover(|s| s.border_color(color))
                                // Drop target for reordering
                                .drag_over::<PluginDragInfo>({
                                    let drop_highlight = theme_c.feedback.drag_over_highlight;
                                    let drop_border = theme_c.feedback.drag_over_border;
                                    move |style, _, _, _| {
                                        style
                                            .bg(drop_highlight)
                                            .border_color(drop_border)
                                            .border_l_4()
                                    }
                                })
                                .on_drop(cx.listener(
                                    move |view, info: &PluginDragInfo, _window, cx| {
                                        let source = info.source_index;
                                        view.state.update(cx, |state, _cx| {
                                            // Clamp to the last valid user-plugin position
                                            // (don't drop onto permanent tail plugins)
                                            let target = if is_permanent {
                                                state.app.plugin_state.graph.user_plugin_insert_index().saturating_sub(1)
                                            } else {
                                                idx
                                            };
                                            if source != target {
                                                match state.app.plugin_state.graph.move_plugin(source, target) {
                                                    Ok(()) => {
                                                        state.app.plugin_state.selected_plugin_index = target;
                                                        state.app.plugin_state.clear_confirmations();
                                                        state.app.plugin_state.update_state.pending_plugin_update =
                                                            Some(PluginUpdateType::Structural);
                                                        state.app.update_level_meter_groups();
                                                    }
                                                    Err(error) => {
                                                        state.app.ui_state.toast_message =
                                                            Some(crate::app::ToastMessage::error(error));
                                                    }
                                                }
                                            }
                                        });
                                        cx.notify();
                                    },
                                ))
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(
                                        move |view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.plugin_state.selected_plugin_index = idx;
                                                state.app.plugin_state.clear_confirmations();
                                            });
                                            cx.notify();
                                        },
                                    ),
                                )
                                // Left button column (just enable toggle + lock)
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap(d.half_grid)
                                        // intentional: 3px asymmetric inset to fit permanent tail module — do not scale
                                        .p(px(3.0))
                                        .border_r_1()
                                        .border_color(theme_c.border)
                                        .h_full()
                                        .child({
                                            let is_on = enabled;
                                            div()
                                                .id(("plugin-enable-tail", idx))
                                                .w(rems(1.125))
                                                .h(rems(1.125))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .bg(if is_on { color } else { theme_c.surface })
                                                .text_size(d.text_xs)
                                                .text_color(if is_on { theme_c.text_on_accent } else { theme_c.text_muted })
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |view, _e: &MouseUpEvent, _, cx| {
                                                            cx.stop_propagation();
                                                            view.state.update(cx, |state, _cx| {
                                                                state.app.toggle_plugin(idx);
                                                            });
                                                            cx.notify();
                                                        },
                                                    ),
                                                )
                                                .child(if is_on { "ON" } else { "OFF" })
                                        })
                                        // X (Remove) or lock icon
                                        .when(!is_permanent, |el| {
                                            let theme_tt = theme_c.clone();
                                            let confirming = self.state.read(cx).app.plugin_state.preset_state.confirm_remove_plugin == Some(idx);
                                            el.child(
                                                div()
                                                    .id(("plugin-close-tail", idx))
                                                    .w(rems(1.125))
                                                    .h(rems(1.125))
                                                    .rounded_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .cursor_pointer()
                                                    .when(confirming, |d| d.bg(theme_c.error).text_color(theme_c.text_on_accent))
                                                    .when(!confirming, |d| d.hover(|s| s.bg(theme_c.error).text_color(theme_c.text_on_accent)))
                                                    .text_size(d.text_xs)
                                                    .when(!confirming, |d| d.text_color(theme_c.text_muted))
                                                    .font_weight(FontWeight::BOLD)
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |view, _e: &MouseUpEvent, _, cx| {
                                                                cx.stop_propagation();
                                                                view.state.update(cx, |state, _cx| {
                                                                    if state.app.plugin_state.preset_state.confirm_remove_plugin == Some(idx) {
                                                                        state.app.plugin_state.preset_state.confirm_remove_plugin = None;
                                                                        state.app.remove_plugin(idx);
                                                                        state.app.update_level_meter_groups();
                                                                    } else {
                                                                        state.app.plugin_state.preset_state.confirm_remove_plugin = Some(idx);
                                                                    }
                                                                });
                                                                cx.notify();
                                                            },
                                                        ),
                                                    )
                                                    .child(if confirming { "?" } else { "X" })
                                                    .tooltip({
                                                        let label = if confirming { "Click again to confirm removal" } else { "Remove" };
                                                        move |_window, cx| make_tooltip(label, &theme_tt, cx)
                                                    }),
                                            )
                                        })
                                        .when(is_permanent, |d| {
                                            d.child(
                                                div()
                                                    .w(rems(1.125))
                                                    .h(rems(1.125))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(theme_c.text_muted)
                                                    .child(
                                                        Icon::new(IconName::Lock)
                                                            .small()
                                                            .color(theme_c.text_muted),
                                                    ),
                                            )
                                        }),
                                )
                                // Right content area
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .h_full()
                                        .child(
                                            div()
                                                .px(d.pad_y)
                                                .pt(d.pad_y_half)
                                                .text_size(d.text_xs)
                                                .text_color(if is_selected { color } else { theme_c.text_primary })
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .child(short_name_with_permanent(plugin_type, is_input_mon, is_output_mon, is_permanent)),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    Icon::new(icon)
                                                        .size(IconSize::Xxl)
                                                        .color(color),
                                                ),
                                        ),
                                )
                        },
                    ))
                    // Empty state
                    .when(is_empty, |d| {
                        d.child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.text_muted)
                                .child(text.empty_rack),
                        )
                    })
                    // Add plugin menu (shown when "+" is clicked). Deferred
                    // overlay keeps it above the rack/detail pane divider
                    // instead of clipping inside the fixed-height rack strip.
                    .when(show_add_plugin_menu, |rack| {
                        let menu = div()
                            .id("add-plugin-menu")
                            .absolute()
                            .top_full()
                            .left_0()
                            .right_0()
                            .mt(d.grid)
                            .px(d.card)
                            .py(d.card)
                            .max_h(rems(25.0))
                            .overflow_y_scroll()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.border)
                            .rounded(d.r_md)
                            .shadow_lg()
                            .occlude()
                            .child(self.render_add_plugin_buttons(cx));

                        rack.child(deferred(menu).with_priority(10))
                    })
                    )
                    .child({
                        let state_for_load = self.state.clone();
                        let state_for_save = self.state.clone();
                        let text_muted = theme.text_muted;
                        let surface_hover = theme.surface_hover;
                        let save_surface_hover = theme.surface_hover;
                        div()
                            .w(rems(10.75))
                            .h(rems(6.5))
                            .flex_none()
                            .flex()
                            .flex_col()
                            .items_end()
                            .justify_center()
                            .gap(d.grid)
                            .px(d.card)
                            .py(d.pad_x)
                            .text_color(theme.text_muted)
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .gap(d.grid)
                                    .child(
                                        div()
                                            .id("rack-load-preset")
                                            .cursor_pointer()
                                            .rounded(d.r_md)
                                            .px(d.pad_y)
                                            .py(d.pad_y_half)
                                            .text_size(d.text_xs)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(text_muted)
                                            .hover(move |s| s.bg(surface_hover))
                            .child(text.load)
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                state_for_load.update(cx, |state, _cx| {
                                                    state.app.refresh_plugin_presets();
                                                    state.app.input_state.plugin_file_input.clear();
                                                    state.app.ui_state.input_mode =
                                                        crate::app::InputMode::LoadPlugins;
                                                });
                                            }),
                                    )
                                    .child(div().text_size(d.text_xs).child("|"))
                                    .child(
                                        div()
                                            .id("rack-save-preset")
                                            .cursor_pointer()
                                            .rounded(d.r_md)
                                            .px(d.pad_y)
                                            .py(d.pad_y_half)
                                            .text_size(d.text_xs)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(text_muted)
                                            .hover(move |s| s.bg(save_surface_hover))
                            .child(text.save)
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                state_for_save.update(cx, |state, _cx| {
                                                    state.app.refresh_plugin_presets();
                                                    state.app.input_state.plugin_file_input.clear();
                                                    state.app.ui_state.input_mode =
                                                        crate::app::InputMode::SavePlugins;
                                                });
                                            }),
                                    ),
                            )
                            .when_some(last_loaded_preset.clone(), |el, name| {
                                el.child(
                                    div()
                                        .w_full()
                                        .text_align(TextAlign::Right)
                                        .text_size(d.text_xs)
                                        .child(name),
                                )
                            })
                            .child(
                                div()
                                    .w_full()
                                    .text_align(TextAlign::Right)
                                    .text_size(d.text_xs)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(chain_plugin_count.clone()),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .text_align(TextAlign::Right)
                                    .text_size(d.text_xs)
                                    .child(chain_channels.clone()),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .text_align(TextAlign::Right)
                                    .text_size(d.text_xs)
                                    .child(chain_clock.clone()),
                            )
                            .when(has_pending_update, |el| {
                                el.child(
                                    div()
                                        .w_full()
                                        .text_align(TextAlign::Right)
                                        .text_size(d.text_xs)
                                        .text_color(theme.accent)
                                        .font_weight(FontWeight::MEDIUM)
                                .child(text.applying),
                                )
                            })
                    }),
            )
    }

    /// Render a side level meter group for the detail panel
    /// Matches the style of the queue screen meters with vertical dB legend
    pub fn render_side_meter(
        &self,
        cx: &mut Context<Self>,
        channels: usize,
        label: &str,
        legend_on_left: bool,
        is_input: bool,
    ) -> impl IntoElement {
        let (theme, loudness) = {
            let state = self.state.read(cx);
            let loudness = if is_input {
                state.app.playback.input_loudness_info.clone()
            } else {
                state.app.playback.loudness_info.clone()
            };
            (state.app.ui_state.theme.clone(), loudness)
        };

        let theme_c = theme.clone();
        let label = label.to_string();
        let d = Ds::from_cx(cx);

        // Pre-compute channel data
        let channel_data: Vec<_> = (0..channels)
            .map(|i| {
                let val_db = if let Some(l) = &loudness {
                    let peak = l.channel_peaks.get(i).copied().unwrap_or(0.0);
                    if peak > 0.0001 {
                        20.0 * peak.log10()
                    } else {
                        -60.0
                    }
                } else {
                    -60.0
                };

                let fill_ratio = db_to_position(val_db);
                let yellow_threshold = db_to_position(-6.0);
                let red_threshold = db_to_position(-1.0);
                let name = match i {
                    0 => "L",
                    1 => "R",
                    2 => "C",
                    3 => "LFE",
                    4 => "Ls",
                    5 => "Rs",
                    6 => "Lb",
                    7 => "Rb",
                    8 => "Lw",
                    9 => "Rw",
                    10 => "Tfl",
                    11 => "Tfr",
                    12 => "Tml",
                    13 => "Tmr",
                    14 => "Tbl",
                    15 => "Tbr",
                    _ => ".",
                };
                (
                    fill_ratio,
                    yellow_threshold,
                    red_threshold,
                    name.to_string(),
                )
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .h_full()
            .py(d.card)
            .px(d.pad_y)
            .bg(theme.background_secondary)
            .border_x_1()
            .border_color(theme.border)
            // Label at top
            .child(
                div()
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_secondary)
                    .mb(d.gap)
                    .text_align(TextAlign::Center)
                    .child(label),
            )
            // Meters with legend
            .child(
                div()
                    .flex()
                    .gap(px(0.0))
                    .flex_1()
                    // Legend on left side if requested
                    .when(legend_on_left, |el| {
                        el.child(Self::render_side_meter_legend(&theme, false, d))
                    })
                    // Channel meters
                    .child(
                        div()
                            .flex()
                            // intentional: pixel-exact meter divider — do not scale
                            .gap(px(1.0))
                            .flex_1()
                            .p(d.half_grid)
                            .bg(theme_c.background_secondary)
                            .children(channel_data.into_iter().map(
                                |(fill_ratio, yellow_threshold, red_threshold, name)| {
                                    render_gradient_meter(
                                        &d,
                                        fill_ratio,
                                        yellow_threshold,
                                        red_threshold,
                                        None, // Side panel meters don't show peak hold
                                        name,
                                        &theme_c,
                                    )
                                },
                            )),
                    )
                    // Legend on right side if not on left
                    .when(!legend_on_left, |el| {
                        el.child(Self::render_side_meter_legend(&theme, true, d))
                    }),
            )
    }

    /// Render vertical dB legend for side meters (simplified version without M/S/D spacers)
    pub(super) fn render_side_meter_legend(
        theme: &crate::theme::Theme,
        align_right: bool,
        d: Ds,
    ) -> impl IntoElement {
        let ticks = [0, -6, -12, -18, -24, -30, -40, -50, -60];
        let theme = theme.clone();

        // Outer container matches render_gradient_meter structure
        div()
            .flex()
            .flex_col()
            .flex_1()
            .p(d.half_grid)
            // Ticks area (matches meter_bar flex_1)
            .child(
                div()
                    .relative()
                    .flex_1()
                    // Keep the legend width and edge offsets tied to the
                    // design scale so labels do not drift into the meter at
                    // large font zooms.
                    .w(rems(1.5))
                    .overflow_hidden()
                    .children(ticks.into_iter().map(move |db| {
                        let pos = db_to_position(db as f64);
                        // Use top positioning: top = (1 - pos), then offset by half line height
                        let top_fraction = 1.0 - pos;

                        // Adjust label offset for edge labels to keep them visible:
                        // - Top label (0 dB): move label down
                        // - Bottom label (-60 dB): move label up
                        // - Other labels: no additional offset
                        let label_offset = if db == 0 {
                            d.grid // Top: move label down
                        } else if db == -60 {
                            rems(-d.grid.0) // Bottom: move label up
                        } else {
                            rems(0.0) // No additional offset
                        };

                        let label = div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .mt(label_offset)
                            .child(format!("{}", db));

                        let tick = div().w(d.grid).h(d.half_grid).bg(theme.border);

                        let container = div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                top_fraction,
                            )))
                            // Offset by half line height (~6px for 9px text) to center tick on position
                            .mt(rems(-d.grid.0))
                            .flex()
                            .items_center()
                            .justify_between();

                        if align_right {
                            // Legend on right: tick → label (tick points toward meter on left)
                            container.child(tick).child(label)
                        } else {
                            // Legend on left: label → tick (tick points toward meter on right)
                            container.child(label).child(tick)
                        }
                    })),
            )
            // Spacer for channel name (to align with render_gradient_meter)
            .child(
                div()
                    .text_size(d.text_xs)
                    .mt(d.grid)
                    .opacity(0.0)
                    .child("X"), // Invisible spacer
            )
    }

    /// Render add plugin buttons grouped by category
    pub(super) fn render_add_plugin_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let release_channel = state.app.ui_state.release_channel;
        let common_text = PluginCommonTranslations::for_language(state.app.ui_state.language);
        let d = Ds::from_cx(cx);

        // Get list of plugins already in chain
        let present_plugins: Vec<_> = state
            .app
            .plugin_state
            .graph
            .plugins()
            .iter()
            .map(|p| p.plugin_type().clone())
            .collect();

        // Count LoudnessMonitor plugins (max 2 allowed: one for input, one for output)
        let monitor_count = present_plugins
            .iter()
            .filter(|p| matches!(p, PluginType::LoudnessMonitor))
            .count();

        // Categories sourced from `sotf_audio_player::plugin_categories` so
        // every player frontend (TUI, GPUI rack, GPUI rack-detail) renders
        // the same grouping.
        let categories = sotf_audio_player::plugin_categories::CATEGORIES;

        let mut category_rows: Vec<gpui::AnyElement> = Vec::new();

        for cat in categories {
            let cat_name = &cat.name;
            let plugins = cat.plugins;
            let mut buttons: Vec<gpui::AnyElement> = Vec::new();

            for pt in plugins {
                let pt = pt.clone();
                if !release_channel.allows(pt.maturity()) {
                    continue;
                }
                let is_single_instance =
                    matches!(pt, PluginType::Upmixer | PluginType::BinauralDecoder);
                if is_single_instance && present_plugins.contains(&pt) {
                    continue;
                }
                if matches!(pt, PluginType::LoudnessMonitor) && monitor_count >= 2 {
                    continue;
                }

                let color = plugin_color(&pt, &theme);
                // The add workflow favors comprehension over rack density.
                // Abbreviated names remain appropriate on compact rack cards.
                let name = pt.name();
                let description = plugin_description(&pt, common_text);
                let theme_c = theme.clone();
                let add_element_id = ElementId::from(SharedString::from(format!(
                    "rack-add-{}",
                    name.to_ascii_lowercase().replace([' ', '/'], "-")
                )));
                let add_accessibility = AriaProps::with_role(AriaRole::Button);
                cx.register_accessible(AccessibilityNode {
                    element_id: add_element_id.clone(),
                    label: name.into(),
                    props: add_accessibility,
                });
                #[cfg(feature = "dev-api")]
                let add_selector = format!(
                    "rack.add.{}",
                    name.to_ascii_lowercase().replace([' ', '/'], "-")
                );
                let state_for_add = self.state.clone();
                buttons.push(
                    div()
                        .id(SharedString::from(format!("rack-add-card-{name}")))
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap(d.grid)
                        .px(d.pad_y)
                        .py(d.pad_y_half)
                        .rounded(d.r_md)
                        .bg(theme_c.surface)
                        .border_1()
                        .border_color(color)
                        .text_size(d.text_xs)
                        .text_color(color)
                        .tooltip({
                            let theme = theme_c.clone();
                            move |_window, cx| make_tooltip(description, &theme, cx)
                        })
                        .child(dev_track!(
                            Button::new(add_element_id.clone(), name)
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .theme(theme_c.to_button_theme())
                                .on_click_event(move |_event, _window, cx| {
                                    state_for_add.update(cx, |state, cx| {
                                        state.app.add_plugin(&pt);
                                        state.app.update_level_meter_groups();
                                        state.app.plugin_ui.show_add_plugin_menu = false;
                                        cx.notify();
                                    });
                                }),
                            add_selector.clone()
                        ))
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(theme_c.text_muted)
                                .child(description),
                        )
                        .into_any_element(),
                );
            }

            if !buttons.is_empty() {
                category_rows.push(
                    div()
                        .flex()
                        .items_start()
                        .gap(d.gap)
                        // Category label
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_muted)
                                // intentional: fixed 60px label column width for category labels
                                .w(rems(3.75))
                                .flex_shrink_0()
                                // intentional: 3px top offset to align with adjacent buttons
                                .pt(px(3.0))
                                .child(*cat_name),
                        )
                        // Plugin buttons
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .flex_1()
                                .min_w_0()
                                .items_center()
                                .gap(d.gap)
                                .children(buttons),
                        )
                        .into_any_element(),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap(d.section)
            .children(category_rows)
    }

    /// Render the plugin detail/settings panel
    pub(super) fn render_plugin_detail_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let text = PluginRackTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let common_text =
            PluginCommonTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let (
            plugin_data,
            selected_idx,
            editing_idx,
            param_selection,
            is_input_monitor,
            is_output_monitor,
            theme,
        ) = {
            let state = self.state.read(cx);
            let plugin = state
                .app
                .plugin_state
                .graph
                .get_plugin(state.app.plugin_state.selected_plugin_index)
                .cloned();
            (
                plugin,
                state.app.plugin_state.selected_plugin_index,
                state.app.plugin_state.editing_plugin_index,
                state.app.plugin_state.plugin_param_selection,
                state
                    .app
                    .plugin_state
                    .graph
                    .is_input_monitor(state.app.plugin_state.selected_plugin_index),
                state
                    .app
                    .plugin_state
                    .graph
                    .is_output_monitor(state.app.plugin_state.selected_plugin_index),
                state.app.ui_state.theme.clone(),
            )
        };

        let has_plugin = plugin_data.is_some();
        let d = Ds::from_cx(cx);

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h(rems(21.875))
            .min_h_0()
            .when_some(plugin_data, |el, plugin| {
                let is_editing = editing_idx.is_some();
                el.child({
                    let state = self.state.read(cx);
                    let (min_meter_width, max_meter_width) =
                        output_meter_width_bounds(state);
                    let (_, output_channels) =
                        state.app.plugin_state.graph.compute_channel_flow();

                    let divider_theme = PaneDividerTheme {
                        background: theme.background,
                        background_hover: theme.surface_hover,
                        background_collapsed: theme.surface,
                        foreground: theme.text_muted,
                        foreground_hover: theme.text_secondary,
                        border: theme.border,
                        tint: Rgba {
                            a: 0.42,
                            ..theme.accent
                        },
                        tint_hover: theme.accent,
                    };

                    let output_collapsed = state.app.layout.output_meter_collapsed;
                    let config_open = state.app.plugin_state.plugin_ui_state.rack_config_overlay_open;
                    // If stored width is below minimum (e.g. channel count increased), snap to min
                    let output_meter_width = if state.app.layout.output_meter_width < min_meter_width {
                        min_meter_width
                    } else {
                        state.app.layout.output_meter_width.min(max_meter_width)
                    };
                    let plugin_bg = plugin_theme_id_for_app_theme(
                        state
                            .app
                            .plugin_state
                            .rack_theme_state
                            .resolved_id(selected_idx),
                        &theme,
                        state.app.ui_state.theme_id,
                    )
                    .theme()
                    .chassis_bg_top;

                    // Create state clones for divider callbacks
                    let state_for_output_toggle = self.state.clone();
                    let state_for_output_drag = self.state.clone();
                    let state_for_config = self.state.clone();
                    let gear_theme = theme.clone();
                    let output_meter_drag_start_width = output_meter_width;

                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .min_h(rems(18.75)) // Minimum height for meters and content
                        .min_h_0()
                        // Plugin Content (now takes full left area)
                        .child(
                            div()
                                .relative()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .id("params-scroll")
                                        .flex_1()
                                        .min_w_0()
                                        .min_h_0()
                                        .overflow_scroll()
                                        .bg(plugin_bg)
                                        .p(d.card)
                                        .child({
                                            // Get plugin-specific real-time data based on plugin type
                                            let plugin_data = self
                                                .state
                                                .read(cx)
                                                .app
                                                .playback
                                                .rack_plugin_data
                                                .clone();

                                    // Determine which loudness data to pass for LoudnessMonitor plugins
                                    // First monitor in chain gets input data, last gets output data
                                    let loudness_for_plugin = {
                                        let state = self.state.read(cx);
                                        if matches!(
                                            plugin.settings,
                                            sotf_audio_player::PluginSettings::LoudnessMonitor
                                        ) {
                                            // Find all LoudnessMonitor indices in the chain
                                            let monitor_indices: Vec<usize> = state
                                                .app
                                                .plugin_state.graph
                                                .plugins()
                                                .iter()
                                                .enumerate()
                                                .filter(|(_, p)| {
                                                    matches!(
                                                        p.settings,
                                                        sotf_audio_player::PluginSettings::LoudnessMonitor
                                                    )
                                                })
                                                .map(|(i, _)| i)
                                                .collect();

                                            if monitor_indices.len() <= 1 {
                                                // Only one monitor - use output (default behavior)
                                                state.app.playback.loudness_info.clone()
                                            } else if selected_idx == monitor_indices[0] {
                                                // First monitor - show input levels
                                                state.app.playback.input_loudness_info.clone()
                                            } else if monitor_indices
                                                .last()
                                                .is_some_and(|last| selected_idx == *last)
                                            {
                                                // Last monitor - show output levels
                                                state.app.playback.loudness_info.clone()
                                            } else {
                                                // Middle monitor - show output (best available approximation)
                                                state.app.playback.loudness_info.clone()
                                            }
                                        } else {
                                            // Non-monitor plugins use output loudness
                                            state.app.playback.loudness_info.clone()
                                        }
                                    };

                                    let app_st = self.state.read(cx);
                                    let upmixer_config_open = app_st.app.plugin_ui.upmixer_config_open;
                                    let selected_eq_band = app_st.app.plugin_state.selected_eq_band;
                                    let spectrum_tilt_open = app_st.app.plugin_ui.spectrum_tilt_select_open;
                                    let spectrum_ref_open = app_st.app.plugin_ui.spectrum_reference_select_open;
                                    let plugin_graph = app_st.app.plugin_state.graph.clone();
                                    let midi_overlay = app_st.app.plugin_state.midi_mapping.build_overlay(&[]);
                                    let plugin_ui_view = app_st.app.plugin_state.plugin_ui_state.plugin_ui_view.clone();

                                    let midi_ref = if midi_overlay.has_controller() {
                                        Some(midi_overlay)
                                    } else {
                                        None
                                    };

                                    let d = Ds::from_cx(cx);

                                    // Resolve the chassis theme once for the
                                    // Simple/Controller branches so all four
                                    // views share the same theming.
                                    let chassis = plugin_theme_id_for_app_theme(
                                        app_st
                                            .app
                                            .plugin_state
                                            .rack_theme_state
                                            .resolved_id(selected_idx),
                                        &theme,
                                        app_st.app.ui_state.theme_id,
                                    )
                                    .theme()
                                    .apply_to(&theme);

                                            match &plugin_ui_view {
                                                PluginUiView::Simple => {
                                                    super::super::render_app_plugin_shell(
                                                        &d,
                                                        self.state.clone(),
                                                        selected_idx,
                                                        &plugin.plugin_type(),
                                                        is_input_monitor,
                                                        is_output_monitor,
                                                        plugin.enabled,
                                                    common_text,
                                                    &chassis,
                                                        super::super::ui_simple::render_simple_plugin_view(
                                                            &d,
                                                            self.state.clone(),
                                                            selected_idx,
                                                            &plugin.settings,
                                                            is_editing,
                                                            param_selection,
                                                            &chassis,
                                                            midi_ref.as_ref(),
                                                            common_text,
                                                        ),
                                                    )
                                                }
                                                PluginUiView::Controller(controller_id) => {
                                                    super::super::render_app_plugin_shell(
                                                        &d,
                                                        self.state.clone(),
                                                        selected_idx,
                                                        &plugin.plugin_type(),
                                                        is_input_monitor,
                                                        is_output_monitor,
                                                        plugin.enabled,
                                                    common_text,
                                                    &chassis,
                                                        super::super::render_controller_view(
                                                            &d,
                                                            controller_id,
                                                            &plugin.settings,
                                                            selected_idx,
                                                            &app_st.app.plugin_state.midi_mapping,
                                                            self.state.clone(),
                                                            is_editing,
                                                            param_selection,
                                                            app_st.app.ui_state.window_width,
                                                            crate::ui::compute_combined_scale(
                                                                app_st.app.ui_state.window_width,
                                                                app_st.app.ui_state.window_height,
                                                                app_st.app.ui_state.font_scale,
                                                                app_st.app.ui_state.min_font_size_px,
                                                                app_st.app.ui_state.max_font_size_px,
                                                            ),
                                                            &chassis,
                                                        ),
                                                    )
                                                }
                                                PluginUiView::UI => {
                                                    render_plugin_content(
                                                        self.state.clone(),
                                                        selected_idx,
                                                        &plugin.settings,
                                                        is_editing,
                                                        param_selection,
                                                        &theme,
                                                        upmixer_config_open,
                                                        selected_eq_band,
                                                        loudness_for_plugin,
                                                        plugin_data,
                                                        spectrum_tilt_open,
                                                        spectrum_ref_open,
                                                        &plugin_graph,
                                                        midi_ref.as_ref(),
                                                        self.eq_chart_focus_handle.clone(),
                                                        cx,
                                                    )
                                                }
                                            }
                                        }),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top(d.pad_y)
                                        .right(d.pad_y)
                                        .child(
                                            IconButton::with_child(
                                                "rack-plugin-config",
                                                Icon::new(IconName::Settings)
                                                    .size(IconSize::Sm)
                                                    .color(gear_theme.text_primary),
                                            )
                                            .variant(IconButtonVariant::Filled)
                                            .size(IconButtonSize::Sm)
                                            .theme(gear_theme.to_icon_button_theme())
                                            .aria_label(text.plugin_configuration)
                                            .on_click_event(move |_event, _window, cx| {
                                                state_for_config.update(cx, |state, _cx| {
                                                    let open = &mut state
                                                        .app
                                                        .plugin_state
                                                        .plugin_ui_state.rack_config_overlay_open;
                                                    *open = !*open;
                                                });
                                            }),
                                        ),
                                )
                                .when(config_open, |el| {
                                    el.child(self.render_rack_config_overlay(cx))
                                }),
                        )
                        // Divider 2: Between main zone and output meter (always shown)
                        .child(
                            PaneDivider::vertical("output-meter-divider", CollapseDirection::Right)
                                .label("OUT")
                                .theme(divider_theme.clone())
                                .thickness(px(6.0))
                                .collapsed(output_collapsed)
                                .on_toggle(move |collapsed, _window, cx| {
                                    state_for_output_toggle.update(cx, |s, _| {
                                        s.app.layout.output_meter_collapsed = collapsed;
                                    });
                                })
                                .on_drag_start(move |pos, _window, cx| {
                                    state_for_output_drag.update(cx, |s, _| {
                                        s.app.layout.dragging_divider = Some(DividerDragState {
                                            divider_type: DividerType::OutputMeter,
                                            start_x: pos,
                                            start_width: output_meter_drag_start_width,
                                        });
                                    });
                                }),
                        )
                        // Right: Combined IN/OUT Meter + Chain Controls
                        .when(!output_collapsed, |el| {
                            el.child(
                                div()
                                    .w(px(output_meter_width))
                                    .h_full()
                                    .relative()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .child(self.render_combined_meter(cx, 2, output_channels))
                                    .child(self.render_chain_controls(cx)),
                            )
                        })
                })
            })
            .when(!has_plugin, |el| {
                el.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_col()
                        .gap(d.gap)
                        .text_color(theme.text_muted)
                        .child(text.no_plugin_selected)
                        .child(div().text_size(d.text_sm).child(text.add_plugin_to_start)),
                )
            })
    }

    /// Gear popover for plugin setup. Keeps the plugin body to two visible
    /// zones (`Main | Output`) while still exposing view, skin, and setup
    /// controls on demand.
    pub(super) fn render_rack_config_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = PluginRackTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let common_text =
            PluginCommonTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let (
            plugin_data,
            selected_idx,
            editing_idx,
            param_selection,
            plugin_ui_view,
            controller_picker_open,
            skin_picker_open,
            upmixer_config_open,
            theme,
            plugin_theme_id,
            plugin_theme,
        ) = {
            let state = self.state.read(cx);
            let selected_idx = state.app.plugin_state.selected_plugin_index;
            let plugin = state
                .app
                .plugin_state
                .graph
                .get_plugin(selected_idx)
                .cloned();
            (
                plugin,
                selected_idx,
                state.app.plugin_state.editing_plugin_index,
                state.app.plugin_state.plugin_param_selection,
                state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .plugin_ui_view
                    .clone(),
                state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .controller_picker_open,
                state.app.plugin_state.plugin_ui_state.rack_skin_picker_open,
                state.app.plugin_ui.upmixer_config_open,
                state.app.ui_state.theme.clone(),
                state
                    .app
                    .plugin_state
                    .rack_theme_state
                    .resolved_id(selected_idx),
                plugin_theme_id_for_app_theme(
                    state
                        .app
                        .plugin_state
                        .rack_theme_state
                        .resolved_id(selected_idx),
                    &state.app.ui_state.theme,
                    state.app.ui_state.theme_id,
                )
                .theme(),
            )
        };

        let Some(plugin) = plugin_data else {
            return div().into_any_element();
        };

        let plugin_type = plugin.plugin_type().clone();
        let state_for_view = self.state.clone();
        let state_for_skin = self.state.clone();
        let mut view_options: Vec<SelectOption> = vec![SelectOption::new(
            "ui".to_string(),
            text.native_ui.to_string(),
        )];
        for (id, label) in available_controllers() {
            view_options.push(SelectOption::new(format!("ctrl:{id}"), label.to_string()));
        }
        view_options.push(SelectOption::new(
            "simple".to_string(),
            text.simple.to_string(),
        ));

        let selected_value = match &plugin_ui_view {
            PluginUiView::UI => "ui".to_string(),
            PluginUiView::Simple => "simple".to_string(),
            PluginUiView::Controller(name) => format!("ctrl:{name}"),
        };
        let skin_options: Vec<SelectOption> = PluginThemeId::all()
            .iter()
            .map(|theme_id| {
                SelectOption::new(
                    plugin_theme_select_value(*theme_id).to_string(),
                    theme_id.name().to_string(),
                )
            })
            .collect();
        let selected_skin_value = plugin_theme_select_value(plugin_theme_id).to_string();

        let live_plugin_data = self.state.read(cx).app.playback.rack_plugin_data.clone();
        let (layout_scale, window_width) = {
            let state = self.state.read(cx);
            (
                crate::ui::compute_combined_scale(
                    state.app.ui_state.window_width,
                    state.app.ui_state.window_height,
                    state.app.ui_state.font_scale,
                    state.app.ui_state.min_font_size_px,
                    state.app.ui_state.max_font_size_px,
                ),
                state.app.ui_state.window_width,
            )
        };
        let config_content_width =
            super::super::ui_layout_renderer::config_controls_preferred_width(
                &plugin.settings,
                layout_scale,
            );
        let overlay_width = (config_content_width + 32.0 * layout_scale)
            .min((window_width - 32.0 * layout_scale).max(1.0));
        let generated_config = super::super::ui_layout_renderer::render_config_controls_from_layout(
            &d,
            self.state.clone(),
            selected_idx,
            &plugin.settings,
            editing_idx.is_some(),
            param_selection,
            config_content_width,
            layout_scale,
            live_plugin_data.as_ref(),
            common_text,
            &theme,
            &plugin_theme,
        );

        div()
            .id("rack-config-overlay")
            .absolute()
            .top(rems(2.5))
            .right(d.pad_y)
            .w(px(overlay_width))
            .max_h(rems(30.0))
            .overflow_y_scroll()
            .occlude()
            .flex()
            .flex_col()
            .gap(d.section)
            .p(d.card)
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded(d.r_md)
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child(text.configuration),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(short_name(&plugin.plugin_type(), false, false)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_secondary)
                            .child(text.view),
                    )
                    .child(
                        Select::new("rack-config-view-mode")
                            .options(view_options)
                            .selected(selected_value)
                            .is_open(controller_picker_open)
                            .size(SelectSize::Xs)
                            .theme(theme.to_select_theme())
                            .on_toggle({
                                let state_for_view = state_for_view.clone();
                                move |is_open, _window, cx| {
                                    state_for_view.update(cx, |state, cx| {
                                        state
                                            .app
                                            .plugin_state
                                            .plugin_ui_state
                                            .controller_picker_open = is_open;
                                        cx.notify();
                                    });
                                }
                            })
                            .on_change({
                                move |value, _window, cx| {
                                    let view = match value.as_ref() {
                                        "ui" => PluginUiView::UI,
                                        "simple" => PluginUiView::Simple,
                                        v if v.starts_with("ctrl:") => {
                                            PluginUiView::Controller(v[5..].to_string())
                                        }
                                        _ => PluginUiView::UI,
                                    };
                                    state_for_view.update(cx, |state, _cx| {
                                        state.app.plugin_state.plugin_ui_state.plugin_ui_view =
                                            view;
                                        state
                                            .app
                                            .plugin_state
                                            .plugin_ui_state
                                            .controller_picker_open = false;
                                    });
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_secondary)
                            .child(text.skin),
                    )
                    .child(
                        Select::new("rack-config-skin")
                            .options(skin_options)
                            .selected(selected_skin_value)
                            .is_open(skin_picker_open)
                            .size(SelectSize::Xs)
                            .theme(theme.to_select_theme())
                            .on_toggle({
                                let state_for_skin = state_for_skin.clone();
                                move |is_open, _window, cx| {
                                    state_for_skin.update(cx, |state, cx| {
                                        state
                                            .app
                                            .plugin_state
                                            .plugin_ui_state
                                            .rack_skin_picker_open = is_open;
                                        cx.notify();
                                    });
                                }
                            })
                            .on_change(move |value, _window, cx| {
                                let selected_theme = plugin_theme_from_select_value(value.as_ref());
                                state_for_skin.update(cx, |state, cx| {
                                    let rack_theme =
                                        state.app.plugin_state.rack_theme_state.rack_theme;
                                    if selected_theme == rack_theme {
                                        state
                                            .app
                                            .plugin_state
                                            .rack_theme_state
                                            .clear_override(selected_idx);
                                    } else {
                                        state
                                            .app
                                            .plugin_state
                                            .rack_theme_state
                                            .set_override(selected_idx, selected_theme);
                                    }
                                    state.app.plugin_state.plugin_ui_state.rack_skin_picker_open =
                                        false;
                                    let layout = state.layout.read(cx);
                                    if let Err(e) = state.app.save_config(layout) {
                                        log::error!("Failed to save plugin skin: {}", e);
                                    }
                                });
                            }),
                    ),
            )
            .when(
                matches!(plugin_type, PluginType::Upmixer),
                |el: Stateful<Div>| {
                    let state_for_output = self.state.clone();
                    let state_for_binaural = self.state.clone();
                    let labels = pk(UP, "speaker_config").choice_labels();
                    let output_options: Vec<SelectOption> = labels
                        .iter()
                        .map(|label| SelectOption::new(label.to_string(), label.to_string()))
                        .collect();
                    let (speaker_config, binaural_preview) = match &plugin.settings {
                        sotf_audio_player::PluginSettings::Upmixer {
                            speaker_config,
                            output:
                                UpmixerOutputSettings {
                                    binaural_preview, ..
                                },
                            ..
                        } => (speaker_config.clone(), *binaural_preview),
                        _ => ("5.1".to_string(), false),
                    };
                    let binaural_idx = index_of(UP, "binaural_preview");
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.gap)
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_secondary)
                                    .child(text.output),
                            )
                            .child(
                                Select::new("rack-config-upmixer-output")
                                    .options(output_options)
                                    .selected(speaker_config)
                                    .is_open(upmixer_config_open)
                                    .size(SelectSize::Xs)
                                    .theme(theme.to_select_theme())
                                    .on_toggle({
                                        let state_for_output = state_for_output.clone();
                                        move |is_open, _window, cx| {
                                            state_for_output.update(cx, |state, cx| {
                                                state.app.plugin_ui.upmixer_config_open = is_open;
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .on_change(move |value, _window, cx| {
                                        let idx = labels
                                            .iter()
                                            .position(|label| *label == value.as_ref())
                                            .unwrap_or(0);
                                        state_for_output.update(cx, |state, _cx| {
                                            state.app.set_plugin_param(selected_idx, 0, idx as f64);
                                            state.app.plugin_ui.upmixer_config_open = false;
                                            state.app.update_level_meter_groups();
                                        });
                                    }),
                            )
                            .child(
                                Toggle::new("rack-config-binaural-preview")
                                    .size(ToggleSize::Sm)
                                    .checked(binaural_preview)
                                    .label(text.binaural_preview)
                                    .style(ToggleStyle::Segmented)
                                    .theme(theme.to_toggle_theme())
                                    .on_change(move |enabled, _window, cx| {
                                        state_for_binaural.update(cx, |state, _cx| {
                                            state.app.set_plugin_param(
                                                selected_idx,
                                                binaural_idx,
                                                if enabled { 1.0 } else { 0.0 },
                                            );
                                            state.app.update_level_meter_groups();
                                        });
                                    }),
                            ),
                    )
                },
            )
            .when_some(generated_config, |el: Stateful<Div>, config| {
                el.child(config)
            })
            .into_any_element()
    }

    /// Render combined IN + OUT meter panel using the same meter components as the main library view.
    /// Uses proper speaker-config-aware channel groups (L/R, Center, LFE, Surrounds, etc.)
    /// instead of a single flat group for all channels.
    pub(super) fn render_combined_meter(
        &self,
        cx: &mut Context<Self>,
        _input_channels: usize,
        _output_channels: usize,
    ) -> impl IntoElement {
        use sotf_audio_player::{ChannelGroup, ChannelInfo};

        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let input_loudness = state.app.playback.input_loudness_info.clone();
        let output_loudness = state.app.playback.loudness_info.clone();
        let peak_hold = state.app.level_meters.peak_hold.clone();

        // Build input groups (always stereo L/R)
        let in_groups = [ChannelGroup {
            name: "IN".to_string(),
            channels: vec![
                ChannelInfo {
                    index: 0,
                    name: "L".to_string(),
                    display_name: vec!["L".to_string()],
                },
                ChannelInfo {
                    index: 1,
                    name: "R".to_string(),
                    display_name: vec!["R".to_string()],
                },
            ],
            muted: false,
            soloed: false,
            dimmed: false,
        }];

        // Use the app's canonical level_meter_groups for output (M/S/D state is preserved)
        let out_groups = state.app.level_meters.groups.clone();

        // Fake index for input (no M/S/D control on input)
        let fake_in_idx = 1000;

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(theme.background_secondary)
            .border_l_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_stretch()
                    .justify_center()
                    .gap_0()
                    .flex_1()
                    .w_full()
                    .min_h(rems(18.75))
                    .child(
                        self.render_vertical_legend(&d, &theme, false)
                            .into_any_element(),
                    )
                    // Input group (stereo L/R, no M/S/D)
                    .children(in_groups.iter().enumerate().map(|(i, group)| {
                        self.render_meter_group(
                            group,
                            fake_in_idx + i,
                            false,
                            input_loudness.as_deref(),
                            &peak_hold,
                            &theme,
                            cx,
                        )
                        .into_any_element()
                    }))
                    // Output groups (real indices → M/S/D connected to matrix plugin)
                    .children(out_groups.iter().enumerate().map(|(i, group)| {
                        self.render_meter_group(
                            group,
                            i, // real index into level_meter_groups
                            false,
                            output_loudness.as_deref(),
                            &peak_hold,
                            &theme,
                            cx,
                        )
                        .into_any_element()
                    }))
                    .child(
                        self.render_vertical_legend(&d, &theme, true)
                            .into_any_element(),
                    ),
            )
    }

    /// Render chain-level control buttons (Bypass, AutoGain, Mono, M/S) in a 2×2 grid
    pub(crate) fn render_chain_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let chain_bypass = state.app.plugin_state.chain_state.chain_bypass;
        let chain_autogain = state.app.plugin_state.chain_state.chain_autogain;
        let d = Ds::from_cx(cx);

        // Detect current matrix preset for Mono/M/S button states
        let (is_mono, is_ms) = {
            let mut mono = false;
            let mut ms = false;
            for plugin in state.app.plugin_state.graph.plugins() {
                if plugin.is_permanent() && matches!(plugin.plugin_type(), PluginType::Matrix) {
                    if let sotf_audio_player::PluginSettings::Matrix {
                        input_channels,
                        output_channels,
                        ref matrix,
                        ..
                    } = plugin.settings
                    {
                        let preset = sotf_audio_player::detect_matrix_preset(
                            input_channels,
                            output_channels,
                            matrix,
                        );
                        mono = preset == "Mono Mix";
                        ms = preset == "M/S Encode" || preset == "M/S Decode";
                    }
                    break;
                }
            }
            (mono, ms)
        };

        let active_bg = theme.accent;
        let active_text = theme.text_on_accent;
        let inactive_bg = theme.surface;
        let inactive_text = theme.text_muted;
        let hover_bg = theme.surface_hover;
        let border = theme.border;

        let make_button = |id: &'static str,
                           label: &'static str,
                           is_active: bool,
                           active_bg: Rgba,
                           active_text: Rgba,
                           inactive_bg: Rgba,
                           inactive_text: Rgba,
                           hover_bg: Rgba|
         -> Stateful<Div> {
            div()
                .id(id)
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .py(d.grid)
                .cursor_pointer()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .rounded(d.r_sm)
                .when(is_active, |el| el.bg(active_bg).text_color(active_text))
                .when(!is_active, |el| {
                    el.bg(inactive_bg)
                        .text_color(inactive_text)
                        .hover(move |s| s.bg(hover_bg))
                })
                .child(label)
        };

        div()
            .flex()
            .flex_col()
            .gap(d.half_grid)
            .p(d.pad_y)
            .bg(theme.background_secondary)
            .border_l_1()
            .border_t_1()
            .border_color(border)
            // Row 1: Bypass | AutoGain
            .child(
                div()
                    .flex()
                    .gap(d.half_grid)
                    .child(
                        make_button(
                            "chain-bypass",
                            "Bypass",
                            chain_bypass,
                            active_bg,
                            active_text,
                            inactive_bg,
                            inactive_text,
                            hover_bg,
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.toggle_chain_bypass();
                                    state.app.update_level_meter_groups();
                                });
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        make_button(
                            "chain-autogain",
                            "AutoGain",
                            chain_autogain,
                            active_bg,
                            active_text,
                            inactive_bg,
                            inactive_text,
                            hover_bg,
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.toggle_chain_autogain();
                                });
                                cx.notify();
                            }),
                        ),
                    ),
            )
            // Row 2: Mono | M/S
            .child(
                div()
                    .flex()
                    .gap(d.half_grid)
                    .child(
                        make_button(
                            "chain-mono",
                            "Mono",
                            is_mono,
                            active_bg,
                            active_text,
                            inactive_bg,
                            inactive_text,
                            hover_bg,
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.apply_matrix_mono();
                                });
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        make_button(
                            "chain-ms",
                            "M/S",
                            is_ms,
                            active_bg,
                            active_text,
                            inactive_bg,
                            inactive_text,
                            hover_bg,
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.apply_matrix_ms();
                                });
                                cx.notify();
                            }),
                        ),
                    ),
            )
    }

    pub(super) fn toggle_upmixer_config(
        &mut self,
        action: &ToggleUpmixerConfig,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.plugin_ui.upmixer_config_open = action.open;
        });
        cx.notify();
    }
}
