//! A/B Compare Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | MIX              | PATH CONFIG                                | AUTO GAIN        |
//! |                  |                                            |                  |
//! | [Mix A/B]  knob  | [Path A Config] file                       | [AutoGain] tog   |
//! | [Mix Mode] choic | [Path B Config] file                       | [Loudness] choic |
//! | [Selected] choic |                                            | [Max AG]   knob  |
//! | [Bypass]   toggl |                                            | [AG Smooth] knob |
//! | [Transition] knob|                                            |                  |
//! +------------------+--------------------------------------------+------------------+

use super::common::{render_knob, render_section_title};
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    ButtonSet, ButtonSetOption, ButtonSetSize, Select, SelectOption, SelectSize, Slider,
};

/// State for rendering the A/B Compare plugin
pub struct ABCompareRenderState<'a> {
    pub mix: f64,
    pub mix_mode: i32,
    pub selected_path: i32,
    pub bypass: bool,
    pub auto_gain_enabled: bool,
    pub loudness_type: i32,
    pub max_auto_gain_db: f64,
    pub gain_smoothing_ms: f64,
    pub mix_transition_ms: f64,
    pub path_a_config: &'a str,
    pub path_b_config: &'a str,
    pub is_editing: bool,
    pub selected_param: usize,
    /// Dropdown open states
    pub path_a_select_open: bool,
    pub path_b_select_open: bool,
}

/// Path config presets
const PATH_PRESETS: &[(&str, &str, &str)] = &[
    ("none", "None", r#"{"type":"None"}"#),
    (
        "eq",
        "EQ",
        r#"{"type":"Plugin","plugin_type":"EQ","parameters":{"filters":[]}}"#,
    ),
    (
        "gain",
        "Gain",
        r#"{"type":"Plugin","plugin_type":"gain","parameters":{"gain_db":0.0}}"#,
    ),
    (
        "comp",
        "Compressor",
        r#"{"type":"Plugin","plugin_type":"compressor","parameters":{"threshold_db":-20.0,"ratio":4.0,"attack_ms":10.0,"release_ms":100.0,"knee_db":3.0,"makeup_gain_db":0.0,"mix":1.0}}"#,
    ),
    (
        "limiter",
        "Limiter",
        r#"{"type":"Plugin","plugin_type":"limiter","parameters":{"threshold_db":-1.0,"release_ms":100.0,"lookahead_ms":5.0,"soft":false,"mix":1.0}}"#,
    ),
    (
        "gate",
        "Gate",
        r#"{"type":"Plugin","plugin_type":"gate","parameters":{"threshold_db":-40.0,"ratio":10.0,"attack_ms":1.0,"hold_ms":50.0,"release_ms":100.0,"mix":1.0}}"#,
    ),
    (
        "expander",
        "Expander",
        r#"{"type":"Plugin","plugin_type":"expander","parameters":{"threshold_db":-40.0,"ratio":2.0,"attack_ms":5.0,"release_ms":50.0,"range_db":20.0,"knee_db":3.0,"hysteresis_db":2.0,"hold_ms":10.0,"mix":1.0}}"#,
    ),
    (
        "denoiser",
        "Denoiser",
        r#"{"type":"Plugin","plugin_type":"denoiser","parameters":{"reduction_db":12.0,"floor_db":-60.0,"smoothing":0.5,"attack_ms":5.0,"release_ms":50.0}}"#,
    ),
    (
        "loudness",
        "Loudness Comp",
        r#"{"type":"Plugin","plugin_type":"loudness_compensation","parameters":{"low_freq":100.0,"low_gain":3.0,"high_freq":8000.0,"high_gain":2.0}}"#,
    ),
];

/// Get preset value from config JSON
fn config_to_preset_value(config: &str) -> &'static str {
    for (value, _, json) in PATH_PRESETS {
        if config == *json {
            return value;
        }
    }
    if config.is_empty() || config == r#"{"type":"None"}"# {
        "none"
    } else if config.contains(r#""plugin_type":"EQ""#) {
        "eq"
    } else if config.contains(r#""plugin_type":"gain""#) {
        "gain"
    } else if config.contains(r#""plugin_type":"compressor""#) {
        "comp"
    } else if config.contains(r#""plugin_type":"limiter""#) {
        "limiter"
    } else if config.contains(r#""plugin_type":"gate""#) {
        "gate"
    } else if config.contains(r#""plugin_type":"expander""#) {
        "expander"
    } else if config.contains(r#""plugin_type":"denoiser""#) {
        "denoiser"
    } else if config.contains(r#""plugin_type":"loudness_compensation""#) {
        "loudness"
    } else {
        "custom"
    }
}

// Layout constants
const MIX_WIDTH: f32 = 160.0;
const OUTPUT_WIDTH: f32 = 140.0;

