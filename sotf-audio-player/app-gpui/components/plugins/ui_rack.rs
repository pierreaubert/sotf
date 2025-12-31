//! Plugin screen rendering functions - Professional DAW-style interface

use super::actions::ToggleUpmixerConfig;
use super::level_meters::{db_to_position, render_gradient_meter};
use super::render_plugin_content;
use crate::app::state::{DividerDragState, DividerType};
use crate::app::types::{PluginUpdateType, Screen};
use crate::components::icons::{Icon, IconName};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui::{MouseMoveEvent, MouseUpEvent};
use gpui_ui_kit::{CollapseDirection, PaneDivider, PaneDividerTheme};
use sotf_audio_player::PluginType;

/// Drag information for plugin reordering
#[derive(Clone)]
pub struct PluginDragInfo {
    pub source_index: usize,
    pub name: String,
    pub color: Rgba,
    pub icon: &'static str,
}

impl Render for PluginDragInfo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Drag preview - a smaller version of the plugin module
        div()
            .w(px(70.0))
            .h(px(60.0))
            .flex()
            .flex_col()
            .rounded_lg()
            .border_color(self.color)
            .bg(Theme::opacity_20pct(Rgba {
                r: 0.118,
                g: 0.118,
                b: 0.180,
                a: 1.0,
            })) // Semi-transparent dark background (theme.surface with opacity)
            .shadow_lg()
            .opacity(0.9)
            // Top bar with color
            .child(div().h(px(3.0)).w_full().bg(self.color).rounded_t_md())
            // Icon
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xl()
                    .text_color(self.color)
                    .child(self.icon),
            )
            // Name
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .text_color(rgb(0xffffff)) // text_on_accent color
                    .font_weight(FontWeight::MEDIUM)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(self.name.clone()),
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
        PluginType::BinauralDecoder => theme.plugin_colors.binaural,
        PluginType::Convolution => theme.plugin_colors.convolution,
        PluginType::LoudnessMonitor => theme.plugin_colors.monitor,
        PluginType::SpectrumAnalyzer => theme.plugin_colors.spectrum,
        PluginType::ChannelMuteSolo => theme.plugin_colors.mute_solo,
        PluginType::Matrix => theme.plugin_colors.upmixer, // Reuse upmixer color for matrix
        PluginType::XTC => theme.plugin_colors.binaural,   // Reuse binaural color for XTC
        PluginType::Denoiser => theme.plugin_colors.eq,    // Reuse eq color for denoiser
        PluginType::Pnd => theme.plugin_colors.eq,         // Reuse eq color for pnd
    }
}

fn plugin_icon(plugin_type: &PluginType) -> &'static str {
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
        PluginType::BinauralDecoder => "◎",
        PluginType::Convolution => "∿",
        PluginType::LoudnessMonitor => "◐",
        PluginType::SpectrumAnalyzer => "▓",
        PluginType::ChannelMuteSolo => "◧",
        PluginType::Matrix => "⊞",
        PluginType::XTC => "⊗",
        PluginType::Denoiser => "◌",
        PluginType::Pnd => "♪",
    }
}

