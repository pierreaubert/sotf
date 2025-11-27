//! Binaural Decoder Plugin UI Component

use super::common::{render_edit_hints, render_param_row, render_section_header, render_toggle};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Render the Binaural Decoder plugin
#[allow(clippy::too_many_arguments)]
pub fn render_binaural_plugin(
    sofa_file: &str,
    input_channels: usize,
    enable_optimization: bool,
    externalization: f64,
    near_field_strength: f64,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    let has_sofa = !sofa_file.is_empty();

    div()
        .flex()
        .flex_col()
        .gap_4()
        // HRTF status section
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
                .child(render_section_header("HRTF STATUS", theme))
                // SOFA file status
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
                                .rounded_full()
                                .bg(if has_sofa { rgb(0x22c55e) } else { theme.surface })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xl()
                                .text_color(rgb(0xffffff))
                                .child(if has_sofa { "◎" } else { "○" }),
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
                                        .child(if has_sofa { "SOFA File Loaded" } else { "No SOFA File" }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .max_w(px(200.0))
                                        .child(if has_sofa {
                                            sofa_file.rsplit('/').next().unwrap_or(sofa_file).to_string()
                                        } else {
                                            "Load a SOFA file to enable binaural decoding".to_string()
                                        }),
                                ),
                        ),
                )
                // Spatial visualization (simplified head/ears diagram)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .py_4()
                        .gap_8()
                        // Left ear
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(
                                    div()
                                        .w(px(30.0))
                                        .h(px(30.0))
                                        .rounded_full()
                                        .bg(rgb(0xdb2777))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_sm()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .child("L"),
                                )
                                .child(div().text_xs().text_color(theme.text_muted).mt_1().child("Left")),
                        )
                        // Head icon
                        .child(
                            div()
                                .w(px(50.0))
                                .h(px(60.0))
                                .rounded_2xl()
                                .bg(theme.surface)
                                .border_2()
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_2xl()
                                .child("◎"),
                        )
                        // Right ear
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(
                                    div()
                                        .w(px(30.0))
                                        .h(px(30.0))
                                        .rounded_full()
                                        .bg(rgb(0xdb2777))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_sm()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .child("R"),
                                )
                                .child(div().text_xs().text_color(theme.text_muted).mt_1().child("Right")),
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
                    "SOFA File",
                    if sofa_file.is_empty() { "None" } else { sofa_file.rsplit('/').next().unwrap_or(sofa_file) },
                    0,
                    selected_param,
                    is_editing,
                    theme,
                ))
                .child(render_param_row("Input Channels", &format!("{}", input_channels), 1, selected_param, is_editing, theme))
                .child(render_toggle("Optimization", enable_optimization, 2, selected_param, is_editing, theme))
                .child(render_param_row("Externalization", &format!("{:.2}", externalization), 3, selected_param, is_editing, theme))
                .child(render_param_row("Near Field", &format!("{:.2}", near_field_strength), 4, selected_param, is_editing, theme)),
        )
        // Hint for loading SOFA file
        .when(!has_sofa, |d| {
            d.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(rgba(0xf59e0b33)) // Warning amber with 20% opacity
                    .border_1()
                    .border_color(theme.warning)
                    .text_sm()
                    .text_color(theme.warning)
                    .child("Press 'f' in edit mode to load a SOFA HRTF file"),
            )
        })
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
