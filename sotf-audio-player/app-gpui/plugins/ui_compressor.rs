//! Compressor Plugin UI Component
//!
//! Professional compressor visualization with:
//! - Transfer curve display
//! - Gain reduction meter
//! - Rotary knob controls with keyboard shortcuts

use super::common::{
    render_edit_hints, render_knob, render_section_header, render_toggle, render_transfer_curve,
    ParamSectionStyle,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::compressor::*;

/// State for rendering the Compressor plugin
pub struct CompressorRenderState {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub knee_db: f64,
    pub makeup_gain_db: f64,
    pub mix: f64,
    pub auto_makeup: bool,
    pub link_channels: bool,
    pub sidechain_hpf_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Compressor plugin
pub fn render_compressor_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: CompressorRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Simulated gain reduction (in real implementation, this would come from the audio engine)
    let simulated_gr = if state.threshold_db < -10.0 {
        (state.threshold_db + 10.0) * 0.5
    } else {
        0.0
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Knobs and Transfer Curve side by side
        .child(
            div()
                .flex()
                .gap_4()
                .items_start()
                // Transfer curve and options
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .items_center()
                        .param_section_style_lg(theme)
                        // Toggles above the graph
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(render_toggle(
                                    entity.clone(),
                                    plugin_idx,
                                    "Auto Makeup",
                                    state.auto_makeup,
                                    7, // param index for auto_makeup
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                ))
                                .child(render_toggle(
                                    entity.clone(),
                                    plugin_idx,
                                    "Link Channels",
                                    state.link_channels,
                                    8, // param index for link_channels
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                )),
                        )
                        .child(render_transfer_curve(
                            state.threshold_db,
                            state.ratio,
                            state.knee_db,
                            false,
                            theme,
                        ))
                        // Gain reduction meter
                        .child(
                            div()
                                .w_full()
                                .child(render_gr_meter(simulated_gr, -30.0, theme)),
                        ),
                )
                // Parameters section with knobs
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .flex_1()
                        .param_section_style_lg(theme)
                        .child(render_section_header("DYNAMICS CONTROL", theme))
                        // Row 1: Threshold, Ratio, Attack
                        .child(
                            div()
                                .flex()
                                .gap_4()
                                .justify_center()
                                .child(render_knob(
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
                                .child(render_knob(
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
                                .child(render_knob(
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
                                )),
                        )
                        // Row 2: Release, Knee, Makeup
                        .child(
                            div()
                                .flex()
                                .gap_4()
                                .justify_center()
                                .child(render_knob(
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
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Knee",
                                    state.knee_db,
                                    KNEE_MIN as f64,
                                    KNEE_MAX as f64,
                                    "dB",
                                    4,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('k'),
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Makeup",
                                    state.makeup_gain_db,
                                    MAKEUP_GAIN_MIN as f64,
                                    MAKEUP_GAIN_MAX as f64,
                                    "dB",
                                    5,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('m'),
                                    theme,
                                )),
                        )
                        // Row 3: Mix (centered)
                        .child(div().flex().gap_4().justify_center().child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix,
                            MIX_MIN as f64,
                            MIX_MAX as f64,
                            "%",
                            6,
                            state.selected_param,
                            state.is_editing,
                            Some('x'),
                            theme,
                        )))
                        // Sidechain HPF display
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_1()
                                .p_2()
                                .rounded_lg()
                                .bg(theme.background)
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("Sidechain HPF"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_primary)
                                        .child(format!("{:.0} Hz", state.sidechain_hpf_hz)),
                                ),
                        ),
                ),
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
