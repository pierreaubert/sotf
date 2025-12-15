use crate::components::graphs::common::{band_color, format_frequency};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::EQFilter;

/// Render EQ band control buttons
pub fn render_eq_band_controls(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .gap_2()
        .flex_wrap()
        .children(filters.iter().enumerate().map(|(i, f)| {
            let is_selected = selected_band == Some(i);
            let color = band_color(i, theme);
            let filter_type_name = f.filter_type.short_name();

            div()
                .id(SharedString::from(format!("band-{}", i)))
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(if is_selected {
                    theme.accent_muted
                } else {
                    theme.surface
                })
                .border_2()
                .border_color(if is_selected { color } else { theme.border })
                .min_w(px(75.0))
                .cursor_pointer()
                .hover(|s| s.border_color(color))
                // Band indicator
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(color))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_primary)
                                .child(format!("{} {}", i + 1, filter_type_name)),
                        ),
                )
                // Frequency
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(format_frequency(f.frequency)),
                )
                // Gain
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if f.gain_db > 0.5 {
                            theme.success
                        } else if f.gain_db < -0.5 {
                            theme.error
                        } else {
                            theme.text_muted
                        })
                        .child(format!("{:+.1}dB", f.gain_db)),
                )
                // Q
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(format!("Q:{:.1}", f.q)),
                )
        }))
}
