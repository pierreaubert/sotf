//! Limiter Plugin UI Component
//!
//! Layout (3-column, same pattern as compressor):
//! +--------------------+------------------------------------------------+--------------------+
//! | Setup              | Transfer Curve  | Dynamic                      | Meter              |
//! |                    |                 | Ceiling  Release  Lookahead  | Peak / GR readouts |
//! | Soft Knee [on|off] |                 |                              | Gain Reduction bar |
//! |                    |                 |                              +--------------------+
//! |                    |                 |                              | Output             |
//! | CEILING display    |                 |                              | Mix                |
//! +--------------------+-----------------+------------------------------+--------------------+

use super::common::{
    render_knob, render_section_title, render_toggle_button,
    render_transfer_curve_with_level, render_vertical_slider_with_ticks,
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
const SETUP_WIDTH: f32 = 100.0;
const METER_WIDTH: f32 = 120.0;

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

    let peak_color = if is_limiting { theme.error } else { theme.success };

    // Transfer curve with input level
    let input_level = if is_limiting { Some(peak_db) } else { None };

    // === LEFT COLUMN: Setup ===
    let setup_col = div()
        .flex()
        .flex_col()
        .w(px(SETUP_WIDTH))
        .gap_3()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(render_section_title("SETUP", theme))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(div().text_xs().text_color(theme.text_muted).child("Soft Knee"))
                        .child(render_toggle_button(
                            entity.clone(), plugin_idx, state.soft,
                            3, state.selected_param, state.is_editing, theme,
                        )),
                ),
        )
        // Large ceiling display at bottom
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .p_2()
                .rounded_lg()
                .bg(theme.background)
                .child(div().text_xs().text_color(theme.text_muted).child("CEILING"))
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.warning)
                        .child(format!("{:.2} dB", state.threshold_db)),
                ),
        );

    // === CENTER COLUMN: Transfer curve + Sliders on same row ===
    let transfer_curve = render_transfer_curve_with_level(
        state.threshold_db, f64::INFINITY, 0.0, true, TRANSFER_CURVE_SIZE, input_level, theme,
    );

    let sliders = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(render_section_title("DYNAMIC", theme))
        .child(
            div()
                .flex()
                .gap_2()
                .child(render_vertical_slider_with_ticks(
                    entity.clone(), plugin_idx, "Ceiling", state.threshold_db,
                    pk(LM, "threshold").min_f64(), pk(LM, "threshold").max_f64(),
                    "dB", 0, state.selected_param, state.is_editing, Some('c'), SLIDER_HEIGHT, theme,
                ))
                .child(render_vertical_slider_with_ticks(
                    entity.clone(), plugin_idx, "Release", state.release_ms,
                    pk(LM, "release").min_f64(), pk(LM, "release").max_f64(),
                    "ms", 1, state.selected_param, state.is_editing, Some('r'), SLIDER_HEIGHT, theme,
                ))
                .child(render_vertical_slider_with_ticks(
                    entity.clone(), plugin_idx, "Lookahead", state.lookahead_ms,
                    pk(LM, "lookahead").min_f64(), pk(LM, "lookahead").max_f64(),
                    "ms", 2, state.selected_param, state.is_editing, Some('l'), SLIDER_HEIGHT, theme,
                )),
        );

    let center_col = div()
        .flex()
        .flex_row()
        .flex_1()
        .gap_4()
        .child(transfer_curve)
        .child(sliders);

    // === RIGHT COLUMN: Meter (top) + Output (bottom) ===
    let right_col = div()
        .flex()
        .flex_col()
        .w(px(METER_WIDTH))
        .gap_3()
        // Meter section with Peak/GR readouts
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(render_section_title("METER", theme))
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .justify_center()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(div().text_xs().text_color(theme.text_muted).child("PEAK"))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(peak_color)
                                        .child(format!("{:.1}", peak_db)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(div().text_xs().text_color(theme.text_muted).child("GR"))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.error)
                                        .child(format!("{:.1}", meter_value)),
                                ),
                        ),
                )
                .child(render_gr_meter(meter_value, -20.0, theme)),
        )
        // Output section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title("OUTPUT", theme))
                .child(render_knob(
                    entity.clone(), plugin_idx, "Mix", state.mix * 100.0,
                    pk(LM, "mix").min_f64() * 100.0, pk(LM, "mix").max_f64() * 100.0,
                    "%", 4, state.selected_param, state.is_editing, Some('m'), theme,
                )),
        );

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
