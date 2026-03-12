//! Binaural Decoder Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | SETUP            | CONTROLS                                   | (no meter/AG)    |
//! |                  |                                            |                  |
//! | [SOFA File] path | [Externalization] knob                     |                  |
//! | [Input Ch]  int  | [Near-field]      knob                     |                  |
//! | [Optim]  toggle  |                                            |                  |
//! +------------------+--------------------------------------------+------------------+

use super::actions::OpenSofaFile;
use super::common::{render_knob, render_param_row, render_section_title, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{binaural::PARAMS as BN, find_by_key as pk};

/// State for rendering the Binaural Decoder plugin
pub struct BinauralRenderState<'a> {
    pub sofa_file: &'a str,
    pub input_channels: usize,
    pub enable_optimization: bool,
    pub externalization: f64,
    pub near_field_strength: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Layout constants
const SETUP_WIDTH: f32 = 180.0;

/// Render the Binaural Decoder plugin
pub fn render_binaural_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: BinauralRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let has_sofa = !state.sofa_file.is_empty();

    // === LEFT COLUMN: Setup ===
    let setup_col = div()
        .flex()
        .flex_col()
        .w(px(SETUP_WIDTH))
        .gap_3()
        .child(render_section_title("SETUP", theme))
        // SOFA File with Load Button
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(theme.background_secondary)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("SOFA File"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if has_sofa {
                                    theme.text_primary
                                } else {
                                    theme.text_muted
                                })
                                .overflow_hidden()
                                .text_ellipsis()
                                .max_w(px(120.0))
                                .child(if state.sofa_file.is_empty() {
                                    "None".to_string()
                                } else {
                                    state
                                        .sofa_file
                                        .rsplit('/')
                                        .next()
                                        .unwrap_or(state.sofa_file)
                                        .to_string()
                                }),
                        ),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.border)
                        .text_xs()
                        .id("load-sofa-btn")
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.surface_hover))
                        .on_click(move |_, _, cx| {
                            cx.dispatch_action(&OpenSofaFile { plugin_idx });
                        })
                        .child("Load"),
                ),
        )
        .child(render_param_row(
            "Input Channels",
            &format!("{}", state.input_channels),
            1,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "Optimization",
            state.enable_optimization,
            2,
            state.selected_param,
            state.is_editing,
            theme,
        ));

    // === CENTER COLUMN: Controls ===
    let center_col = div()
        .flex()
        .flex_col()
        .flex_1()
        .gap_3()
        .child(render_section_title("CONTROLS", theme))
        .child(
            div()
                .flex()
                .gap_4()
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Externalization",
                    state.externalization,
                    pk(BN, "externalization").min_f64(),
                    pk(BN, "externalization").max_f64(),
                    "%",
                    3,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Near Field",
                    state.near_field_strength,
                    pk(BN, "near_field_strength").min_f64(),
                    pk(BN, "near_field_strength").max_f64(),
                    "%",
                    4,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                )),
        );

    // === RIGHT COLUMN: empty ===
    let right_col = div().flex().flex_col().w(px(120.0));

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
