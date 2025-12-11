//! Gain Plugin UI Component
//!
//! Simple gain control with:
//! - Large visual gain display
//! - Rotary knob control
//! - Color-coded boost/cut indication

use super::common::{render_edit_hints, render_knob, render_section_header};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// State for rendering the Gain plugin
pub struct GainRenderState {
    pub gain_db: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Gain plugin
pub fn render_gain_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: GainRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let is_boost = state.gain_db > 0.5;
    let is_cut = state.gain_db < -0.5;

    // Color based on gain direction
    let gain_color = if is_boost {
        theme.success // Green for boost
    } else if is_cut {
        theme.error // Red for cut
    } else {
        theme.text_primary // Neutral
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Large gain display and knob
        .child(
            div()
                .flex()
                .gap_4()
                .items_center()
                .justify_center()
                // Knob section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4()
                        .items_center()
                        .child(render_section_header("GAIN CONTROL", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Gain",
                            state.gain_db,
                            -24.0,
                            24.0,
                            "dB",
                            0,
                            state.selected_param,
                            state.is_editing,
                            Some('g'),
                            theme,
                        )),
                )
                // Large gain display
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_4()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_6()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("OUTPUT GAIN"),
                        )
                        // Large circular gain display
                        .child(
                            div()
                                .w(px(120.0))
                                .h(px(120.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(theme.surface)
                                .border_4()
                                .border_color(gain_color)
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_3xl()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(gain_color)
                                                .child(format!("{:+.1}", state.gain_db)),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.text_muted)
                                                .child("dB"),
                                        ),
                                ),
                        )
                        // Status indicator
                        .child(
                            div()
                                .px_4()
                                .py_2()
                                .rounded_full()
                                .bg(if is_boost {
                                    rgba(0x22c55e33)
                                } else if is_cut {
                                    rgba(0xef444433)
                                } else {
                                    rgba(0x6366f133)
                                })
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(gain_color)
                                .child(if is_boost {
                                    "BOOST"
                                } else if is_cut {
                                    "CUT"
                                } else {
                                    "UNITY"
                                }),
                        ),
                ),
        )
        // Horizontal gain bar visualization
        .child(
            div()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child("Gain Range"),
                        )
                        .child(
                            div()
                                .h(px(24.0))
                                .w_full()
                                .bg(theme.background)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                // Center line (0 dB)
                                .child(
                                    div()
                                        .absolute()
                                        .left(relative(0.5))
                                        .top_0()
                                        .bottom_0()
                                        .w(px(2.0))
                                        .bg(theme.text_muted),
                                )
                                // Gain bar (from center)
                                .child(if state.gain_db >= 0.0 {
                                    // Boost - bar goes right from center
                                    let width = (state.gain_db / 24.0).clamp(0.0, 1.0) as f32 * 0.5;
                                    div()
                                        .absolute()
                                        .left(relative(0.5))
                                        .top_0()
                                        .bottom_0()
                                        .w(relative(width))
                                        .bg(theme.success)
                                } else {
                                    // Cut - bar goes left from center
                                    let width =
                                        (-state.gain_db / 24.0).clamp(0.0, 1.0) as f32 * 0.5;
                                    div()
                                        .absolute()
                                        .right(relative(0.5))
                                        .top_0()
                                        .bottom_0()
                                        .w(relative(width))
                                        .bg(theme.error)
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("-24 dB")
                                .child("0 dB")
                                .child("+24 dB"),
                        ),
                ),
        )
        // Keyboard hints
        .child(
            div()
                .p_3()
                .rounded_lg()
                .bg(theme.accent_muted)
                .border_1()
                .border_color(theme.accent)
                .flex()
                .gap_4()
                .text_xs()
                .text_color(theme.text_secondary)
                .child("[G]ain")
                .child("←/→: Adjust")
                .child("[/]: Large step"),
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
