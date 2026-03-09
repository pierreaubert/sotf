//! Expander Plugin UI Component
//!
//! Dynamic range expander with hysteresis:
//! - Threshold, ratio, attack, release
//! - Range (max gain reduction)
//! - Knee softness
//! - Hysteresis for smooth open/close transitions
//! - Hold time before closing
//! - Mix (dry/wet)

use super::common::{
    render_dynamics_layout, render_knob, render_section_title, render_toggle_button,
    render_transfer_curve_sized, render_vertical_slider_sized,
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
    pub link_channels: bool,
    pub sidechain_hpf_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Layout constants
const SLIDER_HEIGHT: f32 = 200.0;

/// Render the Expander plugin
pub fn render_expander_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: ExpanderRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Transfer curve (expander uses inverse curve)
    let transfer_curve = render_transfer_curve_sized(
        state.threshold_db, state.ratio, state.knee_db, false, 180.0, theme,
    );

    // Controls
    let controls = div()
        .flex()
        .gap_4()
        // Dynamics
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
                        .child(render_vertical_slider_sized(
                            entity.clone(), plugin_idx, "Threshold", state.threshold_db,
                            pk(EX, "threshold").min_f64(), pk(EX, "threshold").max_f64(),
                            "dB", 0, state.selected_param, state.is_editing, Some('t'), Some(SLIDER_HEIGHT), theme,
                        ))
                        .child(render_vertical_slider_sized(
                            entity.clone(), plugin_idx, "Ratio", state.ratio,
                            pk(EX, "ratio").min_f64(), pk(EX, "ratio").max_f64(),
                            ":1", 1, state.selected_param, state.is_editing, Some('r'), Some(SLIDER_HEIGHT), theme,
                        )),
                ),
        )
        // Timing
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
                        .child(render_vertical_slider_sized(
                            entity.clone(), plugin_idx, "Attack", state.attack_ms,
                            pk(EX, "attack").min_f64(), pk(EX, "attack").max_f64(),
                            "ms", 2, state.selected_param, state.is_editing, Some('a'), Some(SLIDER_HEIGHT), theme,
                        ))
                        .child(render_vertical_slider_sized(
                            entity.clone(), plugin_idx, "Release", state.release_ms,
                            pk(EX, "release").min_f64(), pk(EX, "release").max_f64(),
                            "ms", 3, state.selected_param, state.is_editing, Some('e'), Some(SLIDER_HEIGHT), theme,
                        )),
                ),
        )
        // Shape
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title("SHAPE", theme))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(render_vertical_slider_sized(
                            entity.clone(), plugin_idx, "Range", state.range_db,
                            pk(EX, "range").min_f64(), pk(EX, "range").max_f64(),
                            "dB", 4, state.selected_param, state.is_editing, Some('g'), Some(SLIDER_HEIGHT), theme,
                        ))
                        .child(render_vertical_slider_sized(
                            entity.clone(), plugin_idx, "Knee", state.knee_db,
                            pk(EX, "knee").min_f64(), pk(EX, "knee").max_f64(),
                            "dB", 5, state.selected_param, state.is_editing, Some('k'), Some(SLIDER_HEIGHT), theme,
                        ))
                        .child(render_vertical_slider_sized(
                            entity.clone(), plugin_idx, "Hyst.", state.hysteresis_db,
                            pk(EX, "hysteresis").min_f64(), pk(EX, "hysteresis").max_f64(),
                            "dB", 6, state.selected_param, state.is_editing, Some('y'), Some(SLIDER_HEIGHT), theme,
                        ))
                        .child(render_vertical_slider_sized(
                            entity.clone(), plugin_idx, "Hold", state.hold_ms,
                            pk(EX, "hold").min_f64(), pk(EX, "hold").max_f64(),
                            "ms", 7, state.selected_param, state.is_editing, Some('h'), Some(SLIDER_HEIGHT), theme,
                        )),
                ),
        )
        // Output
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
                        .child(
                            div().flex().justify_between().items_center().w_full()
                                .child(render_section_title("OUTPUT", theme))
                                .child(div().text_xs().text_color(theme.text_muted).child("Link Ch.")),
                        )
                        .child(div().flex().justify_end().child(render_toggle_button(
                            entity.clone(), plugin_idx, state.link_channels,
                            9, state.selected_param, state.is_editing, theme,
                        )))
                        .child(render_knob(
                            entity.clone(), plugin_idx, "Mix", state.mix * 100.0,
                            pk(EX, "mix").min_f64() * 100.0, pk(EX, "mix").max_f64() * 100.0,
                            "%", 8, state.selected_param, state.is_editing, Some('m'), theme,
                        )),
                )
                .child(render_knob(
                    entity.clone(), plugin_idx, "SC HPF", state.sidechain_hpf_hz,
                    pk(EX, "sidechain_hpf_hz").min_f64(), pk(EX, "sidechain_hpf_hz").max_f64(),
                    "Hz", 10, state.selected_param, state.is_editing, Some('s'), theme,
                )),
        );

    // Meter section (no real-time data for expander yet)
    let meter_section = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(render_section_title("METER", theme))
        .child(render_gr_meter(0.0, -40.0, theme));

    render_dynamics_layout(transfer_curve, controls, meter_section, 180.0)
}
