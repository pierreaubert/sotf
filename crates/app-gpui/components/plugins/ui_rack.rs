//! Plugin screen rendering functions - Professional DAW-style interface

use super::actions::ToggleUpmixerConfig;
use crate::app::state::plugin::available_controllers;
use crate::app::state::DividerType;
use crate::app::types::{PluginUpdateType, Screen};
use crate::components::design::Ds;
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
use sotf_audio_player_midi::MidiMappingEngine;
use sotf_audio_player_midi::mapping::MidiOverlay;

use crate::components::themed_tooltip as make_tooltip;

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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        // Drag preview — matches the Ozone-style card
        div()
            .w(rems(7.0))
            .h(rems(4.0))
            .flex()
            .flex_row()
            .rounded(d.r_md)
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
                            .px(d.pad_y)
                            .pt(d.pad_y_half)
                            .text_size(d.text_xs)
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
                            .text_size(d.text_lg)
                            .text_color(self.color)
                            .child(self.icon),
                    ),
            )
    }
}

// Plugin visual identity — canonical implementations in ui_plugin_shell.rs
use super::ui_plugin_shell::{plugin_accent_color as plugin_color, plugin_icon, plugin_short_name};

pub(crate) fn short_name(plugin_type: &PluginType, is_input_mon: bool, is_output_mon: bool) -> &'static str {
    plugin_short_name(plugin_type, is_input_mon, is_output_mon, false)
}

fn short_name_with_permanent(
    plugin_type: &PluginType,
    is_input_mon: bool,
    is_output_mon: bool,
    is_permanent: bool,
) -> &'static str {
    plugin_short_name(plugin_type, is_input_mon, is_output_mon, is_permanent)
}

/// Brief description for each plugin type (shown in add-plugin menu tooltips).
pub(crate) fn plugin_description(plugin_type: &PluginType) -> &'static str {
    match plugin_type {
        PluginType::EQ => "Parametric equalizer with biquad filters",
        PluginType::Gain => "Simple volume control",
        PluginType::Upmixer => "Stereo to surround upmixing (FFT-based)",
        PluginType::Compressor => "Dynamic range compression",
        PluginType::Limiter => "Peak limiter to prevent clipping",
        PluginType::Gate => "Noise gate — silences below threshold",
        PluginType::Expander => "Dynamic range expansion",
        PluginType::MultibandCompressor => "Multiband dynamic range compression",
        PluginType::MultibandExpander => "Multiband dynamic range expansion",
        PluginType::LoudnessCompensation => "Equal-loudness contour compensation",
        PluginType::FletcherMunson => "Fletcher-Munson equal-loudness correction",
        PluginType::BinauralDecoder => "HRTF-based binaural rendering",
        PluginType::Convolution => "FFT convolution with impulse response files",
        PluginType::LoudnessMonitor => "EBU R128 loudness measurement",
        PluginType::SpectrumAnalyzer => "FFT-based spectrum analysis",
        PluginType::ChannelMuteSolo => "Per-channel mute, solo, and dim",
        PluginType::Matrix => "Channel matrix mixing with gain control",
        PluginType::XTC => "Crosstalk cancellation for speakers",
        PluginType::Denoiser => "Audio denoising (spectral subtraction)",
        PluginType::Pnd => "Perceptual noise diffusion",
        PluginType::ABCompare => "A/B comparison between two signal paths",
        PluginType::BandSplit => "Split signal into frequency bands",
        PluginType::BandMerge => "Merge frequency bands back together",
        PluginType::Downmix => "Downmix multichannel to stereo",
        PluginType::MonoToStereo => "Convert mono to stereo",
        PluginType::Crossfeed => "Headphone crossfeed for natural imaging",
        PluginType::Delay => "Audio delay with feedback control",
        PluginType::Aec => "Acoustic echo cancellation",
        PluginType::Beamformer => "Microphone array beamforming",
        PluginType::AmbisonicsDecoder => "Ambisonics to speaker layout decoder",
        PluginType::StereoImager => "Multi-band M/S stereo width control",
        PluginType::DeEsser => "Sibilance reduction (de-esser)",
        PluginType::TransientShaper => "Attack/sustain shaping (SPL Transient Designer)",
        PluginType::Saturation => "Harmonic saturation / exciter with multiple modes",
        PluginType::DynamicEq => "Frequency-selective dynamics (hybrid EQ + compressor)",
        PluginType::LinearPhaseEq => "Parametric EQ with linear-phase FIR convolution",
        PluginType::SpectralCompressor => {
            "Per-bin FFT dynamics processor for surgical spectral compression"
        }
    }
}