/// Render the A/B Compare plugin
pub fn render_ab_compare_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: ABCompareRenderState<'_>,
    theme: &Theme,
) -> impl IntoElement {
    let is_pot_mode = state.mix_mode == 0;

    let anb_selected: SharedString = if state.bypass {
        "N".into()
    } else if state.selected_path == 0 {
        "A".into()
    } else {
        "B".into()
    };

    let path_a_preset = config_to_preset_value(state.path_a_config);
    let path_b_preset = config_to_preset_value(state.path_b_config);

    let mode_selected: SharedString = if is_pot_mode { "MIX A+B".into() } else { "Choice".into() };
    let auto_gain_selected: SharedString = if state.auto_gain_enabled { "ON".into() } else { "OFF".into() };
    let time_selected: SharedString = if state.loudness_type == 0 { "Fast".into() } else { "Slow".into() };

    // === LEFT COLUMN: Mix ===
    let mix_col = div()
        .flex()
        .flex_col()
        .w(px(MIX_WIDTH))
        .gap_3()
        .child(render_section_title("MIX", theme))
        // Mode selector
        .child(
            ButtonSet::new(("mode", plugin_idx))
                .options(vec![
                    ButtonSetOption::new("MIX A+B", "MIX A+B"),
                    ButtonSetOption::new("Choice", "Choice"),
                ])
                .selected(mode_selected)
                .size(ButtonSetSize::Xs)
                .theme(theme.to_button_set_theme())
                .on_change({
                    let entity = entity.clone();
                    move |value, _, cx| {
                        let is_mix = value.as_ref() == "MIX A+B";
                        entity.update(cx, |state, _| {
                            state.app.set_plugin_param(plugin_idx, 1, if is_mix { 0.0 } else { 1.0 });
                        });
                    }
                }),
        )
        // Mix knob (dimmed in binary mode)
        .child(div().when(!is_pot_mode, |d| d.opacity(0.4)).child(
            render_knob(
                entity.clone(), plugin_idx, "Mix", state.mix * 100.0,
                -100.0, 100.0, "%", 0, state.selected_param,
                state.is_editing && is_pot_mode, Some('m'), theme,
            ),
        ))
        // A/N/B buttons (dimmed in pot mode)
        .child(div().when(is_pot_mode, |d| d.opacity(0.4)).child(
            render_anb_buttons(entity.clone(), plugin_idx, anb_selected, is_pot_mode, theme),
        ))
        // Transition knob
        .child(render_horizontal_slider(
            entity.clone(), plugin_idx, "Mix Smooth", state.mix_transition_ms,
            5.0, 500.0, "ms", 8, state.selected_param, state.is_editing, theme,
        ));

    // === CENTER COLUMN: Path Config ===
    let center_col = div()
        .flex()
        .flex_col()
        .flex_1()
        .gap_3()
        .child(render_section_title("PATH CONFIG", theme))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.text_muted).child("PATH A"))
                .child(render_path_selector(
                    entity.clone(), plugin_idx, "a", path_a_preset, 9, state.path_a_select_open, theme,
                )),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.text_muted).child("PATH B"))
                .child(render_path_selector(
                    entity.clone(), plugin_idx, "b", path_b_preset, 10, state.path_b_select_open, theme,
                )),
        );

    // === RIGHT COLUMN: Auto Gain ===
    let right_col = div()
        .flex()
        .flex_col()
        .w(px(OUTPUT_WIDTH))
        .gap_3()
        .child(render_section_title("AUTO GAIN", theme))
        .child(
            ButtonSet::new(("autogain", plugin_idx))
                .options(vec![
                    ButtonSetOption::new("ON", "ON"),
                    ButtonSetOption::new("OFF", "OFF"),
                ])
                .selected(auto_gain_selected)
                .size(ButtonSetSize::Xs)
                .theme(theme.to_button_set_theme())
                .on_change({
                    let entity = entity.clone();
                    move |value, _, cx| {
                        let is_on = value.as_ref() == "ON";
                        entity.update(cx, |state, _| {
                            state.app.set_plugin_param(plugin_idx, 4, if is_on { 1.0 } else { 0.0 });
                        });
                    }
                }),
        )
        .child(
            ButtonSet::new(("time", plugin_idx))
                .options(vec![
                    ButtonSetOption::new("Fast", "Fast"),
                    ButtonSetOption::new("Slow", "Slow"),
                ])
                .selected(time_selected)
                .size(ButtonSetSize::Xs)
                .theme(theme.to_button_set_theme())
                .on_change({
                    let entity = entity.clone();
                    move |value, _, cx| {
                        let is_slow = value.as_ref() == "Slow";
                        entity.update(cx, |state, _| {
                            state.app.set_plugin_param(plugin_idx, 5, if is_slow { 1.0 } else { 0.0 });
                        });
                    }
                }),
        )
        .child(render_knob(
            entity.clone(), plugin_idx, "Max Gain", state.max_auto_gain_db,
            0.0, 24.0, "dB", 6, state.selected_param, state.is_editing, Some('g'), theme,
        ))
        .child(render_horizontal_slider(
            entity.clone(), plugin_idx, "Gain Smooth", state.gain_smoothing_ms,
            10.0, 500.0, "ms", 7, state.selected_param, state.is_editing, theme,
        ));

    // === Main layout: 3 columns ===
    div()
        .flex()
        .gap_4()
        .p_3()
        .w_full()
        .child(mix_col)
        .child(center_col)
        .child(right_col)
}

