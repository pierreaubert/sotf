//! Crossfeed Plugin UI Component
//!
//! Headphone crossfeed for speaker-like listening:
//! - Mode selector: Off, Bauer, Meier, Multiband
//! - Preset dropdown for quick configuration
//! - Per-mode parameters shown conditionally
//! - Auto gain compensation

use super::common::{render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Select, SelectOption, SelectSize};
use sotf_plugins::param_specs::{crossfeed::PARAMS as CF, find_by_key as pk};
use sotf_plugins::{CrossfeedMode, CrossfeedPreset};

/// State for rendering the Crossfeed plugin
pub struct CrossfeedRenderState {
    pub mode: CrossfeedMode,
    pub preset: CrossfeedPreset,
    pub enabled: bool,
    pub mix: f64,
    // Bauer
    pub bauer_fcut_hz: f64,
    pub bauer_feed_db: f64,
    // Meier
    pub meier_level: f64,
    // Multiband
    pub mb_low_freq_hz: f64,
    pub mb_mid_high_freq_hz: f64,
    pub mb_low_feed_db: f64,
    pub mb_mid_feed_db: f64,
    pub mb_high_feed_db: f64,
    // Auto gain
    pub autogain_enabled: bool,
    pub autogain_target_lufs: f64,
    pub autogain_max_gain_db: f64,
    pub autogain_smoothing_ms: f64,
    // UI state
    pub is_editing: bool,
    pub selected_param: usize,
    pub preset_select_open: bool,
}

/// Render the Crossfeed plugin
pub fn render_crossfeed_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: CrossfeedRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let current_mode = state.mode;

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Row 1: Mode selector + Preset + Enable + Mix
        .child(
            div()
                .flex()
                .gap_6()
                .items_start()
                // Column 1: Mode + Preset + Enabled + Mix
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(render_section_title("GENERAL", theme))
                        // Mode selector (param 0) - segmented buttons
                        .child(render_mode_selector(
                            entity.clone(),
                            plugin_idx,
                            current_mode,
                            0,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        // Preset dropdown (param 1)
                        .child(render_preset_selector(
                            entity.clone(),
                            plugin_idx,
                            state.preset,
                            state.preset_select_open,
                            1,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        // Enabled (param 2)
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Enabled",
                            state.enabled,
                            2,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        // Mix (param 3)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix * 100.0,
                            pk(CF, "mix").min_f64() * 100.0,
                            pk(CF, "mix").max_f64() * 100.0,
                            "%",
                            3,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 2: Mode-specific parameters
                .when(current_mode == CrossfeedMode::Bauer, |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(render_section_title("BAUER", theme))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Fcut",
                                state.bauer_fcut_hz,
                                pk(CF, "bauer_fcut_hz").min_f64(),
                                pk(CF, "bauer_fcut_hz").max_f64(),
                                "Hz",
                                4,
                                state.selected_param,
                                state.is_editing,
                                Some('f'),
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Feed",
                                state.bauer_feed_db,
                                pk(CF, "bauer_feed_db").min_f64(),
                                pk(CF, "bauer_feed_db").max_f64(),
                                "dB",
                                5,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            )),
                    )
                })
                .when(current_mode == CrossfeedMode::Meier, |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(render_section_title("MEIER", theme))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Level",
                                state.meier_level,
                                pk(CF, "meier_level").min_f64(),
                                pk(CF, "meier_level").max_f64(),
                                "%",
                                6,
                                state.selected_param,
                                state.is_editing,
                                Some('l'),
                                theme,
                            )),
                    )
                })
                .when(current_mode == CrossfeedMode::Mb, |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(render_section_title("MULTIBAND", theme))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Low Freq",
                                state.mb_low_freq_hz,
                                pk(CF, "mb_low_freq_hz").min_f64(),
                                pk(CF, "mb_low_freq_hz").max_f64(),
                                "Hz",
                                7,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Mid-Hi Freq",
                                state.mb_mid_high_freq_hz,
                                pk(CF, "mb_mid_high_freq_hz").min_f64(),
                                pk(CF, "mb_mid_high_freq_hz").max_f64(),
                                "Hz",
                                8,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Low Feed",
                                state.mb_low_feed_db,
                                pk(CF, "mb_low_feed_db").min_f64(),
                                pk(CF, "mb_low_feed_db").max_f64(),
                                "dB",
                                9,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Mid Feed",
                                state.mb_mid_feed_db,
                                pk(CF, "mb_mid_feed_db").min_f64(),
                                pk(CF, "mb_mid_feed_db").max_f64(),
                                "dB",
                                10,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "High Feed",
                                state.mb_high_feed_db,
                                pk(CF, "mb_high_feed_db").min_f64(),
                                pk(CF, "mb_high_feed_db").max_f64(),
                                "dB",
                                11,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            )),
                    )
                })
                // Column 3: Auto Gain (always visible)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("AUTO GAIN", theme))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Auto Gain",
                            state.autogain_enabled,
                            12,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Target",
                            state.autogain_target_lufs,
                            pk(CF, "autogain_target_lufs").min_f64(),
                            pk(CF, "autogain_target_lufs").max_f64(),
                            "LUFS",
                            13,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Max Gain",
                            state.autogain_max_gain_db,
                            pk(CF, "autogain_max_gain_db").min_f64(),
                            pk(CF, "autogain_max_gain_db").max_f64(),
                            "dB",
                            14,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Smoothing",
                            state.autogain_smoothing_ms,
                            pk(CF, "autogain_smoothing_ms").min_f64(),
                            pk(CF, "autogain_smoothing_ms").max_f64(),
                            "ms",
                            15,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
}

