//! Plugin screen rendering functions - Professional DAW-style interface

use super::actions::ToggleUpmixerConfig;
use super::level_meters::{db_to_position, render_gradient_meter};
use super::render_plugin_content;
use crate::app::state::plugin::{PluginUiView, available_controllers};
use crate::app::state::{DividerDragState, DividerType};
use crate::app::types::{PluginUpdateType, Screen};
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;
use crate::theme::Theme;

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui::{MouseMoveEvent, MouseUpEvent};
use gpui_ui_kit::{CollapseDirection, PaneDivider, PaneDividerTheme};
use sotf_audio_player::PluginType;
use sotf_audio_player_midi::mapping::MidiOverlay;
use sotf_audio_player_midi::MidiMappingEngine;

/// Drag information for plugin reordering
#[derive(Clone)]
pub struct PluginDragInfo {
    pub source_index: usize,
    pub name: String,
    pub color: Rgba,
    pub icon: &'static str,
    pub surface: Rgba,
    pub text_on_accent: Rgba,
}

impl Render for PluginDragInfo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Drag preview — matches the Ozone-style card
        div()
            .w(rems(7.0))
            .h(rems(4.0))
            .flex()
            .flex_row()
            .rounded_md()
            .border_1()
            .border_color(self.color)
            .bg(Theme::opacity_20pct(self.surface))
            .shadow_lg()
            .opacity(0.85)
            // Left color bar
            .child(div().w(px(3.0)).h_full().bg(self.color).rounded_l_md())
            // Content
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .px_2()
                            .pt_1()
                            .text_xs()
                            .text_color(self.color)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xl()
                            .text_color(self.color)
                            .child(self.icon),
                    ),
            )
    }
}

// Plugin color scheme for different types - uses theme colors
fn plugin_color(plugin_type: &PluginType, theme: &crate::theme::Theme) -> Rgba {
    match plugin_type {
        PluginType::EQ => theme.plugin_colors.eq,
        PluginType::Gain => theme.plugin_colors.gain,
        PluginType::Upmixer => theme.plugin_colors.upmixer,
        PluginType::Compressor => theme.plugin_colors.compressor,
        PluginType::Limiter => theme.plugin_colors.limiter,
        PluginType::Gate => theme.plugin_colors.gate,
        PluginType::Expander => theme.plugin_colors.gate, // Reuse gate color for expander
        PluginType::MultibandCompressor => theme.plugin_colors.compressor, // Reuse compressor color
        PluginType::MultibandExpander => theme.plugin_colors.gate, // Reuse gate color
        PluginType::LoudnessCompensation => theme.plugin_colors.loudness,
        PluginType::FletcherMunson => theme.plugin_colors.loudness, // Reuse loudness color
        PluginType::BinauralDecoder => theme.plugin_colors.binaural,
        PluginType::Convolution => theme.plugin_colors.convolution,
        PluginType::LoudnessMonitor => theme.plugin_colors.monitor,
        PluginType::SpectrumAnalyzer => theme.plugin_colors.spectrum,
        PluginType::ChannelMuteSolo => theme.plugin_colors.mute_solo,
        PluginType::Matrix => theme.plugin_colors.upmixer, // Reuse upmixer color for matrix
        PluginType::XTC => theme.plugin_colors.binaural,   // Reuse binaural color for XTC
        PluginType::Denoiser => theme.plugin_colors.eq,    // Reuse eq color for denoiser
        PluginType::Pnd => theme.plugin_colors.eq,         // Reuse eq color for pnd
        PluginType::ABCompare => theme.plugin_colors.compressor, // A/B Compare - use compressor color
        PluginType::BandSplit => theme.plugin_colors.upmixer, // Reuse upmixer color for band split
        PluginType::BandMerge => theme.plugin_colors.upmixer, // Reuse upmixer color for band merge
        PluginType::Downmix => theme.plugin_colors.upmixer,   // Reuse upmixer color for downmix
        PluginType::MonoToStereo => theme.plugin_colors.binaural, // Reuse binaural color for mono to stereo
        PluginType::Crossfeed => theme.plugin_colors.binaural, // Reuse binaural color for crossfeed
    }
}

fn plugin_icon(plugin_type: &PluginType, is_input_mon: bool, is_output_mon: bool) -> &'static str {
    match plugin_type {
        PluginType::EQ => "≈",
        PluginType::Gain => "▲",
        PluginType::Upmixer => "◈",
        PluginType::Compressor => "◉",
        PluginType::Limiter => "█",
        PluginType::Gate => "⊡",
        PluginType::Expander => "⊟",
        PluginType::MultibandCompressor => "◎",
        PluginType::MultibandExpander => "◇",
        PluginType::LoudnessCompensation => "♫",
        PluginType::FletcherMunson => "🎧",
        PluginType::BinauralDecoder => "◎",
        PluginType::Convolution => "∿",
        PluginType::LoudnessMonitor => {
            if is_input_mon {
                "◁" // Input monitor - left-pointing triangle
            } else if is_output_mon {
                "▷" // Output monitor - right-pointing triangle
            } else {
                "◐" // User-added monitor
            }
        }
        PluginType::SpectrumAnalyzer => "▓",
        PluginType::ChannelMuteSolo => "◧",
        PluginType::Matrix => "⊞",
        PluginType::XTC => "⊗",
        PluginType::Denoiser => "◌",
        PluginType::Pnd => "♪",
        PluginType::ABCompare => "⇄", // A/B Compare - bidirectional arrow
        PluginType::BandSplit => "⊥", // Split - T-junction symbol
        PluginType::BandMerge => "⊤", // Merge - inverted T-junction
        PluginType::Downmix => "▼",   // Downmix - downward triangle
        PluginType::MonoToStereo => "⊕", // Mono to Stereo - circular plus
        PluginType::Crossfeed => "⊞", // Crossfeed - boxed plus
    }
}