/// Render a horizontal slider using gpui-ui-kit Slider
fn render_horizontal_slider(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
    idx: usize,
    _selected_param: usize,
    _is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    Slider::new(("slider", plugin_idx * 1000 + idx))
        .value(value as f32)
        .min(min as f32)
        .max(max as f32)
        .label(format!("{} ({})", label, unit))
        .theme(theme.to_slider_theme())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.set_plugin_param(plugin_idx, idx, new_value as f64);
                });
            }
        })
}

/// Render a path config selector dropdown
fn render_path_selector(
    entity: Entity<AppState>,
    plugin_idx: usize,
    path_id: &str,
    current_preset: &str,
    param_idx: usize,
    is_open: bool,
    theme: &Theme,
) -> impl IntoElement {
    let options: Vec<SelectOption> = PATH_PRESETS
        .iter()
        .map(|(value, label, _)| SelectOption::new(*value, *label))
        .collect();

    let select_id = if path_id == "a" { plugin_idx * 2 } else { plugin_idx * 2 + 1 };
    let selected: SharedString = current_preset.to_string().into();
    let is_path_a = path_id == "a";

    Select::new(("path-select", select_id))
        .options(options)
        .selected(selected)
        .size(SelectSize::Xs)
        .placeholder("Select config...")
        .is_open(is_open)
        .theme(theme.to_select_theme())
        .on_toggle({
            let entity = entity.clone();
            move |open, _, cx| {
                entity.update(cx, |state, cx| {
                    if is_path_a {
                        state.app.plugin_state.ab_compare_dropdowns.path_a_open = open;
                    } else {
                        state.app.plugin_state.ab_compare_dropdowns.path_b_open = open;
                    }
                    cx.notify();
                });
            }
        })
        .on_change({
            let entity = entity.clone();
            move |value, _, cx| {
                if let Some((_, _, json)) = PATH_PRESETS.iter().find(|(v, _, _)| *v == value.as_ref()) {
                    entity.update(cx, |state, _| {
                        if is_path_a {
                            state.app.plugin_state.ab_compare_dropdowns.path_a_open = false;
                        } else {
                            state.app.plugin_state.ab_compare_dropdowns.path_b_open = false;
                        }
                        state.app.set_plugin_param_string(plugin_idx, param_idx, json.to_string());
                    });
                }
            }
        })
}

/// Render the A/N/B button group
fn render_anb_buttons(
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected: SharedString,
    disabled: bool,
    theme: &Theme,
) -> impl IntoElement {
    ButtonSet::new(("anb", plugin_idx))
        .options(vec![
            ButtonSetOption::new("A", "A"),
            ButtonSetOption::new("N", "N"),
            ButtonSetOption::new("B", "B"),
        ])
        .selected(selected)
        .size(ButtonSetSize::Xs)
        .disabled(disabled)
        .theme(theme.to_button_set_theme())
        .on_change({
            let entity = entity.clone();
            move |value, _, cx| {
                if disabled { return; }
                entity.update(cx, |state, _| {
                    match value.as_ref() {
                        "A" => {
                            state.app.set_plugin_param(plugin_idx, 1, 1.0);
                            state.app.set_plugin_param(plugin_idx, 3, 0.0);
                            state.app.set_plugin_param(plugin_idx, 2, 0.0);
                            state.app.set_plugin_param(plugin_idx, 0, -100.0);
                        }
                        "N" => {
                            state.app.set_plugin_param(plugin_idx, 1, 1.0);
                            state.app.set_plugin_param(plugin_idx, 3, 1.0);
                        }
                        "B" => {
                            state.app.set_plugin_param(plugin_idx, 1, 1.0);
                            state.app.set_plugin_param(plugin_idx, 3, 0.0);
                            state.app.set_plugin_param(plugin_idx, 2, 1.0);
                            state.app.set_plugin_param(plugin_idx, 0, 100.0);
                        }
                        _ => {}
                    }
                });
            }
        })
}
