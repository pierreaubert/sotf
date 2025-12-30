//! Multiband Compressor Plugin UI Component
//!
//! Dynamic range compression with frequency band splitting:
//! - Configurable number of bands (2-5)
//! - Crossover frequency controls
//! - Global threshold, ratio, attack, release, knee
//! - Mix (dry/wet)
//! - Link channels option

use super::common::{
    ParamSectionStyle, render_knob, render_section_header, render_toggle_button,
    render_vertical_slider,
};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::multiband_compressor::*;

/// State for rendering the Multiband Compressor plugin
pub struct MbCompressorRenderState {
    pub num_bands: usize,
    pub crossover_preset: i32,
    pub crossover_freq_1: f64,
    pub crossover_freq_2: f64,
    pub crossover_freq_3: f64,
    pub crossover_freq_4: f64,
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub knee_db: f64,
    pub mix: f64,
    pub link_channels: bool,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Fixed height for all columns to ensure consistent layout
const COLUMN_HEIGHT: f32 = 380.0;

/// Render the Multiband Compressor plugin
pub fn render_mb_compressor_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MbCompressorRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Three columns side by side
        .child(
            div()
                .flex()
                .gap_4()
                .items_start()
                // Column 1: Band configuration and crossover
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("CROSSOVER", theme))
                        // Band count selector
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("Bands:"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_primary)
                                        .child(format!("{}", state.num_bands)),
                                ),
                        )
                        // Crossover frequency knobs
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "XOver 1",
                            state.crossover_freq_1,
                            CROSSOVER_FREQ_1_MIN as f64,
                            CROSSOVER_FREQ_1_MAX as f64,
                            "Hz",
                            2, // crossover_freq_1 param index
                            state.selected_param,
                            state.is_editing,
                            Some('1'),
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "XOver 2",
                            state.crossover_freq_2,
                            CROSSOVER_FREQ_2_MIN as f64,
                            CROSSOVER_FREQ_2_MAX as f64,
                            "Hz",
                            3, // crossover_freq_2 param index
                            state.selected_param,
                            state.is_editing,
                            Some('2'),
                            theme,
                        ))
                        .when(state.num_bands >= 4, |d| {
                            d.child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "XOver 3",
                                state.crossover_freq_3,
                                CROSSOVER_FREQ_3_MIN as f64,
                                CROSSOVER_FREQ_3_MAX as f64,
                                "Hz",
                                4, // crossover_freq_3 param index
                                state.selected_param,
                                state.is_editing,
                                Some('3'),
                                theme,
                            ))
                        })
                        .when(state.num_bands >= 5, |d| {
                            d.child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "XOver 4",
                                state.crossover_freq_4,
                                CROSSOVER_FREQ_4_MIN as f64,
                                CROSSOVER_FREQ_4_MAX as f64,
                                "Hz",
                                5, // crossover_freq_4 param index
                                state.selected_param,
                                state.is_editing,
                                Some('4'),
                                theme,
                            ))
                        }),
                )
                // Column 2: Main dynamics controls (sliders)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("DYNAMICS", theme))
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .gap_2()
                                .child(render_vertical_slider(
                                    entity.clone(),
                                    plugin_idx,
                                    "Threshold",
                                    state.threshold_db,
                                    THRESHOLD_MIN as f64,
                                    THRESHOLD_MAX as f64,
                                    "dB",
                                    6, // threshold_db param index
                                    state.selected_param,
                                    state.is_editing,
                                    Some('t'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    entity.clone(),
                                    plugin_idx,
                                    "Ratio",
                                    state.ratio,
                                    RATIO_MIN as f64,
                                    RATIO_MAX as f64,
                                    ":1",
                                    7, // ratio param index
                                    state.selected_param,
                                    state.is_editing,
                                    Some('r'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    entity.clone(),
                                    plugin_idx,
                                    "Attack",
                                    state.attack_ms,
                                    ATTACK_MIN as f64,
                                    ATTACK_MAX as f64,
                                    "ms",
                                    8, // attack_ms param index
                                    state.selected_param,
                                    state.is_editing,
                                    Some('a'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    entity.clone(),
                                    plugin_idx,
                                    "Release",
                                    state.release_ms,
                                    RELEASE_MIN as f64,
                                    RELEASE_MAX as f64,
                                    "ms",
                                    9, // release_ms param index
                                    state.selected_param,
                                    state.is_editing,
                                    Some('e'),
                                    theme,
                                )),
                        ),
                )
                // Column 3: Output controls
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        // Header row with OUTPUT and Link Channels
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .w_full()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.text_secondary)
                                        .child("OUTPUT"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("Link Ch."),
                                ),
                        )
                        // Toggle button below header
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .mt_1()
                                .child(render_toggle_button(
                                    entity.clone(),
                                    plugin_idx,
                                    state.link_channels,
                                    12, // link_channels param index
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                )),
                        )
                        // Spacer to push knobs to bottom
                        .child(div().flex_1())
                        // Knee knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Knee",
                            state.knee_db,
                            KNEE_MIN as f64,
                            KNEE_MAX as f64,
                            "dB",
                            10, // knee_db param index
                            state.selected_param,
                            state.is_editing,
                            Some('k'),
                            theme,
                        ))
                        // Mix knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix * 100.0,
                            MIX_MIN as f64 * 100.0,
                            MIX_MAX as f64 * 100.0,
                            "%",
                            11, // mix param index
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        )),
                ),
        )
        .when(state.is_editing, |d| {
            d.child(
                div()
                    .mt_4()
                    .p_3()
                    .rounded_lg()
                    .bg(theme.background_secondary)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .gap_4()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child("↑/↓: Select")
                    .child("←/→: Adjust")
                    .child("[/]: Large step")
                    .child("Enter: Done"),
            )
        })
}
