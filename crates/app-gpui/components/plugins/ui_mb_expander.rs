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
    render_knob, render_section_title, render_toggle_button, render_vertical_slider_sized,
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
    pub selected_band_idx: usize,
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
    // Helper to get parameter index based on selected band
    // Global: 0-99, Band 1: 100-199, Band 2: 200-299, etc.
    let get_param_idx = |base_idx: usize| -> usize {
        if state.selected_band_idx > 0 {
            state.selected_band_idx * 100 + base_idx
        } else {
            base_idx
        }
    };

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
                        // Band Selector
                        .child(div().flex().gap_1().justify_center().children(
                            (0..=state.num_bands).map(|i| {
                                let is_selected = state.selected_band_idx == i;
                                let label = if i == 0 {
                                    "Global".to_string()
                                } else {
                                    format!("{}", i)
                                };
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .text_xs()
                                    .font_weight(if is_selected {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .bg(if is_selected {
                                        theme.accent
                                    } else {
                                        theme.background_secondary
                                    })
                                    .text_color(if is_selected {
                                        theme.text_on_accent
                                    } else {
                                        theme.text_secondary
                                    })
                                    .cursor_pointer()
                                    .id(("mb-ex-band", i))
                                    .on_mouse_down(MouseButton::Left, {
                                        let entity = entity.clone();
                                        move |_, _window, cx| {
                                            entity.update(cx, |state, _| {
                                                state.app.plugin_state.selected_eq_band = i;
                                            });
                                        }
                                    })
                                    .child(label)
                            }),
                        ))
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
                                    get_param_idx(6),
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
                                    get_param_idx(7),
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
                                    get_param_idx(11),
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
                                    get_param_idx(8),
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
                                    get_param_idx(13),
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
                                    get_param_idx(9),
                                    state.selected_param,
                                    state.is_editing,
                                    Some('e'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                )),
                        ),
                )
                // Column 4: Shape (Range, Hysteresis) + Solo/Bypass
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
                                    get_param_idx(10),
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
                                    get_param_idx(12),
                                    state.selected_param,
                                    state.is_editing,
                                    Some('y'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                )),
                        )
                        // Band controls (Solo/Bypass)
                        .when(state.selected_band_idx > 0, |d| {
                            d.child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .justify_center()
                                    .mt_2()
                                    .child(render_toggle_button(
                                        entity.clone(),
                                        plugin_idx,
                                        false, // TODO: Bind to Solo
                                        get_param_idx(15),
                                        state.selected_param,
                                        state.is_editing,
                                        theme,
                                    ))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_secondary)
                                            .child("Solo"),
                                    )
                                    .child(render_toggle_button(
                                        entity.clone(),
                                        plugin_idx,
                                        false, // TODO: Bind to Bypass
                                        get_param_idx(14),
                                        state.selected_param,
                                        state.is_editing,
                                        theme,
                                    ))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_secondary)
                                            .child("Bypass"),
                                    ),
                            )
                        }),
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
                                    15, // Output link channel index, careful with conflict
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
                            14, // Output mix index, conflict?
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        )),
                ),
        )
    // .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
