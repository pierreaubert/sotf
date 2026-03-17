//! Gate Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | SETUP            | DYNAMICS              TIMING                | OUTPUT           |
//! |                  |                                            |                  |
//! | [Link Ch] toggle | [Threshold] slider    [Attack] slider      | [GR Meter]       |
//! | [SC HPF]  knob   | [Ratio]     slider    [Hold]   slider      | [Mix]      knob  |
//! |                  |                       [Release] slider     |                  |
//! |                  | ┌─ Gate Status ────────────────────┐       |                  |
//! |                  | │                                  │       |                  |
//! |                  | └──────────────────────────────────┘       |                  |
//! +------------------+--------------------------------------------+------------------+

use super::common::{
    render_knob, render_section_title, render_toggle, render_vertical_slider_with_ticks,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::GateData;
use sotf_plugins::param_specs::{find_by_key as pk, gate::PARAMS as GT};

/// State for rendering the Gate plugin
pub struct GateRenderState<'a> {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub hold_ms: f64,
    pub release_ms: f64,
    pub mix: f64,
    pub link_channels: bool,
    pub sidechain_hpf_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
    pub data: Option<&'a GateData>,
}

// Layout constants
const SLIDER_HEIGHT: f32 = 180.0;

// Sidechain HPF UI range (40-160Hz)
const SIDECHAIN_HPF_UI_MIN: f64 = 40.0;
const SIDECHAIN_HPF_UI_MAX: f64 = 160.0;

/// Render the Gate plugin
pub fn render_gate_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: GateRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Get metering data
    let (input_db, is_open, attenuation_db) = if let Some(data) = state.data {
        let max_input = data
            .input_levels_db
            .iter()
            .cloned()
            .fold(-100.0_f32, f32::max) as f64;
        let max_attenuation = data.attenuation_db.iter().cloned().fold(0.0_f32, f32::max) as f64;
        (max_input, data.is_open, max_attenuation)
    } else {
        (-100.0, false, 0.0)
    };

    // Normalize threshold for visual display (-80 to 0 dB range)
    let threshold_normalized = ((state.threshold_db + 80.0) / 80.0).clamp(0.0, 1.0) as f32;
    let input_normalized = ((input_db + 80.0) / 80.0).clamp(0.0, 1.0) as f32;

    // Cache theme colors
    let gate_color = if is_open { theme.success } else { theme.error };
    let gate_glow = if is_open {
        Theme::opacity_20pct(theme.success)
    } else {
        Theme::opacity_20pct(theme.error)
    };

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
            "Link Ch",
            state.link_channels,
            6,
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
            7,
            state.selected_param,
            state.is_editing,
            Some('s'),
            theme,
        ));

    // === CENTER COLUMN: Sliders (top) + Gate status (bottom) ===
    let sliders = div()
        .flex()
        .gap_4()
        // Dynamics: Threshold, Ratio
        .child(
            div()
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
                            "Threshold",
                            state.threshold_db,
                            pk(GT, "threshold").min_f64(),
                            pk(GT, "threshold").max_f64(),
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
                            pk(GT, "ratio").min_f64(),
                            pk(GT, "ratio").max_f64(),
                            ":1",
                            1,
                            state.selected_param,
                            state.is_editing,
                            Some('r'),
                            SLIDER_HEIGHT,
                            theme,
                        )),
                ),
        )
        // Timing: Attack, Hold, Release
        .child(
            div()
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
                            pk(GT, "attack").min_f64(),
                            pk(GT, "attack").max_f64(),
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
                            "Hold",
                            state.hold_ms,
                            pk(GT, "hold").min_f64(),
                            pk(GT, "hold").max_f64(),
                            "ms",
                            3,
                            state.selected_param,
                            state.is_editing,
                            Some('h'),
                            SLIDER_HEIGHT,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Release",
                            state.release_ms,
                            pk(GT, "release").min_f64(),
                            pk(GT, "release").max_f64(),
                            "ms",
                            4,
                            state.selected_param,
                            state.is_editing,
                            Some('e'),
                            SLIDER_HEIGHT,
                            theme,
                        )),
                ),
        );

    // Gate status indicator
    let gate_status = div().flex().flex_col().gap_2().items_center().child(
        div()
            .flex()
            .gap_4()
            .items_center()
            // Gate status circle
            .child(
                div()
                    .w(px(48.0))
                    .h(px(48.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gate_glow)
                    .border_3()
                    .border_color(gate_color)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(gate_color)
                            .child(if is_open { "OPEN" } else { "CLOSED" }),
                    ),
            )
            // Input level meter with threshold marker
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child("Input Level"),
                    )
                    .child(
                        div()
                            .h(px(12.0))
                            .w_full()
                            .bg(theme.background)
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .relative()
                            .overflow_hidden()
                            .child(div().h_full().w(relative(input_normalized)).bg(gate_color))
                            .child(
                                div()
                                    .absolute()
                                    .left(relative(threshold_normalized))
                                    .top_0()
                                    .bottom_0()
                                    .w(px(2.0))
                                    .bg(theme.warning),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child("-80")
                            .child("0 dB"),
                    ),
            ),
    );

    let center_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap_3()
        .child(sliders)
        .child(gate_status);

    // === RIGHT COLUMN: Output (GR Meter + Mix) ===
    let right_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap_3()
        .child(render_section_title("OUTPUT", theme))
        .child(render_gr_meter(-attenuation_db, -40.0, theme))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Mix",
            state.mix * 100.0,
            pk(GT, "mix").min_f64() * 100.0,
            pk(GT, "mix").max_f64() * 100.0,
            "%",
            5,
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
