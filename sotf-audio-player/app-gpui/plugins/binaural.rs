//! Binaural Decoder Plugin UI Component

use super::actions::OpenSofaFile;
use super::common::{render_edit_hints, render_param_row, render_section_header, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Potentiometer, PotentiometerTheme};

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
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_3()
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
                        .justify_center() // Center the knobs
                        // Externalization Knob
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_secondary)
                                        .child("Externalization"),
                                )
                                .child({
                                    let entity = entity.clone();
                                    Potentiometer::new(3)
                                        .value(state.externalization)
                                        .min(0.0)
                                        .max(1.0)
                                        .unit("%".to_string())
                                        .label("Ext".to_string()) // compact label
                                        .selected(state.selected_param == 3 && state.is_editing)
                                        .on_change(move |v, _, cx| {
                                            entity.update(cx, |state, _| {
                                                state.app.set_plugin_param(plugin_idx, 3, v);
                                            });
                                        })
                                        .theme(PotentiometerTheme {
                                            surface: theme.surface,
                                            surface_hover: theme.surface_hover,
                                            accent: theme.accent,
                                            accent_muted: theme.accent_muted,
                                            border: theme.border,
                                            text_secondary: theme.text_secondary,
                                            text_primary: theme.text_primary,
                                            text_muted: theme.text_muted,
                                            text_on_accent: theme.text_on_accent,
                                            background_secondary: theme.background_secondary,
                                            knob_bg: theme.surface,
                                        })
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_primary)
                                        .child(format!("{:.0}%", state.externalization * 100.0)),
                                ),
                        )
                        // Near Field Knob
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_secondary)
                                        .child("Near Field"),
                                )
                                .child({
                                    let entity = entity.clone();
                                    Potentiometer::new(4)
                                        .value(state.near_field_strength)
                                        .min(0.0)
                                        .max(1.0)
                                        .unit("%".to_string())
                                        .label("Near".to_string())
                                        .selected(state.selected_param == 4 && state.is_editing)
                                        .on_change(move |v, _, cx| {
                                            entity.update(cx, |state, _| {
                                                state.app.set_plugin_param(plugin_idx, 4, v);
                                            });
                                        })
                                        .theme(PotentiometerTheme {
                                            surface: theme.surface,
                                            surface_hover: theme.surface_hover,
                                            accent: theme.accent,
                                            accent_muted: theme.accent_muted,
                                            border: theme.border,
                                            text_secondary: theme.text_secondary,
                                            text_primary: theme.text_primary,
                                            text_muted: theme.text_muted,
                                            text_on_accent: theme.text_on_accent,
                                            background_secondary: theme.background_secondary,
                                            knob_bg: theme.surface,
                                        })
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_primary)
                                        .child(format!(
                                            "{:.0}%",
                                            state.near_field_strength * 100.0
                                        )),
                                ),
                        ),
                ),
        )
        // Hint for loading SOFA file
        .when(!has_sofa, |d| {
            d.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(Theme::opacity_20pct(theme.warning))
                    .border_1()
                    .border_color(theme.warning)
                    .text_sm()
                    .text_color(theme.warning)
                    .child("Press 'f' in edit mode to load a SOFA HRTF file"),
            )
        })
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
