//! Binaural Decoder Plugin UI Component

use super::actions::OpenSofaFile;
use super::common::{
    ParamSectionStyle, render_edit_hints, render_knob, render_param_row, render_section_header,
    render_toggle,
};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::binaural::*;

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

/// Render the Binaural Decoder plugin
pub fn render_binaural_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: BinauralRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let has_sofa = !state.sofa_file.is_empty();

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
                .param_section_style_lg(theme)
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
                                .bg(if has_sofa {
                                    theme.success
                                } else {
                                    theme.surface
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xl()
                                .text_color(theme.text_on_accent)
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
                                        .child(if has_sofa {
                                            "SOFA File Loaded"
                                        } else {
                                            "No SOFA File"
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .max_w(px(200.0))
                                        .child(if has_sofa {
                                            state
                                                .sofa_file
                                                .rsplit('/')
                                                .next()
                                                .unwrap_or(state.sofa_file)
                                                .to_string()
                                        } else {
                                            "Load a SOFA file to enable binaural decoding"
                                                .to_string()
                                        }),
                                ),
                        ),
                )
                // Spatial visualization (dynamic head/ears)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .py_6() // Increased padding
                        .relative() // For absolute positioning if needed, but flex gap is easier
                        .h(px(100.0))
                        .child({
                            // Calculate dynamic offset based on externalization (0.0 to 1.0)
                            // Base gap 8 (32px), max gap ?? Let's use margins or a container width.
                            // Better: Use a fixed width container and position ears relative to center.
                            let base_offset = 40.0;
                            let ext_offset = state.externalization * 80.0; // Moves up to 80px further out
                            let total_offset = (base_offset + ext_offset) as f32;

                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(20.0)) // Gap between ears and head? No, let's use absolute for precise control or explicit margins.
                                // simpler: Left Ear --(gap)-- Head --(gap)-- Right Ear
                                // We can just vary the gap!
                                .gap(px(total_offset))
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
                                                .bg(theme.accent)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_sm()
                                                .text_color(theme.text_on_accent)
                                                .font_weight(FontWeight::BOLD)
                                                .child("L"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .mt_1()
                                                .child("Left"),
                                        ),
                                )
                                // Head icon (Absolute center? No, flex item)
                                .child(
                                    div()
                                        .w(px(60.0))
                                        .h(px(70.0))
                                        .rounded_2xl()
                                        .bg(theme.surface)
                                        .border_2()
                                        .border_color(theme.border)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_3xl()
                                        .pb_1()
                                        .text_color(theme.text_primary)
                                        .child("☺"), // Smiley face for head
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
                                                .bg(theme.accent)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_sm()
                                                .text_color(theme.text_on_accent)
                                                .font_weight(FontWeight::BOLD)
                                                .child("R"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .mt_1()
                                                .child("Right"),
                                        ),
                                )
                        }),
                ),
        )
        // Parameters section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .param_section_style(theme)
                .child(render_section_header("PARAMETERS", theme))
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
                                .text_sm()
                                .text_color(theme.text_secondary)
                                .child("SOFA File"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.text_primary)
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
                ))
                // Knobs row for continuous parameters
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .mt_2()
                        .justify_center()
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Externalization",
                            state.externalization,
                            EXTERNALIZATION_MIN as f64,
                            EXTERNALIZATION_MAX as f64,
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
                            NEAR_FIELD_STRENGTH_MIN as f64,
                            NEAR_FIELD_STRENGTH_MAX as f64,
                            "%",
                            4,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