fn short_name(plugin_type: &PluginType) -> &'static str {
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
        PluginType::BinauralDecoder => "Binaural",
        PluginType::Convolution => "Convolution",
        PluginType::LoudnessMonitor => "Monitor",
        PluginType::SpectrumAnalyzer => "Spectrum",
        PluginType::ChannelMuteSolo => "Mixer",
        PluginType::Matrix => "Matrix",
        PluginType::XTC => "XTC",
        PluginType::Denoiser => "Denoiser",
        PluginType::Pnd => "PND",
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
        let theme = self.state.read(cx).app.theme.clone();

        div()
            .id("plugins-screen")
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
            let plugins: Vec<_> = state
                .app
                .plugin_chain
                .plugins()
                .iter()
                .map(|p| {
                    (
                        p.plugin_type().clone(),
                        p.enabled,
                        p.plugin_type().name().to_string(),
                    )
                })
                .collect();
            (
                plugins,
                state.app.selected_plugin_index,
                state.app.theme.clone(),
            )
        };

        // Pre-compute static data for plugin modules
        let modules_info: Vec<_> = plugins_data
            .iter()
            .enumerate()
            .map(|(idx, (pt, enabled, name))| {
                (
                    idx,
                    plugin_color(pt, &theme),
                    plugin_icon(pt),
                    name.clone(),
                    *enabled,
                    selected_idx == idx,
                    pt.clone(), // Include plugin type for short_name
                )
            })
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
                            .w(px(40.0))
                            .h(px(32.0))
                            .cursor_pointer()
                            .rounded_md()
                            .hover(move |s| s.bg(surface_hover))
                            .child(Icon::new(IconName::Home).color(text_muted))
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                state_for_home.update(cx, |state, _cx| {
                                    state.app.current_screen = Screen::Library;
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
                    // Add plugin buttons on the right
                    .child(self.render_add_plugin_buttons(cx))
            })
            // Plugin modules strip
            .child(
                div()
                    .id("plugin-rack")
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .overflow_x_scroll()
                    .min_h(px(140.0))
                    // Input Meter removed from rack strip (moved to detail panel)
                    // Plugin modules - inline creation with drag-and-drop
                    .children(modules_info.into_iter().map(
                        |(idx, color, icon, name, enabled, is_selected, plugin_type)| {
                            let theme_c = theme.clone();
                            let drag_info = PluginDragInfo {
                                source_index: idx,
                                name: name.clone(),
                                color,
                                icon,
                            };
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                // Connection line before
                                .child(div().w(px(20.0)).h(px(2.0)).bg(if enabled {
                                    theme_c.accent
                                } else {
                                    theme_c.text_muted
                                }))
                                // Plugin module box - draggable and droppable
                                .child(
                                    div()
                                        .id(("plugin-module", idx))
                                        .group("plugin-module")
                                        .w(px(80.0))
                                        .h(px(90.0))
                                        .flex()
                                        .flex_col()
                                        .rounded_lg()
                                        .border_2()
                                        .border_color(if is_selected {
                                            color
                                        } else {
                                            theme_c.border
                                        })
                                        .bg(theme_c.surface)
                                        .when(!enabled, |d| d.opacity(0.6))
                                        .shadow_md()
                                        .cursor_grab()
                                        .hover(|s| s.border_color(color))
                                        // Drag-over visual feedback - highlight when dragging over
                                        .drag_over::<PluginDragInfo>(|style, _, _, _| {
                                            style
                                                .bg(rgba(0x3b82f640)) // Blue tint when dragging over
                                                .border_color(rgb(0x3b82f6))
                                        })
                                        // Handle drop - reorder plugins
                                        .on_drop(cx.listener(
                                            move |view, info: &PluginDragInfo, _window, cx| {
                                                let source = info.source_index;
                                                let target = idx;
                                                if source != target {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .plugin_chain
                                                            .move_plugin(source, target);
                                                        state.app.selected_plugin_index = target;
                                                        state.app.pending_plugin_update =
                                                            Some(PluginUpdateType::Structural);
                                                        state.app.update_level_meter_groups(); // Reconfigure metering
                                                    });
                                                    cx.notify();
                                                }
                                            },
                                        ))
                                        // Start drag
                                        .on_drag(drag_info, |info, _position, _window, cx| {
                                            cx.new(|_| info.clone())
                                        })
                                        // Click to select
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(
                                                move |view, _: &MouseUpEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.selected_plugin_index = idx
                                                    });
                                                    cx.notify();
                                                },
                                            ),
                                        )
                                        // Top bar with color
                                        .child(div().h(px(4.0)).w_full().bg(color).rounded_t_md())
                                        // Remove button (X) - visible on group hover
                                        .child(
                                            div()
                                                .absolute()
                                                .top(px(8.0))
                                                .right(px(4.0))
                                                .w(px(12.0))
                                                .h(px(12.0))
                                                .rounded_full()
                                                .bg(theme_c.error)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_xs()
                                                .text_color(rgb(0xffffff))
                                                .cursor_pointer()
                                                .opacity(0.0)
                                                .group_hover("plugin-module", |s| s.opacity(1.0))
                                                .hover(|s| s.bg(theme_c.error))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |view, _e: &MouseUpEvent, _, cx| {
                                                            cx.stop_propagation();
                                                            view.state.update(cx, |state, _cx| {
                                                                state.app.remove_plugin(idx);
                                                                state
                                                                    .app
                                                                    .update_level_meter_groups(); // Reconfigure metering
                                                            });
                                                            cx.notify();
                                                        },
                                                    ),
                                                )
                                                .child("×"),
                                        )
                                        // Power indicator (top left)
                                        .child(
                                            div()
                                                .absolute()
                                                .top(px(8.0))
                                                .left(px(4.0))
                                                .w(px(12.0))
                                                .h(px(12.0))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |view, _e: &MouseUpEvent, _, cx| {
                                                            cx.stop_propagation();
                                                            view.state.update(cx, |state, _cx| {
                                                                state.app.toggle_plugin(idx);
                                                                state
                                                                    .app
                                                                    .update_level_meter_groups(); // Reconfigure metering
                                                            });
                                                            cx.notify();
                                                        },
                                                    ),
                                                )
                                                .bg(if enabled {
                                                    theme_c.success
                                                } else {
                                                    theme_c.error
                                                })
                                                .text_size(px(8.0))
                                                .text_color(rgb(0xffffff))
                                                .child(if enabled { "●" } else { "○" }),
                                        )
                                        // Icon
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_xl()
                                                .text_color(color)
                                                .child(icon),
                                        )
                                        // Name
                                        .child(
                                            div()
                                                .px_2()
                                                .pb_2()
                                                .text_xs()
                                                .text_color(theme_c.text_primary)
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_align(TextAlign::Center)
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .child(short_name(&plugin_type)),
                                        ),
                                )
                                // Connection line after
                                .child(div().w(px(20.0)).h(px(2.0)).bg(if enabled {
                                    theme_c.accent
                                } else {
                                    theme_c.text_muted
                                }))
                        },
                    ))
                    // Output Meter removed from rack strip (moved to detail panel)
                    // Empty state
                    .when(is_empty, |d| {
                        d.child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.text_muted)
                                .child("Click + Add Plugin to insert effects"),
                        )
                    }),
            )
    }

    /// Render a side level meter group for the detail panel
    /// Matches the style of the queue screen meters with vertical dB legend
    fn render_side_meter(
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
                state.app.input_loudness_info.clone()
            } else {
                state.app.loudness_info.clone()
            };
            (state.app.theme.clone(), loudness)
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
                            .text_size(px(9.0))
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
        let theme = state.app.theme.clone();

        // Get list of plugins already in chain
        let present_plugins: Vec<_> = state
            .app
            .plugin_chain
            .plugins()
            .iter()
            .map(|p| p.plugin_type().clone())
            .collect();

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
            PluginType::LoudnessCompensation,
            PluginType::LoudnessMonitor,
            PluginType::SpectrumAnalyzer,
            PluginType::Pnd,
        ];

        // Build row 1 buttons
        let row1_buttons: Vec<_> = row1_plugins
            .into_iter()
            .enumerate()
            .filter_map(|(i, pt)| {
                let is_single_instance =
                    matches!(pt, PluginType::Upmixer | PluginType::BinauralDecoder);
                if is_single_instance && present_plugins.contains(&pt) {
                    return None;
                }

                let color = plugin_color(&pt, &theme);
                let name = short_name(&pt);
                let theme_c = theme.clone();

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
                        .hover(|s| s.bg(color).text_color(rgb(0xffffff)))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.add_plugin(&pt);
                                    state.app.update_level_meter_groups();
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
                let is_single_instance =
                    matches!(pt, PluginType::Upmixer | PluginType::BinauralDecoder);
                if is_single_instance && present_plugins.contains(&pt) {
                    return None;
                }

                let color = plugin_color(&pt, &theme);
                let name = short_name(&pt);
                let theme_c = theme.clone();

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
                        .hover(|s| s.bg(color).text_color(rgb(0xffffff)))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.add_plugin(&pt);
                                    state.app.update_level_meter_groups();
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
                .plugin_chain
                .get_plugin(state.app.selected_plugin_index)
                .cloned();
            (
                plugin,
                state.app.selected_plugin_index,
                state.app.editing_plugin_index,
                state.app.plugin_param_selection,
                state.app.theme.clone(),
            )
        };

        let has_plugin = plugin_data.is_some();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h(px(350.0))
            .when(has_plugin, |d| {
                let plugin = plugin_data.clone().unwrap();
                let plugin_type = plugin.plugin_type().clone();
                let _plugin_name = plugin_type.name().to_string();
                let _color = plugin_color(&plugin_type, &theme);
                let is_editing = editing_idx.is_some();
                let _plugin_enabled = plugin.enabled;

                d.child(
                    // Plugin header bar
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .px_4()
                        .py_3()
                        .bg(theme.background_secondary)
                        .border_b_1()
                        .border_color(theme.border), /* row with infos but not useful
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
                    for p in state.app.plugin_chain.plugins() {
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

                    let input_collapsed = state.app.input_meter_collapsed;
                    let output_collapsed = state.app.output_meter_collapsed;
                    let input_meter_width = state.app.input_meter_width;
                    let output_meter_width = state.app.output_meter_width;

                    // Create state clones for divider callbacks
                    let state_for_input_toggle = self.state.clone();
                    let state_for_input_drag = self.state.clone();
                    let state_for_output_toggle = self.state.clone();
                    let state_for_output_drag = self.state.clone();

                    div()
                        .flex_1()
                        .flex()
                        .min_h(px(300.0)) // Minimum height for meters and content
                        // Left: Input Meter (legend on right side, facing center)
                        .when(!input_collapsed, |d| {
                            d.child(
                                div()
                                    .w(px(input_meter_width))
                                    .h_full()
                                    .flex_shrink_0()
                                    .child(self.render_side_meter(cx, 2, "IN", false, true)),
                            )
                        })
                        // Divider 1: Between input meter and main zone
                        .child(
                            PaneDivider::vertical("input-meter-divider", CollapseDirection::Left)
                                .label("IN")
                                .theme(divider_theme.clone())
                                .thickness(px(6.0))
                                .collapsed(input_collapsed)
                                .on_toggle(move |collapsed, _window, cx| {
                                    state_for_input_toggle.update(cx, |s, _| {
                                        s.app.input_meter_collapsed = collapsed;
                                    });
                                })
                                .on_drag_start(move |pos, _window, cx| {
                                    state_for_input_drag.update(cx, |s, _| {
                                        s.app.dragging_divider = Some(DividerDragState {
                                            divider_type: DividerType::InputMeter,
                                            start_x: pos,
                                            start_width: s.app.input_meter_width,
                                        });
                                    });
                                }),
                        )
                        // Center: Plugin Content
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
                                            self.state.read(cx).app.spectrum_info.clone().map(|s| {
                                                std::sync::Arc::new(s)
                                                    as std::sync::Arc<
                                                        dyn std::any::Any + Send + Sync,
                                                    >
                                            })
                                        }
                                        sotf_audio_player::PluginSettings::Compressor { .. } => {
                                            self.state.read(cx).app.compressor_info.clone().map(|c| {
                                                std::sync::Arc::new(c)
                                                    as std::sync::Arc<
                                                        dyn std::any::Any + Send + Sync,
                                                    >
                                            })
                                        }
                                        _ => None,
                                    };

                                    render_plugin_content(
                                        self.state.clone(),
                                        selected_idx, // Pass index
                                        &plugin.settings,
                                        is_editing,
                                        param_selection,
                                        &theme,
                                        self.state.read(cx).app.upmixer_config_open,
                                        self.state.read(cx).app.selected_eq_band,
                                        // Pass loudness_info for backward compatibility
                                        self.state.read(cx).app.loudness_info.clone(),
                                        plugin_data,
                                    )
                                }),
                        )
                        // Divider 2: Between main zone and output meter (always shown)
                        .child(
                            PaneDivider::vertical(
                                "output-meter-divider",
                                CollapseDirection::Right,
                            )
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
                        // Right: Output Meter (always shown when not collapsed)
                        .when(!output_collapsed, |d| {
                            d.child(
                                div()
                                    .w(px(output_meter_width))
                                    .h_full()
                                    .flex_shrink_0()
                                    .child(self.render_side_meter(cx, output_channels, "OUT", true, false)),
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