/// Render mode selector as segmented buttons
fn render_mode_selector(
    entity: Entity<AppState>,
    plugin_idx: usize,
    current_mode: CrossfeedMode,
    param_idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = is_editing && selected_param == param_idx;
    let modes = [
        (CrossfeedMode::Off, "Off", 0usize),
        (CrossfeedMode::Bauer, "Bauer", 1),
        (CrossfeedMode::Meier, "Meier", 2),
        (CrossfeedMode::Mb, "Multiband", 3),
    ];

    let border_color = if is_selected {
        theme.accent
    } else {
        theme.border
    };
    let surface_color = theme.surface;
    let accent_color = theme.accent;
    let text_on_accent = theme.text_on_accent;
    let text_primary = theme.text_primary;
    let text_muted = theme.text_muted;

    let mut row = div()
        .id("crossfeed-mode-selector")
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if is_selected {
                    accent_color
                } else {
                    text_muted
                })
                .child("Mode"),
        )
        .child({
            let mut btn_row = div()
                .flex()
                .rounded_lg()
                .border_1()
                .border_color(border_color)
                .overflow_hidden();

            for (mode, label, idx) in modes {
                let is_active = current_mode == mode;
                let entity_c = entity.clone();
                btn_row = btn_row.child(
                    div()
                        .id(("mode-btn", idx))
                        .cursor_pointer()
                        .px_3()
                        .py(px(4.0))
                        .text_xs()
                        .font_weight(if is_active {
                            FontWeight::BOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .when(is_active, |d| d.bg(accent_color).text_color(text_on_accent))
                        .when(!is_active, |d| d.bg(surface_color).text_color(text_primary))
                        .on_click(move |_, _window, cx| {
                            entity_c.update(cx, |state, _| {
                                state
                                    .app
                                    .set_plugin_param(plugin_idx, param_idx, idx as f64);
                            });
                        })
                        .child(label),
                );
            }

            btn_row
        });

    // Make the whole mode selector clickable to select this param
    let entity_for_select = entity.clone();
    row = row.on_click(move |_, _window, cx| {
        entity_for_select.update(cx, |state, _| {
            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
            state.app.plugin_state.plugin_param_selection = param_idx;
        });
    });

    row
}

/// Render preset selector as a dropdown
fn render_preset_selector(
    entity: Entity<AppState>,
    plugin_idx: usize,
    current_preset: CrossfeedPreset,
    is_open: bool,
    param_idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = is_editing && selected_param == param_idx;
    let accent_color = theme.accent;
    let text_muted = theme.text_muted;

    let presets = [
        (CrossfeedPreset::Default, "Default"),
        (CrossfeedPreset::Cmoy, "Cmoy"),
        (CrossfeedPreset::Meier, "Meier"),
        (CrossfeedPreset::Mb, "Multiband"),
        (CrossfeedPreset::Off, "Off"),
    ];

    let selected_label = presets
        .iter()
        .find(|(p, _)| *p == current_preset)
        .map(|(_, l)| l.to_string())
        .unwrap_or_else(|| format!("{:?}", current_preset));

    let entity_for_select = entity.clone();

    div()
        .id("crossfeed-preset-selector")
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if is_selected {
                    accent_color
                } else {
                    text_muted
                })
                .child("Preset"),
        )
        .child(
            div().w(px(140.0)).child(
                Select::new("crossfeed-preset-select")
                    .options(
                        presets
                            .iter()
                            .map(|(_, label)| {
                                SelectOption::new(label.to_string(), label.to_string())
                            })
                            .collect(),
                    )
                    .selected(selected_label)
                    .is_open(is_open)
                    .size(SelectSize::Sm)
                    .theme(theme.to_select_theme())
                    .on_toggle({
                        let entity = entity.clone();
                        move |is_open, _window, cx| {
                            entity.update(cx, |state, cx| {
                                state.app.crossfeed_preset_select_open = is_open;
                                cx.notify();
                            });
                        }
                    })
                    .on_change({
                        let entity = entity.clone();
                        move |value, _, cx| {
                            let idx = presets
                                .iter()
                                .position(|(_, l)| *l == value.as_ref())
                                .unwrap_or(0);
                            entity.update(cx, |state, _| {
                                state
                                    .app
                                    .set_plugin_param(plugin_idx, param_idx, idx as f64);
                                state.app.crossfeed_preset_select_open = false;
                            });
                        }
                    }),
            ),
        )
        .on_click(move |_, _window, cx| {
            entity_for_select.update(cx, |state, _| {
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                state.app.plugin_state.plugin_param_selection = param_idx;
            });
        })
}
