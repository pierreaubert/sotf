//! Compressor Plugin UI Component
//!
//! Professional compressor visualization with:
//! - Transfer curve display
//! - Gain reduction meter
//! - Vertical sliders for main dynamics controls
//! - Rotary knobs for secondary parameters

use super::common::{
    render_dynamics_layout, render_knob, render_section_title, render_toggle,
    render_transfer_curve_with_level, render_vertical_slider_with_ticks,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
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

// Sidechain HPF UI range (40-160Hz as per user request)
const SIDECHAIN_HPF_UI_MIN: f64 = 40.0;
const SIDECHAIN_HPF_UI_MAX: f64 = 160.0;

// Column layout constants
const METER_WIDTH: f32 = 180.0; // Width for transfer curve and GR meter
const SLIDER_HEIGHT: f32 = 200.0; // Height for vertical sliders

/// Render the Compressor plugin
pub fn render_compressor_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: CompressorRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Get max gain reduction from all channels
    let gr_db = if let Some(data) = state.data {
        // Find maximum reduction (since GR is positive dB value, we want the max)
        data.gain_reduction_db
            .iter()
            .cloned()
            .fold(0.0_f32, f32::max) as f64
    } else {
        0.0
    };

    // Since gain_reduction_db is stored as the attenuation amount (e.g. 6.0 for -6dB),
    // we want to display it as a negative value for the meter
    let meter_value = -gr_db;

    // Transfer curve with input level indicator
    let input_level = state.data.map(|d| {
        // Estimate input level from GR: if GR = X dB, input is approximately threshold + X dB
        let max_gr = d.gain_reduction_db.iter().cloned().fold(0.0_f32, f32::max) as f64;
        if max_gr > 0.1 { state.threshold_db + max_gr } else { state.threshold_db - 6.0 }
    });

    let transfer_curve = render_transfer_curve_with_level(
        state.threshold_db,
        state.ratio,
        state.knee_db,
        false,
        METER_WIDTH,
        input_level,
        theme,
    );

    // Controls: dynamics + timing + output + setup
    let controls = div()
        .flex()
        .gap_4()
        // Dynamics (Threshold, Ratio, Knee)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title("DYNAMICS", theme))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Threshold", state.threshold_db,
                            pk(CP, "threshold").min_f64(), pk(CP, "threshold").max_f64(),
                            "dB", 0, state.selected_param, state.is_editing, Some('t'), SLIDER_HEIGHT, theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Ratio", state.ratio,
                            pk(CP, "ratio").min_f64(), pk(CP, "ratio").max_f64(),
                            ":1", 1, state.selected_param, state.is_editing, Some('r'), SLIDER_HEIGHT, theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Knee", state.knee_db,
                            pk(CP, "knee").min_f64(), pk(CP, "knee").max_f64(),
                            "dB", 4, state.selected_param, state.is_editing, Some('k'), SLIDER_HEIGHT, theme,
                        )),
                ),
        )
        // Timing (Attack, Release)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title("TIMING", theme))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Attack", state.attack_ms,
                            pk(CP, "attack").min_f64(), pk(CP, "attack").max_f64(),
                            "ms", 2, state.selected_param, state.is_editing, Some('a'), SLIDER_HEIGHT, theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Release", state.release_ms,
                            pk(CP, "release").min_f64(), pk(CP, "release").max_f64(),
                            "ms", 3, state.selected_param, state.is_editing, Some('e'), SLIDER_HEIGHT, theme,
                        )),
                ),
        )
        // Output (Makeup, Mix, Auto)
        .child(
            div()
                .flex()
                .flex_col()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("OUTPUT", theme))
                        .child(render_knob(
                            entity.clone(), plugin_idx, "Makeup", state.makeup_gain_db,
                            pk(CP, "makeup_gain").min_f64(), pk(CP, "makeup_gain").max_f64(),
                            "dB", 5, state.selected_param, state.is_editing, Some('m'), theme,
                        ))
                        .child(render_knob(
                            entity.clone(), plugin_idx, "Mix", state.mix * 100.0,
                            pk(CP, "mix").min_f64() * 100.0, pk(CP, "mix").max_f64() * 100.0,
                            "%", 6, state.selected_param, state.is_editing, Some('x'), theme,
                        )),
                )
                .child(render_toggle(
                    entity.clone(), plugin_idx, "Auto Makeup", state.auto_makeup,
                    7, state.selected_param, state.is_editing, theme,
                )),
        )
        // Setup (Sidechain HPF, Link)
        .child(
            div()
                .flex()
                .flex_col()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("SETUP", theme))
                        .child(render_knob(
                            entity.clone(), plugin_idx, "SC HPF", state.sidechain_hpf_hz,
                            SIDECHAIN_HPF_UI_MIN, SIDECHAIN_HPF_UI_MAX,
                            "Hz", 9, state.selected_param, state.is_editing, Some('s'), theme,
                        )),
                )
                .child(render_toggle(
                    entity.clone(), plugin_idx, "Link Ch", state.link_channels,
                    8, state.selected_param, state.is_editing, theme,
                )),
        );

    // Meter section: GR meter
    let meter_section = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(render_section_title("METER", theme))
        .child(render_gr_meter(meter_value, -30.0, theme));

    render_dynamics_layout(transfer_curve, controls, meter_section, METER_WIDTH)
}
