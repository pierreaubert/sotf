//! Convolution Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | SETUP            | (center empty or waveform display)          | OUTPUT           |
//! |                  |                                            |                  |
//! | [IR File]  path  |                                            | [Mix]      knob  |
//! |                  |                                            | [Gain]     knob  |
//! +------------------+--------------------------------------------+------------------+

use super::common::{render_knob, render_section_title};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{convolution::PARAMS as CV, find_by_key as pk};

/// State for rendering the Convolution plugin
pub struct ConvolutionRenderState<'a> {
    pub ir_file: &'a str,
    pub mix: f64,
    pub gain_db: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Layout constants
const SETUP_WIDTH: f32 = 180.0;
const OUTPUT_WIDTH: f32 = 120.0;

/// Render the Convolution plugin
pub fn render_convolution_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: ConvolutionRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let has_ir = !state.ir_file.is_empty();
    let ir_file = state.ir_file;

    // === LEFT COLUMN: Setup ===
    let setup_col = div()
        .flex()
        .flex_col()
        .w(px(SETUP_WIDTH))
        .gap_3()
        .child(render_section_title("SETUP", theme))
        // IR file status
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(40.0))
                        .h(px(40.0))
                        .rounded_lg()
                        .bg(if has_ir { theme.accent } else { theme.surface })
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_lg()
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
                                .max_w(px(120.0))
                                .child(if has_ir {
                                    ir_file.rsplit('/').next().unwrap_or(ir_file).to_string()
                                } else {
                                    "Load an IR file".to_string()
                                }),
                        ),
                ),
        );

    // === CENTER COLUMN: IR waveform placeholder ===
    let center_col = div()
        .flex()
        .flex_col()
        .flex_1()
        .gap_3()
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
                .child(if has_ir { "IR Waveform" } else { "No IR loaded" }),
        );

    // === RIGHT COLUMN: Output ===
    let right_col = div()
        .flex()
        .flex_col()
        .w(px(OUTPUT_WIDTH))
        .gap_3()
        .child(render_section_title("OUTPUT", theme))
        .child(render_knob(
            entity.clone(), plugin_idx, "Mix", state.mix,
            pk(CV, "mix").min_f64(), pk(CV, "mix").max_f64(),
            "%", 1, state.selected_param, state.is_editing, None, theme,
        ))
        .child(render_knob(
            entity.clone(), plugin_idx, "Gain", state.gain_db,
            pk(CV, "gain_db").min_f64(), pk(CV, "gain_db").max_f64(),
            "dB", 2, state.selected_param, state.is_editing, None, theme,
        ));

    // === Main layout: 3 columns ===
    div()
        .flex()
        .gap_4()
        .p_3()
        .w_full()
        .child(setup_col)
        .child(center_col)
        .child(right_col)
}
