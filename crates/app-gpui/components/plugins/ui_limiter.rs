//! Limiter Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | SETUP            | DYNAMICS              TIMING                | OUTPUT           |
//! |                  |                                            |                  |
//! | [Soft Knee] tog  | [Threshold] slider    [Release] slider     | [GR Meter]       |
//! |                  |                       [Lookahead] slider   | [Mix]      knob  |
//! |                  | ┌─ Limiter Curve ──────────────────┐       |                  |
//! |                  | │                                  │       |                  |
//! |                  | └──────────────────────────────────┘       |                  |
//! +------------------+--------------------------------------------+------------------+

use super::common::{
    render_interactive_transfer_curve, render_knob, render_section_title, render_toggle,
    render_vertical_slider_with_ticks,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::LimiterData;
use sotf_plugins::param_specs::{find_by_key as pk, limiter::PARAMS as LM};

/// State for rendering the Limiter plugin
pub struct LimiterRenderState<'a> {
    pub threshold_db: f64,
    pub release_ms: f64,
    pub lookahead_ms: f64,
    pub soft: bool,
    pub mix: f64,
    pub is_editing: bool,
    pub selected_param: usize,
    pub data: Option<&'a LimiterData>,
}

// Layout constants
const TRANSFER_CURVE_SIZE: f32 = 200.0;
const SLIDER_HEIGHT: f32 = 180.0;

/// Render the Limiter plugin
pub fn render_limiter_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: LimiterRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Get real metering data
    let (gr_db, peak_db, is_limiting) = if let Some(data) = state.data {
        (
            data.gain_reduction_db as f64,
            data.peak_db as f64,
            data.is_limiting,
        )
    } else {
        (0.0, -100.0, false)
    };

    let meter_value = -gr_db;
    let input_level = if is_limiting { Some(peak_db) } else { None };

    // === LEFT COLUMN: Setup ===
    let setup_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap_3()
        .child(render_section_title("SETUP", theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "Soft Knee",
            state.soft,
            3,
            state.selected_param,
            state.is_editing,
            theme,
        ));

    // === CENTER COLUMN: Sliders (top) + Interactive transfer curve (bottom) ===
    let transfer_curve = render_interactive_transfer_curve(
        entity.clone(),
        plugin_idx,
        state.threshold_db,
        f64::INFINITY,
        0.0,
        true, // is_limiter
        TRANSFER_CURVE_SIZE,
        input_level,
        0,    // threshold param idx
        0,    // ratio param idx (unused for limiter)
        pk(LM, "threshold").min_f64(),
        pk(LM, "threshold").max_f64(),
        1.0,  // ratio min (unused)
        20.0, // ratio max (unused)
        theme,
    );

    // Dynamics column with transfer curve aligned below
    let dynamics_col = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(render_section_title("DYNAMICS", theme))
        .child(
            div()
                .flex()
                .gap_2()
                .child(render_vertical_slider_with_ticks(
                    entity.clone(),
                    plugin_idx,
                    "Ceiling",
                    state.threshold_db,
                    pk(LM, "threshold").min_f64(),
                    pk(LM, "threshold").max_f64(),
                    "dB",
                    0,
                    state.selected_param,
                    state.is_editing,
                    Some('c'),
                    SLIDER_HEIGHT,
                    theme,
                )),
        )
        .child(transfer_curve);

    let timing_col = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(render_section_title("TIMING", theme))
        .child(
            div()
                .flex()
                .gap_2()
                .child(render_vertical_slider_with_ticks(
                    entity.clone(),
                    plugin_idx,
                    "Release",
                    state.release_ms,
                    pk(LM, "release").min_f64(),
                    pk(LM, "release").max_f64(),
                    "ms",
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
                    "Lookahead",
                    state.lookahead_ms,
                    pk(LM, "lookahead").min_f64(),
                    pk(LM, "lookahead").max_f64(),
                    "ms",
                    2,
                    state.selected_param,
                    state.is_editing,
                    Some('l'),
                    SLIDER_HEIGHT,
                    theme,
                )),
        );

    let center_col = div()
        .flex()
        .flex_shrink_0()
        .gap_4()
        .child(dynamics_col)
        .child(timing_col);

    // === RIGHT COLUMN: Output (GR Meter + Mix) ===
    let right_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap_3()
        .child(render_section_title("OUTPUT", theme))
        .child(render_gr_meter(meter_value, -20.0, theme))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Mix",
            state.mix * 100.0,
            pk(LM, "mix").min_f64() * 100.0,
            pk(LM, "mix").max_f64() * 100.0,
            "%",
            4,
            state.selected_param,
            state.is_editing,
            Some('m'),
            theme,
        ));

    // === Main layout: 3 columns, centered ===
    div().w_full().flex().justify_center().p_3().child(
        div()
            .flex()
            .gap_4()
            .child(setup_col)
            .child(center_col)
            .child(right_col),
    )
}