fn short_name(plugin_type: &PluginType, is_input_mon: bool, is_output_mon: bool) -> &'static str {
    match plugin_type {
        PluginType::EQ => "Equalizer",
        PluginType::Gain => "Gain",
        PluginType::Upmixer => "Upmixer",
        PluginType::Compressor => "Compressor",
        PluginType::Limiter => "Limiter",
        PluginType::Gate => "Gate",
        PluginType::Expander => "Expander",
        PluginType::MultibandCompressor => "MB Comp",
        PluginType::MultibandExpander => "MB Expand",
        PluginType::LoudnessCompensation => "Loudness",
        PluginType::FletcherMunson => "F-M EQ",
        PluginType::BinauralDecoder => "Binaural",
        PluginType::Convolution => "Convolution",
        PluginType::LoudnessMonitor => {
            if is_input_mon {
                "In Monitor"
            } else if is_output_mon {
                "Out Monitor"
            } else {
                "Monitor"
            }
        }
        PluginType::SpectrumAnalyzer => "Spectrum",
        PluginType::ChannelMuteSolo => "Mixer",
        PluginType::Matrix => "Matrix",
        PluginType::XTC => "XTC",
        PluginType::Denoiser => "Denoiser",
        PluginType::Pnd => "PND",
        PluginType::ABCompare => "A/B Comp",
        PluginType::BandSplit => "Split",
        PluginType::BandMerge => "Merge",
        PluginType::Downmix => "Downmix",
        PluginType::MonoToStereo => "Mono->2.0",
        PluginType::Crossfeed => "Crossfeed",
    }
}

/// Convert speaker config string to output channel count
fn speaker_config_to_channels(config: &str) -> usize {
    match config {
        "2.0" => 2,
        "5.0" => 5,
        "5.1" => 6,
        "7.1" => 8,
        "5.1.2" => 8,  // 5.1 + 2 height
        "5.1.4" => 10, // 5.1 + 4 height
        "7.1.2" => 10, // 7.1 + 2 height
        "7.1.4" => 12, // 7.1 + 4 height
        "9.1.4" => 14, // 9.1 + 4 height
        "9.1.6" => 16, // 9.1 + 6 height
        _ => 6,        // Default to 5.1 if unknown
    }
}

impl PlayerView {
    pub(crate) fn render_plugins_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();

