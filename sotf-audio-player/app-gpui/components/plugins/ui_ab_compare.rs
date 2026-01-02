//! A/B Compare Plugin UI Component
//!
//! Compare two audio processing paths with automatic loudness matching:
//! - Mix: A/B blend (-1.0 = A, +1.0 = B)
//! - Mix Mode: Potentiometer (continuous) or Binary (A or B)
//! - Auto Gain: Match loudness between paths
//! - Path configs: JSON configuration for each path

use super::common::{render_edit_hints, render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// State for rendering the A/B Compare plugin
pub struct ABCompareRenderState<'a> {
    pub mix: f64,
    pub mix_mode: i32,
    pub selected_path: i32,
    pub bypass: bool,
    pub auto_gain_enabled: bool,
    pub loudness_type: i32,
    pub max_auto_gain_db: f64,
    pub gain_smoothing_ms: f64,
    pub mix_transition_ms: f64,
    pub path_a_config: &'a str,
    pub path_b_config: &'a str,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the A/B Compare plugin
pub fn render_ab_compare_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: ABCompareRenderState<'_>,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // Row 1: Mix and Mode controls
        .child(
            div()
                .flex()
                .gap_6()
                // Column 1: Mix control
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("A/B MIX", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix * 100.0, // Display as percentage
                            -100.0,
                            100.0,
                            "%",
                            0,
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        )),
                )
                // Column 2: Mode controls
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("MODE", theme))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            if state.mix_mode == 0 { "Potentiometer" } else { "Binary" },
                            state.mix_mode != 0,
                            1,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            if state.selected_path == 0 { "Path: A" } else { "Path: B" },
                            state.selected_path != 0,
                            2,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        )),
                )
                // Column 3: Bypass
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("BYPASS", theme))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Bypass",
                            state.bypass,
                            3,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        )),
                ),
        )
        // Row 2: Auto-gain controls
        .child(
            div()
                .flex()
                .gap_6()
                // Column 1: Auto Gain toggle
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("AUTO GAIN", theme))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Enabled",
                            state.auto_gain_enabled,
                            4,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            if state.loudness_type == 0 { "Momentary" } else { "Short-term" },
                            state.loudness_type != 0,
                            5,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        )),
                )
                // Column 2: Gain parameters
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("PARAMETERS", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Max Gain",
                            state.max_auto_gain_db,
                            0.0,
                            24.0,
                            "dB",
                            6,
                            state.selected_param,
                            state.is_editing,
                            Some('g'),
                            theme,
                        )),
                )
                // Column 3: Smoothing
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("SMOOTHING", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Gain",
                            state.gain_smoothing_ms,
                            10.0,
                            500.0,
                            "ms",
                            7,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix Trans",
                            state.mix_transition_ms,
                            5.0,
                            500.0,
                            "ms",
                            8,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
        // Row 3: Path configurations (read-only display)
        .child(
            div()
                .flex()
                .gap_6()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(render_section_title("PATH A", theme))
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child(if state.path_a_config.is_empty() || state.path_a_config == r#"{"type":"None"}"# {
                                    "[passthrough]".to_string()
                                } else {
                                    state.path_a_config.chars().take(40).collect::<String>() + "..."
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(render_section_title("PATH B", theme))
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child(if state.path_b_config.is_empty() || state.path_b_config == r#"{"type":"None"}"# {
                                    "[passthrough]".to_string()
                                } else {
                                    state.path_b_config.chars().take(40).collect::<String>() + "..."
                                }),
                        ),
                ),
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
