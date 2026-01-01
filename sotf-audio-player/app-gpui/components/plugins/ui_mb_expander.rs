//! Multiband Expander Plugin UI Component
//!
//! Dynamic range expansion with frequency band splitting:
//! - Configurable number of bands (2-5)
//! - Crossover frequency controls
//! - Global threshold, ratio, attack, release, range, knee
//! - Hysteresis for smooth transitions
//! - Hold time before closing
//! - Mix (dry/wet)
//! - Link channels option

use super::common::{
    render_edit_hints, render_knob, render_section_title, render_toggle_button,
    render_vertical_slider_sized,
};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::multiband_expander::*;

/// State for rendering the Multiband Expander plugin
pub struct MbExpanderRenderState {
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
    pub range_db: f64,
    pub knee_db: f64,
    pub hysteresis_db: f64,
    pub hold_ms: f64,
    pub mix: f64,
    pub link_channels: bool,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Layout constants
const SLIDER_HEIGHT: f32 = 200.0;

/// Render the Multiband Expander plugin
pub fn render_mb_expander_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MbExpanderRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - columns side by side
        .child(
            div()
                .flex()
                .gap_4()
                // Column 1: Band configuration and crossover
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(render_section_title("CROSSOVER", theme))
                                // Band count display
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
                                    2,
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
                                    3,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('2'),
                                    theme,
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .when(state.num_bands >= 4, |d| {
                                    d.child(render_knob(
                                        entity.clone(),
                                        plugin_idx,
                                        "XOver 3",
                                        state.crossover_freq_3,
                                        CROSSOVER_FREQ_3_MIN as f64,
                                        CROSSOVER_FREQ_3_MAX as f64,
                                        "Hz",
                                        4,
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
                                        5,
                                        state.selected_param,
                                        state.is_editing,
                                        Some('4'),
                                        theme,
                                    ))
                                }),
                        ),
                )
                // Column 2: Dynamics (Threshold, Ratio, Knee)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("DYNAMICS", theme))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Threshold",
                                    state.threshold_db,
                                    THRESHOLD_MIN as f64,
                                    THRESHOLD_MAX as f64,
                                    "dB",
                                    6,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('t'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Ratio",
                                    state.ratio,
                                    RATIO_MIN as f64,
                                    RATIO_MAX as f64,
                                    ":1",
                                    7,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('r'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Knee",
                                    state.knee_db,
                                    KNEE_MIN as f64,
                                    KNEE_MAX as f64,
                                    "dB",
                                    11,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('k'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                )),
                        ),
                )
                // Column 3: Timing (Attack, Release, Hold)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("TIMING", theme))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Attack",
                                    state.attack_ms,
                                    ATTACK_MIN as f64,
                                    ATTACK_MAX as f64,
                                    "ms",
                                    8,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('a'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Hold",
                                    state.hold_ms,
                                    HOLD_MIN as f64,
                                    HOLD_MAX as f64,
                                    "ms",
                                    13,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('h'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Release",
                                    state.release_ms,
                                    RELEASE_MIN as f64,
                                    RELEASE_MAX as f64,
                                    "ms",
                                    9,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('e'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                )),
                        ),
                )
                // Column 4: Shape (Range, Hysteresis)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("SHAPE", theme))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Range",
                                    state.range_db,
                                    RANGE_MIN as f64,
                                    RANGE_MAX as f64,
                                    "dB",
                                    10,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('g'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Hysteresis",
                                    state.hysteresis_db,
                                    HYSTERESIS_MIN as f64,
                                    HYSTERESIS_MAX as f64,
                                    "dB",
                                    12,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('y'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                )),
                        ),
                )
                // Column 5: Output controls
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                // Header row with OUTPUT and Link Channels
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .w_full()
                                        .child(render_section_title("OUTPUT", theme))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("Link Ch."),
                                        ),
                                )
                                // Toggle button
                                .child(div().flex().justify_end().child(render_toggle_button(
                                    entity.clone(),
                                    plugin_idx,
                                    state.link_channels,
                                    15,
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                ))),
                        )
                        // Mix knob at bottom
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix * 100.0,
                            MIX_MIN as f64 * 100.0,
                            MIX_MAX as f64 * 100.0,
                            "%",
                            14,
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        )),
                ),
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
