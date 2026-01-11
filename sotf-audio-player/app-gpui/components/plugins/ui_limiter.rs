//! Limiter Plugin UI Component
//!
//! Brick-wall limiter with:
//! - Transfer curve display
//! - Gain reduction meter
//! - Peak meter with ceiling indicator
//! - Vertical sliders and rotary knob controls

use super::common::{
    render_edit_hints, render_knob, render_section_title, render_toggle_button,
    render_transfer_curve_sized, render_vertical_slider_sized,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::limiter::*;

/// State for rendering the Limiter plugin
pub struct LimiterRenderState {
    pub threshold_db: f64,
    pub release_ms: f64,
    pub lookahead_ms: f64,
    pub soft: bool,
    pub mix: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Layout constants
const METER_WIDTH: f32 = 180.0;
const SLIDER_HEIGHT: f32 = 200.0;

/// Render the Limiter plugin
pub fn render_limiter_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: LimiterRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Simulated values (in real implementation, these would come from the audio engine)
    let simulated_gr = if state.threshold_db < -1.0 {
        (state.threshold_db + 1.0) * 2.0
    } else {
        0.0
    };
    let simulated_peak = state.threshold_db - 3.0; // Simulated peak level below ceiling

    // Cache theme colors
    let peak_color = if simulated_peak > state.threshold_db {
        theme.error
    } else {
        theme.success
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - columns side by side
        .child(
            div()
                .flex()
                .gap_6()
                // Column 1: Vertical sliders for main dynamics controls
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
                                    entity.clone(),
                                    plugin_idx,
                                    "Ceiling",
                                    state.threshold_db,
                                    THRESHOLD_MIN as f64,
                                    THRESHOLD_MAX as f64,
                                    "dB",
                                    0,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('c'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Release",
                                    state.release_ms,
                                    RELEASE_MIN as f64,
                                    RELEASE_MAX as f64,
                                    "ms",
                                    1,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('r'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Lookahead",
                                    state.lookahead_ms,
                                    LOOKAHEAD_MIN as f64,
                                    LOOKAHEAD_MAX as f64,
                                    "ms",
                                    2,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('l'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                )),
                        ),
                )
                // Column 2: Mix knob and large ceiling display
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
                                // Soft Toggle
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("Soft Knee"),
                                        )
                                        .child(render_toggle_button(
                                            entity.clone(),
                                            plugin_idx,
                                            state.soft,
                                            3,
                                            state.selected_param,
                                            state.is_editing,
                                            theme,
                                        )),
                                )
                                // Mix knob
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Mix",
                                    state.mix * 100.0,
                                    MIX_MIN as f64 * 100.0,
                                    MIX_MAX as f64 * 100.0,
                                    "%",
                                    4,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('m'),
                                    theme,
                                )),
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
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("CEILING"),
                                )
                                .child(
                                    div()
                                        .text_xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.warning)
                                        .child(format!("{:.2} dB", state.threshold_db)),
                                ),
                        ),
                )
                // Column 3: Transfer curve, Peak meter, GR meter
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(METER_WIDTH))
                        .gap_2()
                        .child(render_section_title("METER", theme))
                        // Transfer curve - grows to fill space
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .child(render_transfer_curve_sized(
                                    state.threshold_db,
                                    f64::INFINITY,
                                    0.0,
                                    true,
                                    METER_WIDTH,
                                    theme,
                                )),
                        )
                        // Peak and GR info row
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
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("PEAK"),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(peak_color)
                                                .child(format!("{:.1}", simulated_peak)),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("GR"),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme.error)
                                                .child(format!("{:.1}", simulated_gr)),
                                        ),
                                ),
                        )
                        // Gain reduction meter
                        .child(render_gr_meter(simulated_gr, -20.0, theme)),
                ),
        )
        // .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
