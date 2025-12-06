//! Convolution Plugin UI Component

use super::common::{render_edit_hints, render_knob, render_param_row, render_section_header};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Render the Convolution plugin
pub fn render_convolution_plugin(
    plugin_idx: usize,
    ir_file: &str,
    mix: f64,
    gain_db: f64,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    let has_ir = !ir_file.is_empty();

    div()
        .flex()
        .flex_col()
        .gap_4()
        // IR status section
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
                .child(render_section_header("IMPULSE RESPONSE", theme))
                // IR file status
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .py_3()
                        .child(
                            div()
                                .w(px(48.0))
                                .h(px(48.0))
                                .rounded_lg()
                                .bg(if has_ir { theme.accent } else { theme.surface })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xl()
                                .text_color(theme.text_on_accent)
                                .child("∿"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_primary)
                                        .child(if has_ir { "IR Loaded" } else { "No IR File" }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .max_w(px(200.0))
                                        .child(if has_ir {
                                            ir_file
                                                .rsplit('/')
                                                .next()
                                                .unwrap_or(ir_file)
                                                .to_string()
                                        } else {
                                            "Load an impulse response file".to_string()
                                        }),
                                ),
                        ),
                )
                // Simulated IR waveform
                .child(
                    div()
                        .h(px(60.0))
                        .w_full()
                        .bg(theme.surface)
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child(if has_ir {
                            "IR Waveform"
                        } else {
                            "No IR loaded"
                        }),
                ),
        )
        // Mix section with visual
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
                .child(render_section_header("MIX CONTROL", theme))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .py_2()
                        .child(div().text_xs().text_color(theme.text_muted).child("DRY"))
                        .child(
                            div()
                                .flex_1()
                                .h(px(12.0))
                                .bg(theme.surface)
                                .rounded_full()
                                .overflow_hidden()
                                .child(div().h_full().w(relative(mix as f32)).bg(theme.accent)),
                        )
                        .child(div().text_xs().text_color(theme.text_muted).child("WET")),
                )
                .child(
                    div()
                        .text_center()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_primary)
                        .child(format!("{:.0}%", mix * 100.0)),
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
                    "IR File",
                    if ir_file.is_empty() {
                        "None"
                    } else {
                        ir_file.rsplit('/').next().unwrap_or(ir_file)
                    },
                    0,
                    selected_param,
                    is_editing,
                    theme,
                ))
                .child(render_knob(
                    plugin_idx,
                    "Mix",
                    mix,
                    0.0,
                    1.0,
                    "%",
                    1,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    plugin_idx,
                    "Gain",
                    gain_db,
                    -20.0,
                    20.0,
                    "dB",
                    2,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                )),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
