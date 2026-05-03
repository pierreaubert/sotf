//! Plugin screen rendering functions - Professional DAW-style interface

// intentional-file: rack with embedded meters and pixel-exact dividers

use super::actions::ToggleUpmixerConfig;
use super::level_meters::{db_to_position, render_gradient_meter};
use super::render_plugin_content;
use crate::app::constants::spacing;
use crate::app::state::plugin::{PluginUiView, available_controllers};
use crate::app::state::{DividerDragState, DividerType};
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
use sotf_plugins::param_specs::{find_by_key as pk, upmixer::PARAMS as UP};

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

pub(crate) fn short_name(
    plugin_type: &PluginType,
    is_input_mon: bool,
    is_output_mon: bool,
) -> &'static str {
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
        PluginType::AAE => "Active acoustic enhancement (LARES-inspired reverb)",
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
        PluginType::Declick => "Time-domain click and transient repair",
        PluginType::HissReducer => "Stationary high-frequency hiss reducer",
        PluginType::SpeechDenoiser => "RNNoise voice denoiser",
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
                let knob_drag = state_read.app.knob_drag;

                // Handle divider dragging
                let divider_drag = state_read.app.dragging_divider.clone();

                if let Some(drag) = knob_drag {
                    let mouse_y: f32 = event.position.y.into();
                    let delta_y = drag.start_y - mouse_y; // Inverted: up = positive (increase)
                    // Scale: 150px drag = full range
                    let range = drag.max - drag.min;
                    let value_delta = (delta_y as f64 / 150.0) * range;
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
                        if state.app.knob_drag.is_some() {
                            state.app.knob_drag = None;
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
                    d
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
                },
            )
    }

    /// Render the empty-state notice shown in Studio when the plugin graph
    /// has a non-linear topology that the rack strip can't render.
    fn render_non_linear_chain_notice(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_ui_kit::{
            Button, ButtonVariant, Card, StackSpacing, Text, TextSize, TextWeight, VStack,
        };
        let d = Ds::from_cx(cx);
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
                        Text::new("Signal chain uses graph routing")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new(
                                    "The current plugin chain has parallel paths or routed \
                                     bass management and can't be shown in the linear rack. \
                                     Open the graph view to inspect, edit, and remove plugins.",
                                )
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("open_graph_view", "Open Graph View")
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
                                        .px(spacing::SM)
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
                                            .py(spacing::XS)
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
                                                        .gap(spacing::XS)
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
                                                        .gap(spacing::XS)
                                                        .mt(spacing::XS)
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
                                                                .py(spacing::XS)
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
                                                        .py(spacing::XS)
                                                        .mt(spacing::XS)
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
                                                        .pb(spacing::XS)
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
                                        .gap(spacing::XS)
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
                    .gap(spacing::NONE)
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
                            .p(spacing::XS)
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
    fn render_side_meter_legend(
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
            .p(spacing::XS)
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
                div()
                    .text_size(d.text_xs)
                    .mt(d.grid)
                    .opacity(0.0)
                    .child("X"), // Invisible spacer
            )
    }

    /// Render add plugin buttons grouped by category
    fn render_add_plugin_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let release_channel = state.app.ui_state.release_channel;
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
        let mut global_idx = 0usize;

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
                let name = short_name(&pt, false, false);
                let icon = plugin_icon(&pt, false, false);
                let description = plugin_description(&pt);
                let theme_c = theme.clone();
                let text_on_accent = theme_c.text_on_accent;
                let btn_id = global_idx;
                global_idx += 1;

                buttons.push(
                    div()
                        .id(("add-plugin", btn_id))
                        .flex()
                        .items_center()
                        .gap(d.grid)
                        .px(d.pad_y)
                        .py(d.pad_y_half)
                        .rounded(d.r_md)
                        .bg(theme_c.surface)
                        .border_1()
                        .border_color(color)
                        .text_size(d.text_xs)
                        .text_color(color)
                        .cursor_pointer()
                        .hover(move |s| s.bg(color).text_color(text_on_accent))
                        .tooltip({
                            let theme = theme_c.clone();
                            move |_window, cx| make_tooltip(description, &theme, cx)
                        })
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
                        // Icon + name
                        .child(icon)
                        .child(name)
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
                                .text_size(rems(0.625))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_muted)
                                // intentional: fixed 60px label column width for category labels
                                .w(px(60.0))
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
    fn render_plugin_detail_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (plugin_data, selected_idx, editing_idx, param_selection, theme) = {
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
            .when(has_plugin, |el| {
                let plugin = plugin_data.clone().unwrap();
                let plugin_type = plugin.plugin_type().clone();
                let _plugin_name = plugin_type.name().to_string();
                let _color = plugin_color(&plugin_type, &theme);
                let is_editing = editing_idx.is_some();
                let _plugin_enabled = plugin.enabled;

                let plugin_ui_view = self.state.read(cx).app.plugin_state.plugin_ui_view.clone();
                let controller_picker_open = self.state.read(cx).app.plugin_state.controller_picker_open;
                let state_for_toggle = self.state.clone();

                el.child(
                    // Plugin header bar
                    {
                        div()
                            .flex()
                            .items_center()
                            .gap(d.section)
                            .px(d.card)
                            .py(d.pad_x)
                            .bg(theme.background_secondary)
                            .border_b_1()
                            .border_color(theme.border)
                            // Left-aligned menus
                            .child({
                                // Unified view mode Select dropdown
                                let state_c = state_for_toggle.clone();

                                // Build options: Native UI, each controller, Simple
                                let mut view_options: Vec<gpui_ui_kit::SelectOption> = vec![
                                    gpui_ui_kit::SelectOption::new("ui".to_string(), "Native UI".to_string()),
                                ];
                                for (id, label) in available_controllers() {
                                    view_options.push(gpui_ui_kit::SelectOption::new(
                                        format!("ctrl:{id}"), label.to_string(),
                                    ));
                                }
                                view_options.push(gpui_ui_kit::SelectOption::new("simple".to_string(), "Simple".to_string()));

                                let selected_value = match &plugin_ui_view {
                                    PluginUiView::UI => "ui".to_string(),
                                    PluginUiView::Simple => "simple".to_string(),
                                    PluginUiView::Controller(name) => format!("ctrl:{name}"),
                                };

                                div().flex().items_center().gap(d.gap)
                                    .child(
                                        div().text_size(d.text_xs).font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text_secondary).child("View"),
                                    )
                                    .child(
                                        div().w(px(130.0)).child(
                                            gpui_ui_kit::Select::new("view-mode-select")
                                                .options(view_options)
                                                .selected(selected_value)
                                                .is_open(controller_picker_open)
                                                .size(gpui_ui_kit::SelectSize::Xs)
                                                .theme(theme.to_select_theme())
                                                .on_toggle({
                                                    let state_c = state_c.clone();
                                                    move |is_open, _window, cx| {
                                                        state_c.update(cx, |s, cx| {
                                                            s.app.plugin_state.controller_picker_open = is_open;
                                                            cx.notify();
                                                        });
                                                    }
                                                })
                                                .on_change({
                                                    let state_c = state_c.clone();
                                                    move |value, _, cx| {
                                                        let view = match value.as_ref() {
                                                            "ui" => PluginUiView::UI,
                                                            "simple" => PluginUiView::Simple,
                                                            v if v.starts_with("ctrl:") => {
                                                                PluginUiView::Controller(v[5..].to_string())
                                                            }
                                                            _ => PluginUiView::UI,
                                                        };
                                                        state_c.update(cx, |s, _| {
                                                            s.app.plugin_state.plugin_ui_view = view;
                                                            s.app.plugin_state.controller_picker_open = false;
                                                        });
                                                    }
                                                }),
                                        ),
                                    )
                            })
                    }
                    // Output config dropdown (only for Upmixer) — next to View, left-aligned
                    .when(matches!(plugin_type, PluginType::Upmixer), |el| {
                        let state_c = state_for_toggle.clone();
                        let upmixer_speaker_config = {
                            let st = state_c.read(cx);
                            if let Some(p) = st.app.plugin_state.graph.get_plugin(selected_idx) {
                                if let sotf_audio_player::PluginSettings::Upmixer { ref speaker_config, .. } = p.settings {
                                    speaker_config.clone()
                                } else { "5.1".to_string() }
                            } else { "5.1".to_string() }
                        };
                        let upmixer_config_open = state_c.read(cx).app.upmixer_config_open;
                        let theme2 = theme.clone();
                        el.child(
                            div().flex().items_center().gap(d.gap)
                                .child(
                                    div().text_size(d.text_xs).font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme2.text_secondary).child("Output"),
                                )
                                .child(
                                    div().w(px(80.0)).child(
                                        gpui_ui_kit::Select::new("rack-config-select")
                                            .options(
                                                pk(UP, "speaker_config").choice_labels()
                                                    .iter()
                                                    .map(|c| gpui_ui_kit::SelectOption::new(c.to_string(), c.to_string()))
                                                    .collect(),
                                            )
                                            .selected(upmixer_speaker_config)
                                            .is_open(upmixer_config_open)
                                            .size(gpui_ui_kit::SelectSize::Xs)
                                            .theme(theme2.to_select_theme())
                                            .on_toggle({
                                                let state_c = state_c.clone();
                                                move |is_open, _window, cx| {
                                                    state_c.update(cx, |state, cx| {
                                                        state.app.upmixer_config_open = is_open;
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                            .on_change({
                                                let state_c = state_c.clone();
                                                move |value, _, cx| {
                                                    let configs = pk(UP, "speaker_config").choice_labels();
                                                    let idx = configs.iter().position(|c| *c == value.as_ref()).unwrap_or(0);
                                                    state_c.update(cx, |state, _| {
                                                        state.app.set_plugin_param(selected_idx, 0, idx as f64); // param 0 = speaker_config
                                                        state.app.upmixer_config_open = false;
                                                        state.app.update_level_meter_groups();
                                                    });
                                                }
                                            }),
                                    ),
                                )
                        )
                    }),
                    /* row with infos but not useful
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
                    for p in state.app.plugin_state.graph.plugins() {
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
                    // Compute minimum meter width based on actual element sizes:
                    // Each meter bar: 1rem (16px) + 1px gap between bars
                    // Each group: 2×2px padding (p_0p5) = 4px
                    // Legend: ~16px
                    // Input group: 2 bars (L/R) = 2×16 + 1 gap + 4 padding = 37px
                    let num_output_groups = state.app.level_meters.groups.len();
                    let meter_bar_width: f32 = 16.0; // 1rem
                    let bar_gap: f32 = 1.0; // gap_px()
                    let group_padding: f32 = 4.0; // p_0p5 = 2px each side
                    let legend_width: f32 = 16.0;
                    let input_width: f32 = 2.0 * meter_bar_width + bar_gap + group_padding;
                    let output_width: f32 = output_channels as f32 * (meter_bar_width + bar_gap)
                        + num_output_groups as f32 * group_padding;
                    let min_meter_width = input_width + legend_width + output_width + 8.0; // 8px outer margin

                    let divider_theme = PaneDividerTheme {
                        background: theme.background,
                        background_hover: theme.surface_hover,
                        background_collapsed: theme.surface,
                        foreground: theme.text_muted,
                        foreground_hover: theme.text_secondary,
                        border: theme.border,
                    };

                    let output_collapsed = state.app.output_meter_collapsed;
                    // Auto-fit: default to min width, allow user to expand up to 2x
                    let max_meter_width = min_meter_width * 2.0;
                    // If stored width is below minimum (e.g. channel count increased), snap to min
                    let output_meter_width = if state.app.output_meter_width < min_meter_width {
                        min_meter_width
                    } else {
                        state.app.output_meter_width.min(max_meter_width)
                    };

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
                                .p(d.card)
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
                                    let plugin_graph = app_st.app.plugin_state.graph.clone();
                                    let midi_overlay = app_st.app.plugin_state.midi_mapping.build_overlay(&[]);
                                    let plugin_ui_view = app_st.app.plugin_state.plugin_ui_view.clone();

                                    let midi_ref = if midi_overlay.has_controller() {
                                        Some(midi_overlay)
                                    } else {
                                        None
                                    };

                                    let d = Ds::from_cx(cx);

                                    match &plugin_ui_view {
                                        PluginUiView::Simple => {
                                            super::ui_simple::render_simple_plugin_view(
                                                &d,
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
                                                &plugin_graph,
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
                                                &plugin_graph,
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
                        .child("No plugin selected")
                        .child(div().text_size(d.text_sm).child("Add a plugin to get started")),
                )
            })
    }

    /// Render combined IN + OUT meter panel using the same meter components as the main library view.
    /// Uses proper speaker-config-aware channel groups (L/R, Center, LFE, Surrounds, etc.)
    /// instead of a single flat group for all channels.
    fn render_combined_meter(
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
            .bg(theme.background_secondary)
            .border_l_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .gap_0()
                    .flex_1()
                    .min_h(rems(18.75))
                    // Input group (stereo L/R, no M/S/D)
                    .children(in_groups.iter().enumerate().map(|(i, group)| {
                        self.render_meter_group(
                            group,
                            fake_in_idx + i,
                            false,
                            input_loudness.as_ref(),
                            &peak_hold,
                            &theme,
                            cx,
                        )
                        .into_any_element()
                    }))
                    .child(
                        self.render_vertical_legend(&d, &theme, true)
                            .into_any_element(),
                    )
                    // Output groups (real indices → M/S/D connected to matrix plugin)
                    .children(out_groups.iter().enumerate().map(|(i, group)| {
                        self.render_meter_group(
                            group,
                            i, // real index into level_meter_groups
                            false,
                            output_loudness.as_ref(),
                            &peak_hold,
                            &theme,
                            cx,
                        )
                        .into_any_element()
                    })),
            )
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
                .py(spacing::SM)
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
            .gap(spacing::XS)
            .p(d.pad_y)
            .bg(theme.background_secondary)
            .border_l_1()
            .border_t_1()
            .border_color(border)
            // Row 1: Bypass | AutoGain
            .child(
                div()
                    .flex()
                    .gap(spacing::XS)
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
                    .gap(spacing::XS)
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
pub(crate) fn build_controller_overlay(
    controller_id: &str,
    engine: &MidiMappingEngine,
) -> MidiOverlay {
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