        div()
            .id("plugins-screen")
            .key_context("PluginRack")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .on_action(cx.listener(Self::toggle_upmixer_config))
            // Plugin parameter actions - needed for knob/slider interaction
            .on_action(cx.listener(Self::on_update_plugin_param))
            .on_action(cx.listener(Self::on_select_plugin_param))
            .on_action(cx.listener(Self::on_reset_plugin_param))
            .on_action(cx.listener(Self::on_start_knob_drag))
            // Global mouse move handler for knob/slider and divider dragging
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, _window, cx| {
                let state_read = view.state.read(cx);

                // Handle knob/slider dragging
                let (is_knob_dragging, start_y, start_value, min, max, plugin_idx, param_idx) = (
                    state_read.app.is_dragging_knob,
                    state_read.app.knob_drag_start_y,
                    state_read.app.knob_drag_start_value,
                    state_read.app.knob_drag_min,
                    state_read.app.knob_drag_max,
                    state_read.app.knob_drag_plugin_idx,
                    state_read.app.knob_drag_param_idx,
                );

                // Handle divider dragging
                let divider_drag = state_read.app.dragging_divider.clone();

                if is_knob_dragging {
                    if let Some(start_y) = start_y {
                        let mouse_y: f32 = event.position.y.into();
                        let delta_y = start_y - mouse_y; // Inverted: up = positive (increase)
                        // Scale: 150px drag = full range
                        let range = max - min;
                        let value_delta = (delta_y as f64 / 150.0) * range;
                        let new_value = (start_value + value_delta).clamp(min, max);

                        // Update the parameter value via the plugin editing system
                        // (set_plugin_param already sets pending_plugin_update)
                        view.state.update(cx, |state, _cx| {
                            state.app.set_plugin_param(plugin_idx, param_idx, new_value);
                        });
                        cx.notify();
                    }
                } else if let Some(drag) = divider_drag {
                    let mouse_x: f32 = event.position.x.into();
                    let delta_x = mouse_x - drag.start_x;

                    view.state.update(cx, |state, _cx| {
                        match drag.divider_type {
                            DividerType::InputMeter => {
                                // Dragging right increases input meter width
                                let new_width = (drag.start_width + delta_x).clamp(60.0, 200.0);
                                state.app.input_meter_width = new_width;
                            }
                            DividerType::OutputMeter => {
                                // Dragging left increases output meter width
                                let new_width = (drag.start_width - delta_x).clamp(60.0, 300.0);
                                state.app.output_meter_width = new_width;
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
                    view.state.update(cx, |state, _cx| {
                        if state.app.is_dragging_knob {
                            state.app.is_dragging_knob = false;
                            state.app.knob_drag_start_y = None;
                        }
                        if state.app.dragging_divider.is_some() {
                            state.app.dragging_divider = None;
                        }
                    });
                }),
            )
            // Plugin Rack Strip (top) - only show if not collapsed
            .when(!self.state.read(cx).app.rack_detail_collapsed, |d| {
                d.child(self.render_plugin_rack(cx))
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
                };
                let state = self.state.clone();
                let is_collapsed = self.state.read(cx).app.rack_detail_collapsed;
                PaneDivider::horizontal("rack-detail-divider", CollapseDirection::Up)
                    .label("Signal Chain")
                    .theme(divider_theme)
                    .thickness(px(4.0))
                    .collapsed(is_collapsed)
                    .on_toggle(move |collapsed, _window, cx| {
                        state.update(cx, |s, _| {
                            s.app.rack_detail_collapsed = collapsed;
                        });
                    })
            })
            // Parameter Panel (bottom, fills remaining space)
            .child(self.render_plugin_detail_panel(cx))
    }

    /// Render the horizontal plugin rack strip
    fn render_plugin_rack(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (plugins_data, selected_idx, theme) = {
            let state = self.state.read(cx);
            let chain = &state.app.plugin_state.chain;
            let plugins: Vec<_> = chain
                .plugins()
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        p.plugin_type().clone(),
                        p.enabled,
                        p.plugin_type().name().to_string(),
                        p.is_permanent(),
                        chain.is_input_monitor(i),
                        chain.is_output_monitor(i),
                    )
                })
                .collect();
            (
                plugins,
                state.app.plugin_state.selected_plugin_index,
                state.app.ui_state.theme.clone(),
            )
        };

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
        let plugin_count = plugins_data.len();

        div()
            .flex()
            .flex_col()
            .bg(theme.background_secondary)
            .border_color(theme.border)
            // Header
            .child({
                let state_for_home = self.state.clone();
                let text_muted = theme.text_muted;
                let surface_hover = theme.surface_hover;

                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px_4()
                    .py_2()
                    // Home button on the left
                    .child(
                        div()
                            .id("rack-home-button")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(rems(2.5))
                            .h(rems(2.0))
                            .cursor_pointer()
                            .rounded_md()
                            .hover(move |s| s.bg(surface_hover))
                            .child(Icon::new(IconName::Home).color(text_muted))
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                state_for_home.update(cx, |state, _cx| {
                                    state.app.ui_state.current_screen = Screen::Library;
                                });
                            }),
                    )
                    // Title and plugin count
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .child("SIGNAL CHAIN"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(format!("{} plugins", plugin_count)),
                            ),
                    )
                    // Spacer to balance home button
                    .child(div().w(rems(2.5)))
            })
            // Plugin modules strip — Ozone-style rack
            .child(
                div()
                    .id("plugin-rack")
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_4()
                    .py_3()
                    .overflow_x_scroll()
                    .min_h(rems(7.0))
                    // Plugin modules - Ozone-style cards with left button column
                    .children(modules_info.into_iter().map(
                        |(idx, color, icon, _name, enabled, is_selected, plugin_type, is_permanent, is_input_mon, is_output_mon)| {
                            let theme_c = theme.clone();
                            let drag_info = PluginDragInfo {
                                source_index: idx,
                                name: short_name(&plugin_type, is_input_mon, is_output_mon).to_string(),
                                color,
                                icon,
                                surface: theme_c.surface,
                                text_on_accent: theme_c.text_on_accent,
                            };
                            let drop_highlight = theme_c.drag_over_highlight;
                            let drop_border = theme_c.drag_over_border;

                            // Module card with drop target
                            div()
                                .id(("plugin-module", idx))
                                .group("plugin-module")
                                .w(rems(8.0))
                                .h(rems(6.5))
                                .flex()
                                .flex_row()
                                .rounded_md()
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
                                .overflow_hidden()
                                // Drag-over feedback
                                .drag_over::<PluginDragInfo>({
                                    move |style, _, _, _| {
                                        style.bg(drop_highlight).border_color(drop_border)
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
                                                    state.app.plugin_state.chain.plugins().len();
                                                let source_is_monitor = state
                                                    .app
                                                    .plugin_state.chain
                                                    .get_plugin(source)
                                                    .map(|p| matches!(p.plugin_type(), PluginType::LoudnessMonitor))
                                                    .unwrap_or(false);
                                                if source_is_monitor && target != 0 && target != chain_len - 1 {
                                                    return;
                                                }
                                                state.app.plugin_state.chain.move_plugin(source, target);
                                                state.app.plugin_state.selected_plugin_index = target;
                                                state.app.plugin_state.chain.update_channel_dependent_plugins();
                                                state.app.plugin_state.pending_plugin_update =
                                                    Some(PluginUpdateType::Structural);
                                                state.app.update_level_meter_groups();
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
                                                state.app.plugin_state.selected_plugin_index = idx
                                            });
                                            cx.notify();
                                        },
                                    ),
                                )
                                // Left button column: A (active), S (solo), P (presets), X (remove/lock)
                                .child({
                                    let is_soloed = {
                                        let state = self.state.read(cx);
                                        state.app.plugin_state.soloed_plugin_index == Some(idx)
                                    };
                                    let warning_color = theme_c.warning;
                                    let accent_color = theme_c.accent;

                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_between()
                                        .py_1()
                                        .px(px(4.0))
                                        .gap_1()
                                        .h_full()
                                        .border_r_1()
                                        .border_color(theme_c.border)
                                        // A (Active/Bypass) button
                                        .child(
                                            div()
                                                .id(("plugin-active", idx))
                                                .w(rems(1.125))
                                                .h(rems(1.125))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .bg(if enabled { color } else { theme_c.text_muted })
                                                .when(!enabled, |d| d.opacity(0.4))
                                                .hover(|s| s.opacity(0.8))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |view, _e: &MouseUpEvent, _, cx| {
                                                            cx.stop_propagation();
                                                            view.state.update(cx, |state, _cx| {
                                                                state.app.toggle_plugin(idx);
                                                                state.app.update_level_meter_groups();
                                                            });
                                                            cx.notify();
                                                        },
                                                    ),
                                                )
                                                .text_size(rems(0.625))
                                                .text_color(theme_c.text_on_accent)
                                                .font_weight(FontWeight::BOLD)
                                                .child(if enabled { "A" } else { "B" }),
                                        )
                                        // S (Solo) button — functional
                                        .child(
                                            div()
                                                .id(("plugin-solo", idx))
                                                .w(rems(1.125))
                                                .h(rems(1.125))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .border_1()
                                                .border_color(if is_soloed { warning_color } else { theme_c.text_muted })
                                                .when(is_soloed, |d| d.bg(warning_color).text_color(theme_c.text_on_accent))
                                                .when(!is_soloed, |d| d.text_color(theme_c.text_muted))
                                                .hover(|s| s.border_color(warning_color))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |view, _e: &MouseUpEvent, _, cx| {
                                                            cx.stop_propagation();
                                                            view.state.update(cx, |state, _cx| {
                                                                state.app.toggle_plugin_solo(idx);
                                                                state.app.update_level_meter_groups();
                                                            });
                                                            cx.notify();
                                                        },
                                                    ),
                                                )
                                                .text_size(rems(0.5625))
                                                .font_weight(FontWeight::BOLD)
                                                .child("S"),
                                        )
                                        // P (Presets) button
                                        .child(
                                            div()
                                                .id(("plugin-presets", idx))
                                                .w(rems(1.125))
                                                .h(rems(1.125))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .border_1()
                                                .border_color(theme_c.text_muted)
                                                .hover(|s| s.border_color(accent_color))
                                                .text_size(rems(0.5625))
                                                .text_color(theme_c.text_muted)
                                                .font_weight(FontWeight::BOLD)
                                                .child("P"),
                                        )
                                        // X (Remove) or lock icon for permanent
                                        .when(!is_permanent, |d| {
                                            d.child(
                                                div()
                                                    .id(("plugin-close", idx))
                                                    .w(rems(1.125))
                                                    .h(rems(1.125))
                                                    .rounded_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(theme_c.error).text_color(theme_c.text_on_accent))
                                                    .text_size(rems(0.625))
                                                    .text_color(theme_c.text_muted)
                                                    .font_weight(FontWeight::BOLD)
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |view, _e: &MouseUpEvent, _, cx| {
                                                                cx.stop_propagation();
                                                                view.state.update(cx, |state, _cx| {
                                                                    state.app.remove_plugin(idx);
                                                                    state.app.update_level_meter_groups();
                                                                });
                                                                cx.notify();
                                                            },
                                                        ),
                                                    )
                                                    .child("X"),
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
                                                    .text_size(rems(0.5))
                                                    .text_color(theme_c.text_muted)
                                                    .child("🔒"),
                                            )
                                        })
                                })
                                // Right content area: name on top, icon preview center, drag handle bottom
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .h_full()
                                        .overflow_hidden()
                                        // Plugin name row (top)
                                        .child(
                                            div()
                                                .px_2()
                                                .pt_1()
                                                .text_xs()
                                                .text_color(if is_selected { color } else { theme_c.text_primary })
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .child(short_name(&plugin_type, is_input_mon, is_output_mon)),
                                        )
                                        // Icon / preview area (center)
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(rems(2.5))
                                                .text_color(color)
                                                .child(icon),
                                        )
                                        // Drag handle (bottom-right)
                                        .when(!is_permanent, |d| {
                                            d.child(
                                                div()
                                                    .flex()
                                                    .justify_end()
                                                    .px_1()
                                                    .pb(px(2.0))
                                                    .text_size(rems(0.5))
                                                    .text_color(theme_c.text_muted)
                                                    .child(":::"),
                                            )
                                        }),
                                )
                        },
                    ))
                    // "+" Add plugin slot (dashed border, Ozone-style)
                    .child({
                        let theme_add = theme.clone();
                        let drop_highlight = theme.drag_over_highlight;
                        let drop_border = theme.drag_over_border;
                        let end_idx = plugin_count;

                        div()
                            .id("plugin-add-slot")
                            .w(rems(4.0))
                            .h(rems(5.5))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .border_2()
                            .border_color(theme_add.text_muted)
                            .cursor_pointer()
                            .hover(|s| s.border_color(theme_add.accent))
                            .text_2xl()
                            .text_color(theme_add.text_muted)
                            .hover(|s| s.text_color(theme_add.accent))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.show_add_plugin_menu = !state.app.show_add_plugin_menu;
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
                                    let target = if end_idx > 0 { end_idx - 1 } else { 0 };
                                    if source != target {
                                        view.state.update(cx, |state, _cx| {
                                            let chain_len =
                                                state.app.plugin_state.chain.plugins().len();
                                            let source_is_monitor = state
                                                .app
                                                .plugin_state
                                                .chain
                                                .get_plugin(source)
                                                .map(|p| matches!(p.plugin_type(), PluginType::LoudnessMonitor))
                                                .unwrap_or(false);
                                            if source_is_monitor && target != chain_len - 1 {
                                                return;
                                            }
                                            state.app.plugin_state.chain.move_plugin(source, target);
                                            state.app.plugin_state.selected_plugin_index = target;
                                            state.app.plugin_state.chain.update_channel_dependent_plugins();
                                            state.app.plugin_state.pending_plugin_update =
                                                Some(PluginUpdateType::Structural);
                                            state.app.update_level_meter_groups();
                                        });
                                        cx.notify();
                                    }
                                },
                            ))
                            .child("+")
                    })
                    // Empty state
                    .when(is_empty, |d| {
                        d.child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.text_muted)
                                .child("Click + to add plugins"),
                        )
                    }),
            )
            // Add plugin menu (shown when "+" is clicked)
            .when(self.state.read(cx).app.show_add_plugin_menu, |d| {
                d.child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(theme.surface)
                        .border_t_1()
                        .border_color(theme.border)
                        .child(self.render_add_plugin_buttons(cx)),
                )
            })
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
            .py_4()
            .px_2()
            .bg(theme.background_secondary)
            .border_x_1()
            .border_color(theme.border)
            // Label at top
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_secondary)
                    .mb_2()
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
                    .when(legend_on_left, |d| {
                        d.child(Self::render_side_meter_legend(&theme, false))
                    })
                    // Channel meters
                    .child(
                        div()
                            .flex()
                            .gap(px(1.0))
                            .flex_1()
                            .p(px(2.0))
                            .bg(theme_c.background_secondary)
                            .children(channel_data.into_iter().map(
                                |(fill_ratio, yellow_threshold, red_threshold, name)| {
                                    render_gradient_meter(
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
                    .when(!legend_on_left, |d| {
                        d.child(Self::render_side_meter_legend(&theme, true))
                    }),
            )
    }

    /// Render vertical dB legend for side meters (simplified version without M/S/D spacers)
    fn render_side_meter_legend(
        theme: &crate::theme::Theme,
        align_right: bool,
    ) -> impl IntoElement {
        let ticks = [0, -6, -12, -18, -24, -30, -40, -50, -60];
        let theme = theme.clone();

        // Outer container matches render_gradient_meter structure
        div()
            .flex()
            .flex_col()
            .flex_1()
            .p(px(2.0))
            // Ticks area (matches meter_bar flex_1)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w(px(24.0))
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
                            px(6.0) // Top: move label down
                        } else if db == -60 {
                            px(-6.0) // Bottom: move label up
                        } else {
                            px(0.0) // No additional offset
                        };

                        let label = div()
                            .text_size(rems(0.5625))
                            .text_color(theme.text_muted)
                            .mt(label_offset)
                            .child(format!("{}", db));

                        let tick = div().w(px(4.0)).h(px(1.0)).bg(theme.border);

                        let container = div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                top_fraction,
                            )))
                            // Offset by half line height (~6px for 9px text) to center tick on position
                            .mt(px(-6.0))
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
                div().text_xs().mt_1().opacity(0.0).child("X"), // Invisible spacer
            )
    }

    /// Render add plugin buttons grouped by category (2 rows)
    fn render_add_plugin_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let release_channel = state.app.ui_state.release_channel;

        // Get list of plugins already in chain
        let present_plugins: Vec<_> = state
            .app
            .plugin_state
            .chain
            .plugins()
            .iter()
            .map(|p| p.plugin_type().clone())
            .collect();

        // Count LoudnessMonitor plugins (max 2 allowed: one for input, one for output)
        let monitor_count = present_plugins
            .iter()
            .filter(|p| matches!(p, PluginType::LoudnessMonitor))
            .count();

        // Row 1: Effects/Dynamics plugins
        let row1_plugins = [
            PluginType::EQ,
            PluginType::Gain,
            PluginType::Compressor,
            PluginType::Limiter,
            PluginType::Gate,
            PluginType::Expander,
            PluginType::MultibandCompressor,
            PluginType::MultibandExpander,
            PluginType::Denoiser,
        ];

        // Row 2: Spatial, Monitor, and other plugins
        let row2_plugins = [
            PluginType::Upmixer,
            PluginType::Matrix,
            PluginType::BinauralDecoder,
            PluginType::Convolution,
            PluginType::XTC,
            PluginType::Crossfeed,
            PluginType::LoudnessCompensation,
            PluginType::FletcherMunson,
            PluginType::LoudnessMonitor,
            PluginType::SpectrumAnalyzer,
            PluginType::Pnd,
            PluginType::ABCompare,
            PluginType::Downmix,
            PluginType::MonoToStereo,
            PluginType::BandSplit,
            PluginType::BandMerge,
        ];

        // Build row 1 buttons
        let row1_buttons: Vec<_> = row1_plugins
            .into_iter()
            .enumerate()
            .filter_map(|(i, pt)| {
                if !release_channel.allows(pt.maturity()) {
                    return None;
                }
                let is_single_instance =
                    matches!(pt, PluginType::Upmixer | PluginType::BinauralDecoder);
                if is_single_instance && present_plugins.contains(&pt) {
                    return None;
                }

                let color = plugin_color(&pt, &theme);
                let name = short_name(&pt, false, false);
                let theme_c = theme.clone();

                let text_on_accent = theme_c.text_on_accent;
                Some(
                    div()
                        .id(("add-plugin", i))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(theme_c.surface)
                        .border_1()
                        .border_color(color)
                        .text_xs()
                        .text_color(color)
                        .cursor_pointer()
                        .hover(move |s| s.bg(color).text_color(text_on_accent))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.add_plugin(&pt);
                                    state.app.update_level_meter_groups();
                                    state.app.show_add_plugin_menu = false;
                                });
                                cx.notify();
                            }),
                        )
                        .child(name),
                )
            })
            .collect();

        // Build row 2 buttons
        let row2_buttons: Vec<_> = row2_plugins
            .into_iter()
            .enumerate()
            .filter_map(|(i, pt)| {
                if !release_channel.allows(pt.maturity()) {
                    return None;
                }
                let is_single_instance =
                    matches!(pt, PluginType::Upmixer | PluginType::BinauralDecoder);
                if is_single_instance && present_plugins.contains(&pt) {
                    return None;
                }

                // LoudnessMonitor: max 2 allowed (one for input, one for output)
                if matches!(pt, PluginType::LoudnessMonitor) && monitor_count >= 2 {
                    return None;
                }

                let color = plugin_color(&pt, &theme);
                let name = short_name(&pt, false, false);
                let theme_c = theme.clone();

                let text_on_accent = theme_c.text_on_accent;
                Some(
                    div()
                        .id(("add-plugin", i + 100))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(theme_c.surface)
                        .border_1()
                        .border_color(color)
                        .text_xs()
                        .text_color(color)
                        .cursor_pointer()
                        .hover(move |s| s.bg(color).text_color(text_on_accent))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.add_plugin(&pt);
                                    state.app.update_level_meter_groups();
                                    state.app.show_add_plugin_menu = false;
                                });
                                cx.notify();
                            }),
                        )
                        .child(name),
                )
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_1()
            // Row 1
            .child(div().flex().items_center().gap_2().children(row1_buttons))
            // Row 2
            .child(div().flex().items_center().gap_2().children(row2_buttons))
    }

    /// Render the plugin detail/settings panel
    fn render_plugin_detail_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (plugin_data, selected_idx, editing_idx, param_selection, theme) = {
            let state = self.state.read(cx);
            let plugin = state
                .app
                .plugin_state
                .chain
                .get_plugin(state.app.plugin_state.selected_plugin_index)
                .cloned();
            (
                plugin,
                state.app.plugin_state.selected_plugin_index,
                state.app.plugin_state.editing_plugin_index,
                state.app.plugin_state.plugin_param_selection,
                state.app.ui_state.theme.clone(),
            )
        };

        let has_plugin = plugin_data.is_some();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h(rems(21.875))
            .when(has_plugin, |d| {
                let plugin = plugin_data.clone().unwrap();
                let plugin_type = plugin.plugin_type().clone();
                let _plugin_name = plugin_type.name().to_string();
                let _color = plugin_color(&plugin_type, &theme);
                let is_editing = editing_idx.is_some();
                let _plugin_enabled = plugin.enabled;

                let plugin_ui_view = self.state.read(cx).app.plugin_state.plugin_ui_view.clone();
                let controller_picker_open = self.state.read(cx).app.plugin_state.controller_picker_open;
                let state_for_toggle = self.state.clone();

                // Pre-extract theme colors for toggle buttons (Rgba is Copy)
                let toggle_text_primary = theme.text_primary;
                let toggle_text_muted = theme.text_muted;
                let toggle_surface = theme.surface;
                let toggle_surface_hover = theme.surface_hover;
                let toggle_accent = theme.accent;
                let toggle_text_on_accent = theme.text_on_accent;
                let toggle_bg_secondary = theme.background_secondary;
                let toggle_border = theme.border;

                d.child(
                    // Plugin header bar
                    {
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .px_4()
                            .py_3()
                            .bg(theme.background_secondary)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                // View mode toggle: UI / Controller / Simple
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    // UI tab
                                    .child({
                                        let is_active = plugin_ui_view.is_ui();
                                        let state_c = state_for_toggle.clone();
                                        div()
                                            .id("view-ui")
                                            .cursor_pointer()
                                            .px_2()
                                            .py(px(2.0))
                                            .rounded_md()
                                            .text_xs()
                                            .font_weight(if is_active { FontWeight::BOLD } else { FontWeight::NORMAL })
                                            .text_color(if is_active { toggle_text_primary } else { toggle_text_muted })
                                            .when(is_active, |d| d.bg(toggle_surface))
                                            .hover(move |s| s.bg(toggle_surface_hover))
                                            .on_click(move |_, _window, cx| {
                                                state_c.update(cx, |s, _| {
                                                    s.app.plugin_state.plugin_ui_view = PluginUiView::UI;
                                                    s.app.plugin_state.controller_picker_open = false;
                                                });
                                            })
                                            .child("UI")
                                    })
                                    // Controller tab with dropdown
                                    .child({
                                        let is_active = plugin_ui_view.is_controller();
                                        let state_c = state_for_toggle.clone();
                                        div()
                                            .relative()
                                            .child(
                                                div()
                                                    .id("view-controller")
                                                    .cursor_pointer()
                                                    .px_2()
                                                    .py(px(2.0))
                                                    .rounded_md()
                                                    .text_xs()
                                                    .font_weight(if is_active { FontWeight::BOLD } else { FontWeight::NORMAL })
                                                    .text_color(if is_active { toggle_text_primary } else { toggle_text_muted })
                                                    .when(is_active, |d| d.bg(toggle_surface))
                                                    .hover(move |s| s.bg(toggle_surface_hover))
                                                    .on_click({
                                                        let state_c = state_c.clone();
                                                        move |_, _window, cx| {
                                                            state_c.update(cx, |s, _| {
                                                                s.app.plugin_state.controller_picker_open =
                                                                    !s.app.plugin_state.controller_picker_open;
                                                            });
                                                        }
                                                    })
                                                    .child(if let PluginUiView::Controller(ref name) = plugin_ui_view {
                                                        // Show selected controller name
                                                        available_controllers()
                                                            .iter()
                                                            .find(|(id, _)| id == name)
                                                            .map(|(_, label)| *label)
                                                            .unwrap_or("Controller")
                                                    } else {
                                                        "Controller"
                                                    }),
                                            )
                                            // Dropdown menu
                                            .when(controller_picker_open, |d| {
                                                d.child(
                                                    div()
                                                        .absolute()
                                                        .top(px(24.0))
                                                        .left_0()
                                                        .min_w(px(160.0))
                                                        .bg(toggle_bg_secondary)
                                                        .border_1()
                                                        .border_color(toggle_border)
                                                        .rounded_md()
                                                        .shadow_md()
                                                        .p_1()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(2.0))
                                                        .children(available_controllers().into_iter().map(|(id, label)| {
                                                            let state_c2 = state_c.clone();
                                                            let is_selected = matches!(&plugin_ui_view, PluginUiView::Controller(n) if n == id);
                                                            let id_owned = id.to_string();
                                                            div()
                                                                .id(SharedString::from(format!("ctrl-{id}")))
                                                                .cursor_pointer()
                                                                .px_2()
                                                                .py(px(3.0))
                                                                .rounded_sm()
                                                                .text_xs()
                                                                .when(is_selected, |d| {
                                                                    d.bg(toggle_accent)
                                                                        .text_color(toggle_text_on_accent)
                                                                        .font_weight(FontWeight::BOLD)
                                                                })
                                                                .when(!is_selected, |d| {
                                                                    d.text_color(toggle_text_primary)
                                                                        .hover(move |s| s.bg(toggle_surface_hover))
                                                                })
                                                                .on_click(move |_, _window, cx| {
                                                                    state_c2.update(cx, |s, _| {
                                                                        s.app.plugin_state.plugin_ui_view =
                                                                            PluginUiView::Controller(id_owned.clone());
                                                                        s.app.plugin_state.controller_picker_open = false;
                                                                    });
                                                                })
                                                                .child(label)
                                                        })),
                                                )
                                            })
                                    })
                                    // Simple tab
                                    .child({
                                        let is_active = plugin_ui_view.is_simple();
                                        let state_c = state_for_toggle.clone();
                                        div()
                                            .id("view-simple")
                                            .cursor_pointer()
                                            .px_2()
                                            .py(px(2.0))
                                            .rounded_md()
                                            .text_xs()
                                            .font_weight(if is_active { FontWeight::BOLD } else { FontWeight::NORMAL })
                                            .text_color(if is_active { toggle_text_primary } else { toggle_text_muted })
                                            .when(is_active, |d| d.bg(toggle_surface))
                                            .hover(move |s| s.bg(toggle_surface_hover))
                                            .on_click(move |_, _window, cx| {
                                                state_c.update(cx, |s, _| {
                                                    s.app.plugin_state.plugin_ui_view = PluginUiView::Simple;
                                                    s.app.plugin_state.controller_picker_open = false;
                                                });
                                            })
                                            .child("Simple")
                                    }),
                            )
                    }, /* row with infos but not useful
                                                                             .child(
                                                                                 div()
                                                                                     .flex()
                                                                                     .items_center()
                                                                                     .gap_3()
                                                                                     // Color indicator
                                                                                     .child(div().w(px(4.0)).h(px(24.0)).rounded_full().bg(color))
                                                                                     .child(
                                                                                         div()
                                                                                             .flex()
                                                                                             .flex_col()
                                                                                             .child(
                                                                                                 div()
                                                                                                     .text_lg()
                                                                                                     .font_weight(FontWeight::BOLD)
                                                                                                     .text_color(theme.text_primary)
                                                                                                     .child(plugin_name),
                                                                                             )
                                                                                             .child(div().text_xs().text_color(theme.text_muted).child(
                                                                                                 format!(
                                                                                                     "[{}] {} - Slot {} - {}",
                                                                                                     selected_idx + 1,
                                                                                                     short_name(&plugin_type),
                                                                                                     selected_idx + 1,
                                                                                                     if plugin_enabled { "Active" } else { "Bypassed" }
                                                                                                 ),
                                                                                             )),
                                                                                     ),
                                                                             ),
                                                     */
                )
                .child({
                    // Calculate output channels based on plugin chain for conditional divider
                    let state = self.state.read(cx);
                    let mut output_channels = 2;
                    for p in state.app.plugin_state.chain.plugins() {
                        if p.enabled {
                            match p.plugin_type() {
                                PluginType::Upmixer => {
                                    if let sotf_audio_player::PluginSettings::Upmixer {
                                        speaker_config,
                                        ..
                                    } = &p.settings
                                    {
                                        output_channels =
                                            speaker_config_to_channels(speaker_config);
                                    } else {
                                        output_channels = 6;
                                    }
                                }
                                PluginType::BinauralDecoder => output_channels = 2,
                                _ => {}
                            }
                        }
                    }
                    let _has_multichannel = output_channels > 2;

                    let divider_theme = PaneDividerTheme {
                        background: theme.background,
                        background_hover: theme.surface_hover,
                        background_collapsed: theme.surface,
                        foreground: theme.text_muted,
                        foreground_hover: theme.text_secondary,
                        border: theme.border,
                    };

                    let output_collapsed = state.app.output_meter_collapsed;
                    let output_meter_width = state.app.output_meter_width;

                    // Create state clones for divider callbacks
                    let state_for_output_toggle = self.state.clone();
                    let state_for_output_drag = self.state.clone();

                    div()
                        .flex_1()
                        .flex()
                        .min_h(rems(18.75)) // Minimum height for meters and content
                        // Plugin Content (now takes full left area)
                        .child(
                            div()
                                .id("params-scroll")
                                .flex_1()
                                .overflow_y_scroll()
                                .p_4()
                                .child({
                                    // Get plugin-specific real-time data based on plugin type
                                    let plugin_data: Option<
                                        std::sync::Arc<dyn std::any::Any + Send + Sync>,
                                    > = match &plugin.settings {
                                        sotf_audio_player::PluginSettings::SpectrumAnalyzer {
                                            ..
                                        } => {
                                            self.state.read(cx).app.playback.spectrum_info.clone().map(|s| {
                                                std::sync::Arc::new(s)
                                                    as std::sync::Arc<
                                                        dyn std::any::Any + Send + Sync,
                                                    >
                                            })
                                        }
                                        sotf_audio_player::PluginSettings::Compressor {
                                            ..
                                        } => self.state.read(cx).app.playback.compressor_info.clone().map(
                                            |c| {
                                                std::sync::Arc::new(c)
                                                    as std::sync::Arc<
                                                        dyn std::any::Any + Send + Sync,
                                                    >
                                            },
                                        ),
                                        _ => None,
                                    };

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
                                                .plugin_state.chain
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
                                            } else if selected_idx
                                                == *monitor_indices.last().unwrap()
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
                                    let upmixer_config_open = app_st.app.upmixer_config_open;
                                    let selected_eq_band = app_st.app.plugin_state.selected_eq_band;
                                    let spectrum_tilt_open = app_st.app.spectrum_tilt_select_open;
                                    let spectrum_ref_open = app_st.app.spectrum_reference_select_open;
                                    let plugin_chain = app_st.app.plugin_state.chain.clone();
                                    let midi_overlay = app_st.app.plugin_state.midi_mapping.build_overlay(&[]);
                                    let plugin_ui_view = app_st.app.plugin_state.plugin_ui_view.clone();

                                    let midi_ref = if midi_overlay.has_controller() {
                                        Some(midi_overlay)
                                    } else {
                                        None
                                    };

                                    match &plugin_ui_view {
                                        PluginUiView::Simple => {
                                            super::ui_simple::render_simple_plugin_view(
                                                self.state.clone(),
                                                selected_idx,
                                                &plugin.settings,
                                                is_editing,
                                                param_selection,
                                                &theme,
                                                midi_ref.as_ref(),
                                            )
                                            .into_any_element()
                                        }
                                        PluginUiView::Controller(controller_id) => {
                                            // Build overlay from selected controller layout
                                            let controller_overlay = build_controller_overlay(
                                                controller_id,
                                                &app_st.app.plugin_state.midi_mapping,
                                            );
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
                                                &plugin_chain,
                                                Some(&controller_overlay),
                                                cx,
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
                                                &plugin_chain,
                                                midi_ref.as_ref(),
                                                cx,
                                            )
                                        }
                                    }
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
                                        s.app.output_meter_collapsed = collapsed;
                                    });
                                })
                                .on_drag_start(move |pos, _window, cx| {
                                    state_for_output_drag.update(cx, |s, _| {
                                        s.app.dragging_divider = Some(DividerDragState {
                                            divider_type: DividerType::OutputMeter,
                                            start_x: pos,
                                            start_width: s.app.output_meter_width,
                                        });
                                    });
                                }),
                        )
                        // Right: Combined IN/OUT Meter + Chain Controls
                        .when(!output_collapsed, |d| {
                            d.child(
                                div()
                                    .w(px(output_meter_width))
                                    .h_full()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .child(self.render_combined_meter(cx, 2, output_channels))
                                    .child(self.render_chain_controls(cx)),
                            )
                        })
                })
            })
            .when(!has_plugin, |d| {
                d.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_col()
                        .gap_2()
                        .text_color(theme.text_muted)
                        .child("No plugin selected")
                        .child(div().text_sm().child("Add a plugin to get started")),
                )
            })
    }

    /// Render combined IN + OUT meter panel using the same meter components as the main library view
    fn render_combined_meter(
        &self,
        cx: &mut Context<Self>,
        input_channels: usize,
        output_channels: usize,
    ) -> impl IntoElement {
        use sotf_audio_player::{ChannelGroup, ChannelInfo};

        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let input_loudness = state.app.playback.input_loudness_info.clone();
        let output_loudness = state.app.playback.loudness_info.clone();
        let peak_hold = state.app.level_meter_peak_hold.clone();

        let channel_names = ["L", "R", "C", "LFE", "Ls", "Rs", "Lb", "Rb", "Lw", "Rw"];

        let make_group = |name: &str, channels: usize| -> ChannelGroup {
            ChannelGroup {
                name: name.to_string(),
                channels: (0..channels)
                    .map(|i| {
                        let ch_name = channel_names.get(i).unwrap_or(&".").to_string();
                        ChannelInfo {
                            index: i,
                            name: ch_name.clone(),
                            display_name: ch_name.chars().map(|c| c.to_string()).collect(),
                        }
                    })
                    .collect(),
                muted: false,
                soloed: false,
                dimmed: false,
            }
        };

        let in_group = make_group("IN", input_channels);
        let out_group = make_group("OUT", output_channels);

        // Use group indices beyond level_meter_groups so M/S/D clicks are no-ops
        let fake_in_idx = 1000;
        let fake_out_idx = 1001;

        div()
            .flex()
            .flex_col()
            .flex_1()
            .bg(theme.background_secondary)
            .border_l_1()
            .border_color(theme.border)
            // Meters: [IN group] [legend] [OUT group]
            .child(
                div()
                    .flex()
                    .gap_0()
                    .flex_1()
                    .min_h(rems(18.75))
                    .child(
                        self.render_meter_group(
                            &in_group,
                            fake_in_idx,
                            false,
                            input_loudness.as_ref(),
                            &peak_hold,
                            &theme,
                            cx,
                        )
                        .into_any_element(),
                    )
                    .child(self.render_vertical_legend(&theme, true).into_any_element())
                    .child(
                        self.render_meter_group(
                            &out_group,
                            fake_out_idx,
                            false,
                            output_loudness.as_ref(),
                            &peak_hold,
                            &theme,
                            cx,
                        )
                        .into_any_element(),
                    ),
            )
    }

    /// Render chain-level control buttons (Bypass, AutoGain, Mono, M/S) in a 2×2 grid
    fn render_chain_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let chain_bypass = state.app.plugin_state.chain_bypass;
        let chain_autogain = state.app.plugin_state.chain_autogain;

        // Detect current matrix preset for Mono/M/S button states
        let (is_mono, is_ms) = {
            let mut mono = false;
            let mut ms = false;
            for plugin in state.app.plugin_state.chain.plugins() {
                if plugin.is_permanent()
                    && matches!(plugin.plugin_type(), PluginType::Matrix)
                {
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
                .py(px(4.0))
                .cursor_pointer()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .rounded_sm()
                .when(is_active, |d| d.bg(active_bg).text_color(active_text))
                .when(!is_active, |d| {
                    d.bg(inactive_bg)
                        .text_color(inactive_text)
                        .hover(move |s| s.bg(hover_bg))
                })
                .child(label)
        };

        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .p_2()
            .bg(theme.background_secondary)
            .border_l_1()
            .border_t_1()
            .border_color(border)
            // Row 1: Bypass | AutoGain
            .child(
                div()
                    .flex()
                    .gap(px(2.0))
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
                    .gap(px(2.0))
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

    fn toggle_upmixer_config(
        &mut self,
        action: &ToggleUpmixerConfig,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.upmixer_config_open = action.open;
        });
        cx.notify();
    }
}

/// Build a MidiOverlay for a specific controller layout.
///
/// If the mapping engine already has a real mapping, use it. Otherwise, create
/// a synthetic overlay showing the controller's layout name so the UI renders
/// the controller header even without live MIDI input.
fn build_controller_overlay(controller_id: &str, engine: &MidiMappingEngine) -> MidiOverlay {
    // If the engine already has a mapping for this controller, use it
    let engine_overlay = engine.build_overlay(&[]);
    if engine_overlay.has_controller()
        && engine_overlay.controller_name.as_ref().is_some_and(|name| {
            available_controllers()
                .iter()
                .any(|(id, _)| name.contains(id) || id == &controller_id)
        })
    {
        return engine_overlay;
    }

    // Build a minimal overlay showing the controller name
    let display_name = available_controllers()
        .iter()
        .find(|(id, _)| *id == controller_id)
        .map(|(_, label)| label.to_string())
        .unwrap_or_else(|| controller_id.to_string());

    MidiOverlay {
        controller_name: Some(display_name),
        current_page: 0,
        total_pages: 1,
        ..MidiOverlay::empty()
    }
}