/// Convert speaker config string to output channel count
pub(crate) fn speaker_config_to_channels(config: &str) -> usize {
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
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let current_hint = self.state.read(cx).app.current_hint.clone();

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
                                // Allow expanding to fit all channels — no fixed upper bound
                                let new_width = (drag.start_width - delta_x).max(60.0);
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
            // Contextual hint banner (dismissible, only show Studio-relevant hints)
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
                                    view.state.update(cx, |state, _cx| {
                                        state.app.dismiss_hint();
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(crate::components::dialogs::tutorial::render_hint_banner(
                                &hint, &theme, d,
                            )),
                    )
                },
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
        let (
            plugins_data,
            selected_idx,
            theme,
            preset_open,
            preset_list,
            last_loaded_preset,
            has_pending_update,
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
                        p.plugin_type().name().to_string(),
                        p.is_permanent(),
                        graph.is_input_monitor(i),
                        graph.is_output_monitor(i),
                    )
                })
                .collect();
            let preset_open = state.app.plugin_state.plugin_preset_open;
            let preset_list = state.app.plugin_state.plugin_preset_list.clone();
            let last_loaded_preset = state.app.plugin_state.last_loaded_preset.clone();
            let has_pending_update = state.app.plugin_state.pending_plugin_update.is_some();
            (
                plugins,
                state.app.plugin_state.selected_plugin_index,
                state.app.ui_state.theme.clone(),
                preset_open,
                preset_list,
                last_loaded_preset,
                has_pending_update,
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
        let d = Ds::from_cx(cx);

        // Split: main plugins, then "+", then Matrix + output monitor
        // The "+" always appears just before the Matrix plugin.
        let trailing_start = modules_info
            .iter()
            .position(|(_, _, _, _, _, _, pt, _, _, _)| *pt == PluginType::Matrix)
            .unwrap_or(modules_info.len());
        let (main_modules, tail_modules) = if trailing_start < modules_info.len() {
            let (main, tail) = modules_info.split_at(trailing_start);
            (main.to_vec(), tail.to_vec())
        } else {
            (modules_info, vec![])
        };

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
                    .px(d.card)
                    .py(d.pad_y)
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
                            .rounded(d.r_md)
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
                            .gap(d.gap)
                            .child(
                                div()
                                    .text_size(d.text_sm)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .child("SIGNAL CHAIN"),
                            )
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_muted)
                                    .child(format!("{} plugins", plugin_count)),
                            )
                            .when(has_pending_update, |el| {
                                el.child(
                                    div()
                                        .text_size(d.text_xs)
                                        .text_color(theme.accent)
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Applying..."),
                                )
                            }),
                    )
                    // Preset buttons (right-aligned)
                    .child({
                        let state_for_load = self.state.clone();
                        let state_for_save = self.state.clone();
                        let btn_text_muted = text_muted;
                        let btn_surface_hover = surface_hover;

                        div()
                            .flex()
                            .items_center()
                            .gap(d.grid)
                            // Show current preset name if one is loaded
                            .when_some(last_loaded_preset.clone(), |el, name| {
                                el.child(
                                    div()
                                        .text_size(d.text_xs)
                                        .text_color(btn_text_muted)
                                        .child(name),
                                )
                            })
                            // Load button
                            .child(
                                div()
                                    .id("rack-load-preset")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .px(d.pad_y)
                                    .h(rems(1.75))
                                    .cursor_pointer()
                                    .rounded(d.r_md)
                                    .text_size(d.text_xs)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(btn_text_muted)
                                    .hover(move |s| s.bg(btn_surface_hover))
                                    .child("Load")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        move |_, _, cx| {
                                            state_for_load.update(cx, |state, _cx| {
                                                state.app.refresh_plugin_presets();
                                                state.app.input_state.plugin_file_input.clear();
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::LoadPlugins;
                                            });
                                        },
                                    ),
                            )
                            // Save button
                            .child(
                                div()
                                    .id("rack-save-preset")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .px(d.pad_y)
                                    .h(rems(1.75))
                                    .cursor_pointer()
                                    .rounded(d.r_md)
                                    .text_size(d.text_xs)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(btn_text_muted)
                                    .hover(move |s| s.bg(btn_surface_hover))
                                    .child("Save")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        move |_, _, cx| {
                                            state_for_save.update(cx, |state, _cx| {
                                                state.app.refresh_plugin_presets();
                                                state.app.input_state.plugin_file_input.clear();
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::SavePlugins;
                                            });
                                        },
                                    ),
                            )
                    })
            })
            // Plugin modules strip — Ozone-style rack
            .child(
                div()
                    .id("plugin-rack")
                    .flex()
                    .items_center()
                    .gap(d.section)
                    .px(d.card)
                    .py(d.pad_x)
                    .overflow_x_scroll()
                    .min_h(rems(7.0))
                    // Plugin modules - Ozone-style cards with left button column
                    .children(main_modules.into_iter().map(
                        |(idx, color, icon, _name, enabled, is_selected, plugin_type, is_permanent, is_input_mon, is_output_mon)| {
                            let theme_c = theme.clone();
                            let drag_info = PluginDragInfo {
                                source_index: idx,
                                name: short_name_with_permanent(&plugin_type, is_input_mon, is_output_mon, is_permanent).to_string(),
                                color,
                                icon,
                                surface: theme_c.surface,
                                text_on_accent: theme_c.text_on_accent,
                            };
                            let drop_highlight = theme_c.drag_over_highlight;
                            let drop_border = theme_c.drag_over_border;
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
                                                    state.app.plugin_state.graph.len();
                                                let source_is_monitor = state
                                                    .app
                                                    .plugin_state.graph
                                                    .get_plugin(source)
                                                    .map(|p| matches!(p.plugin_type(), PluginType::LoudnessMonitor))
                                                    .unwrap_or(false);
                                                if source_is_monitor && target != 0 && target != chain_len - 1 {
                                                    return;
                                                }
                                                state.app.plugin_state.graph.move_plugin(source, target);
                                                state.app.plugin_state.selected_plugin_index = target;
                                                state.app.plugin_state.graph.update_channel_dependent_plugins();
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
                                        .py(d.pad_y_half)
                                        .px(px(4.0))
                                        .gap(d.grid)
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
                                                .child(if enabled { "A" } else { "B" })
                                                .tooltip({
                                                    let theme = theme_c.clone();
                                                    let label = if enabled { "Bypass" } else { "Activate" };
                                                    move |_window, cx| make_tooltip(label, &theme, cx)
                                                }),
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
                                                .child("S")
                                                .tooltip({
                                                    let theme = theme_c.clone();
                                                    move |_window, cx| make_tooltip("Solo", &theme, cx)
                                                }),
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
                                                .border_color(if preset_open == Some(idx) { accent_color } else { theme_c.text_muted })
                                                .when(preset_open == Some(idx), |d| d.bg(accent_color).text_color(theme_c.text_on_accent))
                                                .when(preset_open != Some(idx), |d| d.text_color(theme_c.text_muted))
                                                .hover(|s| s.border_color(accent_color))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |view, _e: &MouseUpEvent, _, cx| {
                                                            cx.stop_propagation();
                                                            view.state.update(cx, |state, _cx| {
                                                                let ps = &mut state.app.plugin_state;
                                                                if ps.plugin_preset_open == Some(idx) {
                                                                    // Close
                                                                    ps.plugin_preset_open = None;
                                                                    ps.plugin_preset_save_mode = false;
                                                                    ps.plugin_preset_input.clear();
                                                                    ps.confirm_delete_preset = None;
                                                                } else {
                                                                    // Open and populate list
                                                                    if let Some(plugin) = ps.graph.get_plugin(idx) {
                                                                        let pt = plugin.plugin_type();
                                                                        ps.plugin_preset_list = sotf_audio_player::PluginController::list_plugin_presets(&pt);
                                                                    }
                                                                    ps.plugin_preset_open = Some(idx);
                                                                    ps.plugin_preset_save_mode = false;
                                                                    ps.plugin_preset_input.clear();
                                                                }
                                                            });
                                                            cx.notify();
                                                        },
                                                    ),
                                                )
                                                .text_size(rems(0.5625))
                                                .font_weight(FontWeight::BOLD)
                                                .child("P")
                                                .tooltip({
                                                    let theme = theme_c.clone();
                                                    move |_window, cx| make_tooltip("Presets", &theme, cx)
                                                }),
                                        )
                                        // X (Remove) or lock icon for permanent
                                        .when(!is_permanent, |d| {
                                            let theme_tt = theme_c.clone();
                                            let confirming = self.state.read(cx).app.plugin_state.confirm_remove_plugin == Some(idx);
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
                                                    .when(confirming, |d| d.bg(theme_c.error).text_color(theme_c.text_on_accent))
                                                    .when(!confirming, |d| d.hover(|s| s.bg(theme_c.error).text_color(theme_c.text_on_accent)))
                                                    .text_size(rems(0.625))
                                                    .when(!confirming, |d| d.text_color(theme_c.text_muted))
                                                    .font_weight(FontWeight::BOLD)
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |view, _e: &MouseUpEvent, _, cx| {
                                                                cx.stop_propagation();
                                                                view.state.update(cx, |state, _cx| {
                                                                    if state.app.plugin_state.confirm_remove_plugin == Some(idx) {
                                                                        // Second click: confirmed
                                                                        state.app.plugin_state.confirm_remove_plugin = None;
                                                                        state.app.remove_plugin(idx);
                                                                        state.app.update_level_meter_groups();
                                                                    } else {
                                                                        // First click: ask for confirmation
                                                                        state.app.plugin_state.confirm_remove_plugin = Some(idx);
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
                                            .py(px(2.0))
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
                                                        .gap(px(2.0))
                                                        .px(d.grid)
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
                                                                                ps.pending_plugin_update = Some(PluginUpdateType::Structural);
                                                                            }
                                                                            Err(e) => {
                                                                                log::error!("Failed to load preset: {e}");
                                                                            }
                                                                        }
                                                                        ps.plugin_preset_open = None;
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
                                                            let confirming_del = self.state.read(cx).app.plugin_state.confirm_delete_preset.as_ref()
                                                                .is_some_and(|(pi_idx, pn)| *pi_idx == idx && pn == name);
                                                            let name_confirm = name.clone();
                                                            div()
                                                                .id(("preset-del", pi))
                                                                .text_size(rems(0.5))
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
                                                                                let is_confirmed = ps.confirm_delete_preset.as_ref()
                                                                                    .is_some_and(|(pi_idx, pn)| *pi_idx == idx && *pn == name_del);
                                                                                if is_confirmed {
                                                                                    // Second click: confirmed
                                                                                    ps.confirm_delete_preset = None;
                                                                                    if let Some(plugin) = ps.graph.get_plugin(idx) {
                                                                                        let pt = plugin.plugin_type().clone();
                                                                                        let _ = sotf_audio_player::PluginController::delete_plugin_preset(&pt, &name_del);
                                                                                        ps.plugin_preset_list = sotf_audio_player::PluginController::list_plugin_presets(&pt);
                                                                                    }
                                                                                } else {
                                                                                    // First click: ask for confirmation
                                                                                    ps.confirm_delete_preset = Some((idx, name_confirm.clone()));
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
                                                let save_mode = self.state.read(cx).app.plugin_state.plugin_preset_save_mode;
                                                let theme_s = theme_p.clone();
                                                if save_mode {
                                                    // Text input + confirm button
                                                    let preset_name = self.state.read(cx).app.plugin_state.plugin_preset_input.clone();
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(2.0))
                                                        .mt(px(2.0))
                                                        .child({
                                                            let state_for_text = self.state.clone();
                                                            div()
                                                                .flex_1()
                                                                .child(
                                                                    gpui_ui_kit::Input::new("preset-name-input")
                                                                        .value(gpui::SharedString::from(preset_name))
                                                                        .placeholder("Preset name...")
                                                                        .size(gpui_ui_kit::InputSize::Xs)
                                                                        .bg_color(theme_s.background_secondary)
                                                                        .on_text_change(move |text, _window, cx| {
                                                                            state_for_text.update(cx, |state, _cx| {
                                                                                state.app.plugin_state.plugin_preset_input = text;
                                                                            });
                                                                        }),
                                                                )
                                                        })
                                                        .child(
                                                            div()
                                                                .id(("preset-confirm", idx))
                                                                .px(d.grid)
                                                                .py(px(2.0))
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
                                                                            let name = ps.plugin_preset_input.trim().to_string();
                                                                            if !name.is_empty() {
                                                                                match ps.save_plugin_preset(idx, &name) {
                                                                                    Ok(_) => {
                                                                                        if let Some(plugin) = ps.graph.get_plugin(idx) {
                                                                                            let pt = plugin.plugin_type().clone();
                                                                                            ps.plugin_preset_list = sotf_audio_player::PluginController::list_plugin_presets(&pt);
                                                                                        }
                                                                                    }
                                                                                    Err(e) => {
                                                                                        log::error!("Failed to save preset: {e}");
                                                                                    }
                                                                                }
                                                                                ps.plugin_preset_save_mode = false;
                                                                                ps.plugin_preset_input.clear();
                                                                            }
                                                                        });
                                                                        cx.notify();
                                                                    }),
                                                                )
                                                                .child("OK"),
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
                                                        .py(px(2.0))
                                                        .mt(px(2.0))
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
                                                                    let n = ps.plugin_preset_list.len() + 1;
                                                                    ps.plugin_preset_input = format!("Preset {n}");
                                                                    ps.plugin_preset_save_mode = true;
                                                                });
                                                                cx.notify();
                                                            }),
                                                        )
                                                        .child("+ Save")
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
                                                    .child(short_name_with_permanent(&plugin_type, is_input_mon, is_output_mon, is_permanent)),
                                            )
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
                                            .when(!is_permanent, |el| {
                                                el.child(
                                                    div()
                                                        .flex()
                                                        .justify_end()
                                                        .px(d.grid)
                                                        .pb(px(2.0))
                                                        .text_size(rems(0.5))
                                                        .text_color(theme_c.text_muted)
                                                        .child(":::"),
                                                )
                                            })
                                    }
                                })
                        },
                    ))
                    // "+" Add plugin slot (visually distinct from solid plugin boxes)
                    .child({
                        let theme_add = theme.clone();
                        let drop_highlight = theme.drag_over_highlight;
                        let drop_border = theme.drag_over_border;

                        div()
                            .id("plugin-add-slot")
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
                                            return;
                                        }
                                        state.app.plugin_state.graph.move_plugin(source, target);
                                        state.app.plugin_state.selected_plugin_index = target;
                                        state.app.plugin_state.graph.update_channel_dependent_plugins();
                                        state.app.plugin_state.pending_plugin_update =
                                            Some(PluginUpdateType::Structural);
                                        state.app.update_level_meter_groups();
                                    });
                                    cx.notify();
                                },
                            ))
                            .child("+")
                    })
                    // Trailing permanent plugins (output matrix/monitor) after "+"
                    .children(tail_modules.into_iter().map(
                        |(idx, color, icon, _name, enabled, is_selected, plugin_type, is_permanent, is_input_mon, is_output_mon)| {
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
                                    let drop_highlight = theme_c.drag_over_highlight;
                                    let drop_border = theme_c.drag_over_border;
                                    move |style, _, _, _| {
                                        style.bg(drop_highlight).border_color(drop_border)
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
                                                state.app.plugin_state.graph.move_plugin(source, target);
                                                state.app.plugin_state.selected_plugin_index = target;
                                                state.app.plugin_state.graph.update_channel_dependent_plugins();
                                                state.app.plugin_state.pending_plugin_update =
                                                    Some(PluginUpdateType::Structural);
                                                state.app.update_level_meter_groups();
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
                                        .gap(px(2.0))
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
                                                .text_size(rems(0.625))
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
                                        .when(!is_permanent, |d| {
                                            let theme_tt = theme_c.clone();
                                            let confirming = self.state.read(cx).app.plugin_state.confirm_remove_plugin == Some(idx);
                                            d.child(
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
                                                    .text_size(rems(0.625))
                                                    .when(!confirming, |d| d.text_color(theme_c.text_muted))
                                                    .font_weight(FontWeight::BOLD)
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |view, _e: &MouseUpEvent, _, cx| {
                                                                cx.stop_propagation();
                                                                view.state.update(cx, |state, _cx| {
                                                                    if state.app.plugin_state.confirm_remove_plugin == Some(idx) {
                                                                        state.app.plugin_state.confirm_remove_plugin = None;
                                                                        state.app.remove_plugin(idx);
                                                                        state.app.update_level_meter_groups();
                                                                    } else {
                                                                        state.app.plugin_state.confirm_remove_plugin = Some(idx);
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
                                                    .text_size(rems(0.5))
                                                    .text_color(theme_c.text_muted)
                                                    .child("🔒"),
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
                                                .child(short_name_with_permanent(&plugin_type, is_input_mon, is_output_mon, is_permanent)),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(rems(2.5))
                                                .text_color(color)
                                                .child(icon),
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
                                .child("Click + to add plugins"),
                        )
                    }),
            )
            // Add plugin menu (shown when "+" is clicked)
            .when(self.state.read(cx).app.show_add_plugin_menu, |el| {
                el.child(
                    div()
                        .id("add-plugin-menu")
                        .px(d.card)
                        .py(d.card)
                        .max_h(px(400.0))
                        .overflow_y_scroll()
                        .bg(theme.surface)
                        .border_t_1()
                        .border_color(theme.border)
                        .child(self.render_add_plugin_buttons(cx)),
                )
            })
    }
    /// Render chain-level control buttons (Bypass, AutoGain, Mono, M/S) in a 2×2 grid
    pub(crate) fn render_chain_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let chain_bypass = state.app.plugin_state.chain_bypass;
        let chain_autogain = state.app.plugin_state.chain_autogain;
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
                .py(px(4.0))
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
            .gap(px(2.0))
            .p(d.pad_y)
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
pub(crate) fn build_controller_overlay(controller_id: &str, engine: &MidiMappingEngine) -> MidiOverlay {
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
