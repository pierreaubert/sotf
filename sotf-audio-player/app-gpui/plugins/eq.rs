//! EQ Plugin UI Component
//!
//! Provides a professional parametric EQ visualization with:
//! - Frequency response graph
//! - Band controls with color coding
//! - Interactive editing

use crate::theme::Theme;
use crate::ui::components::graphs::{
    render_db_labels, render_eq_band_controls, render_eq_visualization, render_freq_labels,
};
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::EQFilter;

/// Render the EQ plugin with graphical visualization
pub fn render_eq_plugin(
    filters: &[EQFilter],
    is_editing: bool,
    selected_band: usize,
    theme: &Theme,
) -> impl IntoElement {
    let selected = if is_editing {
        Some(selected_band)
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
                                        .child(render_eq_visualization(filters, selected, theme)),
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
                .child(render_eq_band_controls(filters, selected, theme)),
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
