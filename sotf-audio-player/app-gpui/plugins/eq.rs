//! EQ Plugin UI Component
//!
//! Provides a professional parametric EQ visualization with:
//! - Frequency response graph
//! - Band controls with color coding
//! - Interactive editing

use super::common::render_knob;
use crate::theme::Theme;
use crate::ui::components::graphs::{
    render_db_labels, render_eq_visualization, render_freq_labels,
};
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::EQFilter;

/// Render the EQ plugin with graphical visualization
pub fn render_eq_plugin(
    plugin_idx: usize,
    filters: &[EQFilter],
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    let selected_band_idx = if is_editing {
        // Determine which band is selected based on the selected_param
        // Each band has 4 parameters (Freq, Q, Gain, Type - though Type isn't a knob)
        // So, param_idx / 4 gives the band index.
        Some(selected_param / 4)
    } else {
        None
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        // EQ Graph section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_3()
                // Title
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .mb_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_primary)
                                .child("FREQUENCY RESPONSE"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(format!("{} bands", filters.len())),
                        ),
                )
                // Graph with axis labels
                .child(
                    div()
                        .flex()
                        .gap_2()
                        // dB axis
                        .child(render_db_labels(theme))
                        // Graph
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .flex_1()
                                        .bg(theme.surface)
                                        .rounded_lg()
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(render_eq_visualization(
                                            filters,
                                            selected_band_idx,
                                            theme,
                                        )),
                                )
                                // Frequency axis
                                .child(render_freq_labels(theme)),
                        ),
                ),
        )
        // Band controls section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_3()
                // Title
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_primary)
                        .mb_2()
                        .child("FILTER BANDS"),
                )
                // Band controls
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_4()
                        .children(filters.iter().enumerate().map(|(i, filter)| {
                            // Each filter has 4 params: Freq, Q, Gain, Type
                            let base_param_idx = i * 4;

                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .p_2()
                                .rounded_lg()
                                .bg(theme.background)
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_secondary)
                                        .child(format!("Band {}", i + 1)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .child(render_knob(
                                            plugin_idx,
                                            "Freq",
                                            filter.frequency,
                                            20.0,
                                            20000.0,
                                            "Hz",
                                            base_param_idx,
                                            selected_param,
                                            is_editing,
                                            None,
                                            theme,
                                        ))
                                        .child(render_knob(
                                            plugin_idx,
                                            "Gain",
                                            filter.gain_db,
                                            -24.0,
                                            24.0,
                                            "dB",
                                            base_param_idx + 2,
                                            selected_param,
                                            is_editing,
                                            None,
                                            theme,
                                        ))
                                        .child(render_knob(
                                            plugin_idx,
                                            "Q",
                                            filter.q,
                                            0.1,
                                            10.0,
                                            "",
                                            base_param_idx + 1,
                                            selected_param,
                                            is_editing,
                                            None,
                                            theme,
                                        )),
                                )
                        })),
                ),
        )
        // Edit mode hint
        .when(is_editing, |d| {
            d.child(
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
                    .child("↑/↓: Select band")
                    .child("←/→: Adjust value")
                    .child("[/]: Large step")
                    .child("Enter: Done"),
            )
        })
}
