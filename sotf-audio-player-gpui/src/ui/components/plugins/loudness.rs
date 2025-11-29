//! Loudness Plugin UI Components

use super::common::{render_edit_hints, render_param_row, render_section_header};
use super::ticks::{ScaleType, TickConfig, render_tick_row};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Render the Loudness Compensation plugin
pub fn render_loudness_compensation_plugin(
    target_lufs: f64,
    min_gain_db: f64,
    max_gain_db: f64,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    // TickConfig for target LUFS display (-30 to -10 range)
    let lufs_tick_config = TickConfig::new(ScaleType::Linear, -30.0, -10.0)
        .with_major_values(vec![-30.0, -24.0, -18.0, -14.0, -10.0]);

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Visual section - Target LUFS display
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
                .child(render_section_header("TARGET LOUDNESS", theme))
                // Large LUFS display
                .child(
                    div().flex().items_center().justify_center().py_4().child(
                        div()
                            .text_size(px(36.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0x0891b2))
                            .child(format!("{:.1} LUFS", target_lufs)),
                    ),
                )
                // LUFS meter bar (uses same scale as ticks)
                .child(
                    div()
                        .h(px(12.0))
                        .w_full()
                        .bg(theme.surface)
                        .rounded_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(lufs_tick_config.value_to_position(target_lufs)))
                                .bg(rgb(0x0891b2)),
                        ),
                )
                // Tick marks
                .child(render_tick_row(&lufs_tick_config, 0.0, 0.0))
                // Legend (aligned with ticks)
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .children(
                            lufs_tick_config
                                .major_values
                                .iter()
                                .map(|v| div().child(format!("{:.0}", v))),
                        ),
                ),
        )
        // Gain range section
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
                .child(render_section_header("GAIN RANGE", theme))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .py_2()
                        // Min gain
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(div().text_xs().text_color(theme.text_muted).child("MIN"))
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xef4444))
                                        .child(format!("{:.1}", min_gain_db)),
                                )
                                .child(div().text_xs().text_color(theme.text_muted).child("dB")),
                        )
                        // Range bar
                        .child(
                            div()
                                .flex_1()
                                .h(px(8.0))
                                .bg(theme.surface)
                                .rounded_full()
                                .relative()
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .bottom_0()
                                        .left(relative(0.25))
                                        .right(relative(0.25))
                                        .bg(rgb(0x0891b2)),
                                ),
                        )
                        // Max gain
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(div().text_xs().text_color(theme.text_muted).child("MAX"))
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0x22c55e))
                                        .child(format!("{:+.1}", max_gain_db)),
                                )
                                .child(div().text_xs().text_color(theme.text_muted).child("dB")),
                        ),
                ),
        )
        // Parameters section
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
                .child(render_section_header("PARAMETERS", theme))
                .child(render_param_row(
                    "Target LUFS",
                    &format!("{:.1}", target_lufs),
                    0,
                    selected_param,
                    is_editing,
                    theme,
                ))
                .child(render_param_row(
                    "Min Gain",
                    &format!("{:.1} dB", min_gain_db),
                    1,
                    selected_param,
                    is_editing,
                    theme,
                ))
                .child(render_param_row(
                    "Max Gain",
                    &format!("{:.1} dB", max_gain_db),
                    2,
                    selected_param,
                    is_editing,
                    theme,
                )),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}

/// Render the Loudness Monitor plugin (analyzer)
pub fn render_loudness_monitor_plugin(_is_editing: bool, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // LUFS meters
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
                .child(render_section_header("EBU R128 LOUDNESS", theme))
                // Integrated LUFS
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .py_2()
                        .child(
                            div()
                                .w(px(80.0))
                                .text_sm()
                                .text_color(theme.text_secondary)
                                .child("Integrated"),
                        )
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0x14b8a6))
                                .child("--- LUFS"),
                        ),
                )
                // Momentary LUFS
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .py_2()
                        .child(
                            div()
                                .w(px(80.0))
                                .text_sm()
                                .text_color(theme.text_secondary)
                                .child("Momentary"),
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("--- LUFS"),
                        ),
                )
                // Short-term LUFS
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .py_2()
                        .child(
                            div()
                                .w(px(80.0))
                                .text_sm()
                                .text_color(theme.text_secondary)
                                .child("Short-term"),
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("--- LUFS"),
                        ),
                )
                // True Peak
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .py_2()
                        .child(
                            div()
                                .w(px(80.0))
                                .text_sm()
                                .text_color(theme.text_secondary)
                                .child("True Peak"),
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("--- dBTP"),
                        ),
                ),
        )
        // Info section
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
                .child(div().text_sm().text_color(theme.text_muted).child(
                    "Real-time loudness monitoring following EBU R128 / ITU-R BS.1770 standards.",
                ))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child("Values update during playback."),
                ),
        )
}
