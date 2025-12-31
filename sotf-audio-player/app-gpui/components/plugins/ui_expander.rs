//! Expander Plugin UI Component
//!
//! Dynamic range expander with hysteresis:
//! - Threshold, ratio, attack, release
//! - Range (max gain reduction)
//! - Knee softness
//! - Hysteresis for smooth open/close transitions
//! - Hold time before closing
//! - Mix (dry/wet)

use super::common::{
    render_edit_hints, render_knob, render_section_title, render_toggle_button,
    render_vertical_slider,
};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::expander::*;

/// State for rendering the Expander plugin
pub struct ExpanderRenderState {
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
    pub sidechain_hpf_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Expander plugin
pub fn render_expander_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: ExpanderRenderState,
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
                .gap_6()
                .items_start()
                // Column 1: Main dynamics controls (sliders)
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
                                .child(render_vertical_slider(
                                    entity.clone(),
                                    plugin_idx,
                                    "Threshold",
                                    state.threshold_db,
                                    THRESHOLD_MIN as f64,
                                    THRESHOLD_MAX as f64,
                                    "dB",
                                    0,
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
                                    1,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('r'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    entity.clone(),
                                    plugin_idx,
                                    "Range",
                                    state.range_db,
                                    RANGE_MIN as f64,
                                    RANGE_MAX as f64,
                                    "dB",
                                    4,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('g'),
                                    theme,
                                )),
                        ),
                )
                // Column 2: Timing controls
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
                                .child(render_vertical_slider(
                                    entity.clone(),
                                    plugin_idx,
                                    "Attack",
                                    state.attack_ms,
                                    ATTACK_MIN as f64,
                                    ATTACK_MAX as f64,
                                    "ms",
                                    2,
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
                                    3,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('e'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    entity.clone(),
                                    plugin_idx,
                                    "Hold",
                                    state.hold_ms,
                                    HOLD_MIN as f64,
                                    HOLD_MAX as f64,
                                    "ms",
                                    7,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('h'),
                                    theme,
                                )),
                        ),
                )
                // Column 3: Output and advanced controls
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
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .child(render_toggle_button(
                                    entity.clone(),
                                    plugin_idx,
                                    state.link_channels,
                                    9, // link_channels param index
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                )),
                        )
                        // Knee knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Knee",
                            state.knee_db,
                            KNEE_MIN as f64,
                            KNEE_MAX as f64,
                            "dB",
                            5,
                            state.selected_param,
                            state.is_editing,
                            Some('k'),
                            theme,
                        ))
                        // Hysteresis knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Hysteresis",
                            state.hysteresis_db,
                            HYSTERESIS_MIN as f64,
                            HYSTERESIS_MAX as f64,
                            "dB",
                            6,
                            state.selected_param,
                            state.is_editing,
                            Some('y'),
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
                            8,
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        ))
                        // SC HPF knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "SC HPF",
                            state.sidechain_hpf_hz,
                            SIDECHAIN_HPF_HZ_MIN as f64,
                            SIDECHAIN_HPF_HZ_MAX as f64,
                            "Hz",
                            10,
                            state.selected_param,
                            state.is_editing,
                            Some('s'),
                            theme,
                        )),
                ),
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
