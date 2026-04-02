//! Compressor Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | SETUP            | DYNAMICS              TIMING                | OUTPUT           |
//! |                  |                                            |                  |
//! | [Link Ch] toggle | [Threshold] slider    [Attack] slider      | [GR Meter]       |
//! | [SC HPF]  knob   | [Ratio]     slider    [Release] slider     | [AutoMakeup] tog |
//! |                  | [Knee]      slider                         | [Makeup]   knob  |
//! |                  |                                            | [Mix]      knob  |
//! |                  | ┌─ Transfer Curve ─────────────────┐       |                  |
//! |                  | │                                  │       |                  |
//! |                  | └──────────────────────────────────┘       |                  |
//! +------------------+--------------------------------------------+------------------+

use super::common::{
    render_interactive_transfer_curve, render_knob, render_section_title, render_toggle,
    render_vertical_slider_with_ticks,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::CompressorData;
use sotf_plugins::param_specs::{compressor::PARAMS as CP, find_by_key as pk};

/// State for rendering the Compressor plugin
pub struct CompressorRenderState<'a> {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub knee_db: f64,
    pub makeup_gain_db: f64,
    pub mix: f64,
    pub auto_makeup: bool,
    pub link_channels: bool,
    pub sidechain_hpf_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
    pub data: Option<&'a CompressorData>,
}

// Sidechain HPF UI range
const SIDECHAIN_HPF_UI_MIN: f64 = 40.0;
const SIDECHAIN_HPF_UI_MAX: f64 = 160.0;

// Layout constants
const TRANSFER_CURVE_SIZE: f32 = 200.0;
const SLIDER_HEIGHT: f32 = 180.0;

/// Render the Compressor plugin
pub fn render_compressor_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: CompressorRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Gain reduction value
    let gr_db = if let Some(data) = state.data {
        data.gain_reduction_db
            .iter()
            .cloned()
            .fold(0.0_f32, f32::max) as f64
    } else {
        0.0
    };
    let meter_value = -gr_db;

    // Input level estimate for transfer curve indicator
    let input_level = state.data.map(|d| {
        let max_gr = d.gain_reduction_db.iter().cloned().fold(0.0_f32, f32::max) as f64;
        if max_gr > 0.1 {
            state.threshold_db + max_gr
        } else {
            state.threshold_db - 6.0
        }
    });

    // === LEFT COLUMN: Setup ===
    let setup_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(d.gap_md)
        .child(render_section_title(d, "SETUP", theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "Link Ch",
            state.link_channels,
            8,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "SC HPF",
            state.sidechain_hpf_hz,
            SIDECHAIN_HPF_UI_MIN,
            SIDECHAIN_HPF_UI_MAX,
            "Hz",
            9,
            state.selected_param,
            state.is_editing,
            Some('s'),
            theme,
        ));

    // === CENTER COLUMN: Interactive transfer curve (top) + Sliders (bottom) ===
    let transfer_curve = render_interactive_transfer_curve(
        d,
        entity.clone(),
        plugin_idx,
        state.threshold_db,
        state.ratio,
        state.knee_db,
        false,
        TRANSFER_CURVE_SIZE,
        input_level,
        0, // threshold param idx
        1, // ratio param idx
        pk(CP, "threshold").min_f64(),
        pk(CP, "threshold").max_f64(),
        pk(CP, "ratio").min_f64(),
        pk(CP, "ratio").max_f64(),
        theme,
    );

    // Restructure center: (Dynamics + Transfer Curve) column + Timing column, centered
    let dynamics_col = div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(render_section_title(d, "DYNAMIC", theme))
        .child(
            div()
                .flex()
                .gap(d.gap)
                .child(render_vertical_slider_with_ticks(
                    entity.clone(),
                    plugin_idx,
                    "Threshold",
                    state.threshold_db,
                    pk(CP, "threshold").min_f64(),
                    pk(CP, "threshold").max_f64(),
                    "dB",
                    0,
                    state.selected_param,
                    state.is_editing,
                    Some('t'),
                    SLIDER_HEIGHT,
                    theme,
                ))
                .child(render_vertical_slider_with_ticks(
                    entity.clone(),
                    plugin_idx,
                    "Ratio",
                    state.ratio,
                    pk(CP, "ratio").min_f64(),
                    pk(CP, "ratio").max_f64(),
                    ":1",
                    1,
                    state.selected_param,
                    state.is_editing,
                    Some('r'),
                    SLIDER_HEIGHT,
                    theme,
                ))
                .child(render_vertical_slider_with_ticks(
                    entity.clone(),
                    plugin_idx,
                    "Knee",
                    state.knee_db,
                    pk(CP, "knee").min_f64(),
                    pk(CP, "knee").max_f64(),
                    "dB",
                    4,
                    state.selected_param,
                    state.is_editing,
                    Some('k'),
                    SLIDER_HEIGHT,
                    theme,
                )),
        )
        .child(transfer_curve);

    let timing_col = div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(render_section_title(d, "TIMING", theme))
        .child(
            div()
                .flex()
                .gap(d.gap)
                .child(render_vertical_slider_with_ticks(
                    entity.clone(),
                    plugin_idx,
                    "Attack",
                    state.attack_ms,
                    pk(CP, "attack").min_f64(),
                    pk(CP, "attack").max_f64(),
                    "ms",
                    2,
                    state.selected_param,
                    state.is_editing,
                    Some('a'),
                    SLIDER_HEIGHT,
                    theme,
                ))
                .child(render_vertical_slider_with_ticks(
                    entity.clone(),
                    plugin_idx,
                    "Release",
                    state.release_ms,
                    pk(CP, "release").min_f64(),
                    pk(CP, "release").max_f64(),
                    "ms",
                    3,
                    state.selected_param,
                    state.is_editing,
                    Some('e'),
                    SLIDER_HEIGHT,
                    theme,
                )),
        );

    let center_col = div()
        .flex()
        .flex_shrink_0()
        .gap(d.section)
        .child(dynamics_col)
        .child(timing_col);

    // === RIGHT COLUMN: Output (GR Meter + controls) ===
    let right_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(d.gap_md)
        .child(render_section_title(d, "OUTPUT", theme))
        .child(render_gr_meter(d, meter_value, -30.0, theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "AutoGain",
            state.auto_makeup,
            7,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Makeup",
            state.makeup_gain_db,
            pk(CP, "makeup_gain").min_f64(),
            pk(CP, "makeup_gain").max_f64(),
            "dB",
            5,
            state.selected_param,
            state.is_editing,
            Some('m'),
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Mix",
            state.mix * 100.0,
            pk(CP, "mix").min_f64() * 100.0,
            pk(CP, "mix").max_f64() * 100.0,
            "%",
            6,
            state.selected_param,
            state.is_editing,
            Some('x'),
            theme,
        ));

    // === Main layout: 3 columns, centered ===
    div().w_full().flex().justify_center().p(d.pad_x).child(
        div()
            .flex()
            .gap(d.section)
            .child(setup_col)
            .child(center_col)
            .child(right_col),
    )
}
