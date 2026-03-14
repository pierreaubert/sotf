//! Expander Plugin UI Component
//!
//! Layout (4-column):
//! +------------------+------------------------+------------------+------------------+
//! | SETUP            | DYNAMICS               | TIMING           | OUTPUT           |
//! |                  |                        |                  |                  |
//! | Link Ch          | [Threshold] slider     | [Attack] slider  | [GR Meter]       |
//! |        [on|off]  | [Ratio]     slider     | [Release] slider | [Mix]      knob  |
//! | [SC HPF]  knob   | [Range]     slider     | [Hold]   slider  |                  |
//! |                  | [Knee]      slider     |                  |                  |
//! |                  | [Hysteresis] slider    |                  |                  |
//! |                  | ┌─ Transfer Curve ──┐  |                  |                  |
//! |                  | │                   │  |                  |                  |
//! |                  | └───────────────────┘  |                  |                  |
//! +------------------+------------------------+------------------+------------------+

use super::common::{
    render_knob, render_section_title, render_toggle, render_transfer_curve_sized,
    render_vertical_slider_with_ticks,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{expander::PARAMS as EX, find_by_key as pk};

/// State for rendering the Expander plugin
pub struct ExpanderRenderState {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub range_db: f64,
    pub knee_db: f64,
    pub hysteresis_db: f64,
    pub hold_ms: f64,
    pub mix: f64,
    pub auto_makeup: bool,
    pub link_channels: bool,
    pub sidechain_hpf_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Layout constants
const SLIDER_HEIGHT: f32 = 180.0;
const TRANSFER_CURVE_SIZE: f32 = 200.0;
const SETUP_WIDTH: f32 = 100.0;
const OUTPUT_WIDTH: f32 = 120.0;

// Sidechain HPF UI range
const SIDECHAIN_HPF_UI_MIN: f64 = 40.0;
const SIDECHAIN_HPF_UI_MAX: f64 = 160.0;

/// Render the Expander plugin
pub fn render_expander_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: ExpanderRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // === LEFT COLUMN: Setup ===
    let setup_col = div()
        .flex()
        .flex_col()
        .w(px(SETUP_WIDTH))
        .gap_3()
        .child(render_section_title("SETUP", theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "Link Ch",
            state.link_channels,
            10,
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
            11,
            state.selected_param,
            state.is_editing,
            Some('s'),
            theme,
        ));

    // === CENTER COLUMN: Dynamics (sliders + transfer curve) + Timing ===
    let transfer_curve = render_transfer_curve_sized(
        state.threshold_db,
        state.ratio,
        state.knee_db,
        false,
        TRANSFER_CURVE_SIZE,
        theme,
    );

    let dynamics_sliders = div()
        .flex()
        .gap_2()
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Threshold",
            state.threshold_db,
            pk(EX, "threshold").min_f64(),
            pk(EX, "threshold").max_f64(),
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
            pk(EX, "ratio").min_f64(),
            pk(EX, "ratio").max_f64(),
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
            "Range",
            state.range_db,
            pk(EX, "range").min_f64(),
            pk(EX, "range").max_f64(),
            "dB",
            4,
            state.selected_param,
            state.is_editing,
            Some('g'),
            SLIDER_HEIGHT,
            theme,
        ))
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Knee",
            state.knee_db,
            pk(EX, "knee").min_f64(),
            pk(EX, "knee").max_f64(),
            "dB",
            5,
            state.selected_param,
            state.is_editing,
            Some('k'),
            SLIDER_HEIGHT,
            theme,
        ))
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Hyst.",
            state.hysteresis_db,
            pk(EX, "hysteresis").min_f64(),
            pk(EX, "hysteresis").max_f64(),
            "dB",
            6,
            state.selected_param,
            state.is_editing,
            Some('y'),
            SLIDER_HEIGHT,
            theme,
        ));

    // Dynamics column: sliders + transfer curve (same width)
    let dynamics_col = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(render_section_title("DYNAMICS", theme))
        .child(dynamics_sliders)
        .child(transfer_curve);

    // Timing column: Attack, Release, Hold
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
                    "Attack",
                    state.attack_ms,
                    pk(EX, "attack").min_f64(),
                    pk(EX, "attack").max_f64(),
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
                    pk(EX, "release").min_f64(),
                    pk(EX, "release").max_f64(),
                    "ms",
                    3,
                    state.selected_param,
                    state.is_editing,
                    Some('e'),
                    SLIDER_HEIGHT,
                    theme,
                ))
                .child(render_vertical_slider_with_ticks(
                    entity.clone(),
                    plugin_idx,
                    "Hold",
                    state.hold_ms,
                    pk(EX, "hold").min_f64(),
                    pk(EX, "hold").max_f64(),
                    "ms",
                    7,
                    state.selected_param,
                    state.is_editing,
                    Some('h'),
                    SLIDER_HEIGHT,
                    theme,
                )),
        );

    let center_col = div()
        .flex_1()
        .flex()
        .justify_center()
        .child(
            div()
                .flex()
                .gap_4()
                .child(dynamics_col)
                .child(timing_col),
        );

    // === RIGHT COLUMN: Output (GR Meter + AutoGain + Mix) ===
    let right_col = div()
        .flex()
        .flex_col()
        .w(px(OUTPUT_WIDTH))
        .gap_3()
        .child(render_section_title("OUTPUT", theme))
        .child(render_gr_meter(0.0, -40.0, theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "AutoGain",
            state.auto_makeup,
            9,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Mix",
            state.mix * 100.0,
            pk(EX, "mix").min_f64() * 100.0,
            pk(EX, "mix").max_f64() * 100.0,
            "%",
            8,
            state.selected_param,
            state.is_editing,
            Some('m'),
            theme,
        ));

    // === Main layout: 3 columns ===
    div()
        .flex()
        .gap_4()
        .p_3()
        .w_full()
        .child(setup_col)
        .child(center_col)
        .child(right_col)
}
