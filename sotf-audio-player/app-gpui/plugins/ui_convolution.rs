//! Convolution Plugin UI Component

use super::common::{render_edit_hints, render_knob, render_param_row, render_section_header};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::convolution::*;

/// State for rendering the Convolution plugin
pub struct ConvolutionRenderState<'a> {
    pub ir_file: &'a str,
    pub mix: f64,
    pub gain_db: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Convolution plugin
pub fn render_convolution_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: ConvolutionRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let has_ir = !state.ir_file.is_empty();
    let ir_file = state.ir_file;

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
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(state.mix as f32))
                                        .bg(theme.accent),
                                ),
                        )
                        .child(div().text_xs().text_color(theme.text_muted).child("WET")),
                )
                .child(
                    div()
                        .text_center()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_primary)
                        .child(format!("{:.0}%", state.mix * 100.0)),
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
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Mix",
                    state.mix,
                    MIX_MIN as f64,
                    MIX_MAX as f64,
                    "%",
                    1,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Gain",
                    state.gain_db,
                    GAIN_DB_MIN as f64,
                    GAIN_DB_MAX as f64,
                    "dB",
                    2,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                )),
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
