//! Plugin rack detail panel — meter rendering, plugin detail view, and add-plugin menu.
//!
//! Extracted from ui_rack.rs for maintainability.

// intentional-file: rack detail view with embedded meters and dividers

use super::level_meters::{db_to_position, render_gradient_meter};
use super::render_plugin_content;
use super::ui_plugin_shell::{plugin_accent_color as plugin_color, plugin_icon};
use crate::app::constants::spacing;
use crate::app::state::plugin::{PluginUiView, available_controllers};
use crate::app::state::{DividerDragState, DividerType};
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{CollapseDirection, PaneDivider, PaneDividerTheme};
use sotf_audio_player::PluginType;
use sotf_plugins::param_specs::{find_by_key as pk, upmixer::PARAMS as UP, aae::PARAMS as AAE_P};

use crate::components::themed_tooltip as make_tooltip;
use super::ui_rack::{build_controller_overlay, plugin_description, short_name, speaker_config_to_channels};

impl PlayerView {
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
    pub(crate) fn render_add_plugin_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

        // Categories with their plugins
        let categories: &[(&str, &[PluginType])] = &[
            (
                "Dynamics",
                &[
                    PluginType::Compressor,
                    PluginType::Limiter,
                    PluginType::Gate,
                    PluginType::Expander,
                    PluginType::MultibandCompressor,
                    PluginType::MultibandExpander,
                    PluginType::TransientShaper,
                    PluginType::DeEsser,
                    PluginType::Saturation,
                    PluginType::DynamicEq,
                    PluginType::LinearPhaseEq,
                    PluginType::SpectralCompressor,
                ],
            ),
            (
                "EQ & Tone",
                &[
                    PluginType::EQ,
                    PluginType::Gain,
                    PluginType::Delay,
                    PluginType::LoudnessCompensation,
                    PluginType::FletcherMunson,
                ],
            ),
            (
                "Denoising",
                &[
                    PluginType::Denoiser,
                    PluginType::Declick,
                    PluginType::HissReducer,
                    PluginType::SpeechDenoiser,
                    PluginType::Aec,
                    PluginType::Pnd,
                ],
            ),
            (
                "Spatial",
                &[
                    PluginType::Upmixer,
                    PluginType::AAE,
                    PluginType::Matrix,
                    PluginType::BinauralDecoder,
                    PluginType::Convolution,
                    PluginType::XTC,
                    PluginType::Crossfeed,
                    PluginType::Beamformer,
                    PluginType::Downmix,
                    PluginType::MonoToStereo,
                ],
            ),
            (
                "Analysis",
                &[
                    PluginType::LoudnessMonitor,
                    PluginType::SpectrumAnalyzer,
                    PluginType::ABCompare,
                ],
            ),
            (
                "Routing",
                &[
                    PluginType::ChannelMuteSolo,
                    PluginType::BandSplit,
                    PluginType::BandMerge,
                ],
            ),
        ];

        let mut category_rows: Vec<gpui::AnyElement> = Vec::new();
        let mut global_idx = 0usize;

        for (cat_name, plugins) in categories {
            let mut buttons: Vec<gpui::AnyElement> = Vec::new();

            for pt in *plugins {
                let pt = pt.clone();
                if !release_channel.allows(pt.maturity()) {
                    continue;
                }
                let is_single_instance =
                    matches!(pt, PluginType::Upmixer | PluginType::AAE | PluginType::BinauralDecoder);
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
    pub(crate) fn render_plugin_detail_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                let Some(plugin) = plugin_data.clone() else {
                    return el;
                };
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
                    // Output config dropdown (Upmixer and AAE) — next to View, left-aligned
                    .when(matches!(plugin_type, PluginType::Upmixer | PluginType::AAE), |el| {
                        let state_c = state_for_toggle.clone();
                        let is_aae = matches!(plugin_type, PluginType::AAE);
                        let specs = if is_aae { AAE_P } else { UP };
                        let upmixer_speaker_config = {
                            let st = state_c.read(cx);
                            if let Some(p) = st.app.plugin_state.graph.get_plugin(selected_idx) {
                                match &p.settings {
                                    sotf_audio_player::PluginSettings::Upmixer { speaker_config, .. }
                                    | sotf_audio_player::PluginSettings::AAE { speaker_config, .. } => speaker_config.clone(),
                                    _ => "5.1".to_string(),
                                }
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
                                                pk(specs, "speaker_config").choice_labels()
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
                                                    let configs = pk(specs, "speaker_config").choice_labels();
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
                                PluginType::Upmixer | PluginType::AAE => {
                                    match &p.settings {
                                        sotf_audio_player::PluginSettings::Upmixer { speaker_config, .. }
                                        | sotf_audio_player::PluginSettings::AAE { speaker_config, .. } => {
                                            output_channels = speaker_config_to_channels(speaker_config);
                                        }
                                        _ => output_channels = 6,
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
                    let num_output_groups = state.app.level_meter_groups.len();
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
                                            } else if monitor_indices
                                                .last()
                                                .is_some_and(|&last| selected_idx == last)
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
        let peak_hold = state.app.level_meter_peak_hold.clone();

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
        let out_groups = state.app.level_meter_groups.clone();

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
}
