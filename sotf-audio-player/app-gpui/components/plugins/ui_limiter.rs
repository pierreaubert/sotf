//! Limiter Plugin UI Component
//!
//! Brick-wall limiter with:
//! - Transfer curve display
//! - Gain reduction meter
//! - Peak meter with ceiling indicator
//! - Vertical sliders and rotary knob controls

use super::common::{
    ParamSectionStyle, render_knob, render_section_header, render_transfer_curve,
    render_vertical_slider,
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
    pub mix: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Fixed height for all columns to ensure consistent layout
// Height sized to fit columns with stacked knobs
const COLUMN_HEIGHT: f32 = 380.0;

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
        // Main section - Three columns side by side, all same height
        .child(
            div()
                .flex()
                .gap_4()
                .items_start()
                // Column 1: Vertical sliders for main dynamics controls
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("DYNAMICS", theme))
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .gap_2()
                                .child(render_vertical_slider(
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
                                    theme,
                                ))
                                .child(render_vertical_slider(
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
                                    theme,
                                )),
                        ),
                )
                // Column 2: Mix knob and large ceiling display
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("OUTPUT", theme))
                        // Spacer
                        .child(div().flex_1())
                        // Mix knob (direct child, no wrapper)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix * 100.0,
                            MIX_MIN as f64 * 100.0,
                            MIX_MAX as f64 * 100.0,
                            "%",
                            2,
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        ))
                        // Large ceiling display
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
                // Column 3: Transfer curve (top), Peak meter, GR meter (bottom)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("METER", theme))
                        // Transfer curve
                        .child(div().flex().justify_center().child(render_transfer_curve(
                            state.threshold_db,
                            f64::INFINITY,
                            0.0,
                            true,
                            theme,
                        )))
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
                        .child(
                            div()
                                .flex_1()
                                .child(render_gr_meter(simulated_gr, -20.0, theme)),
                        ),
                ),
        )
        .when(state.is_editing, |d| {
            d.child(
                div()
                    .mt_4()
                    .p_3()
                    .rounded_lg()
                    .bg(theme.background_secondary)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .gap_4()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child("↑/↓: Select")
                    .child("←/→: Adjust")
                    .child("[/]: Large step")
                    .child("Enter: Done"),
            )
        })
}
