//! Downmix Plugin UI Component
//!
//! Controls for surround to stereo downmix with:
//! - Channel group gains (Center, Surround, Height, LFE)
//! - Phase coherence toggle
//! - Frequency blending controls

use super::common::{render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{downmix::PARAMS as DM, find_by_key as pk};

/// State for rendering the Downmix plugin
pub struct DownmixRenderState {
    pub center_gain_db: f64,
    pub surround_gain_db: f64,
    pub height_gain_db: f64,
    pub lfe_gain_db: f64,
    pub phase_coherence: bool,
    pub phase_blend_low_hz: f64,
    pub phase_blend_high_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Downmix plugin
pub fn render_downmix_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: DownmixRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(d.section)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(render_section_title(d, "CHANNEL GAINS", theme))
                .child(
                    div()
                        .flex()
                        .gap(d.section)
                        .justify_around()
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Center",
                            state.center_gain_db,
                            pk(DM, "center_gain_db").min_f64(),
                            pk(DM, "center_gain_db").max_f64(),
                            "dB",
                            0,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Surround",
                            state.surround_gain_db,
                            pk(DM, "surround_gain_db").min_f64(),
                            pk(DM, "surround_gain_db").max_f64(),
                            "dB",
                            1,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Height",
                            state.height_gain_db,
                            pk(DM, "height_gain_db").min_f64(),
                            pk(DM, "height_gain_db").max_f64(),
                            "dB",
                            2,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "LFE",
                            state.lfe_gain_db,
                            pk(DM, "lfe_gain_db").min_f64(),
                            pk(DM, "lfe_gain_db").max_f64(),
                            "dB",
                            3,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(render_section_title(d, "PHASE COHERENCE", theme))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(d.section)
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Enable FFT Phase Alignment",
                            state.phase_coherence,
                            4,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        )),
                )
                .when(state.phase_coherence, |el| {
                    el.child(
                        div()
                            .flex()
                            .gap(d.section)
                            .justify_around()
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Blend Low",
                                state.phase_blend_low_hz,
                                pk(DM, "phase_blend_low_hz").min_f64(),
                                pk(DM, "phase_blend_low_hz").max_f64(),
                                "Hz",
                                5,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Blend High",
                                state.phase_blend_high_hz,
                                pk(DM, "phase_blend_high_hz").min_f64(),
                                pk(DM, "phase_blend_high_hz").max_f64(),
                                "Hz",
                                6,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            )),
                    )
                }),
        )
}
