//! Polyphonic Note Detection (PND) Plugin UI Component
//!
//! Automatic pitch drift correction:
//! - Correction Strength - How much to correct detected drift
//! - Analysis Window - Time window for pitch analysis
//! - Drift Smoothing - Smoothing factor for drift detection

use super::common::{render_knob, render_section_title};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{find_by_key as pk, pnd::PARAMS as PN};

/// State for rendering the PND plugin
pub struct PndRenderState {
    pub correction_strength: f64,
    pub analysis_window_ms: f64,
    pub drift_smoothing: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the PND plugin
pub fn render_pnd_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: PndRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - single row of knobs
        .child(
            div()
                .flex()
                .gap_6()
                // Column 1: Correction Strength
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("CORRECTION", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Strength",
                            state.correction_strength * 100.0,
                            pk(PN, "correction_strength").min_f64() * 100.0,
                            pk(PN, "correction_strength").max_f64() * 100.0,
                            "%",
                            0,
                            state.selected_param,
                            state.is_editing,
                            Some('s'),
                            theme,
                        )),
                )
                // Column 2: Analysis parameters
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("ANALYSIS", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Window",
                            state.analysis_window_ms,
                            pk(PN, "analysis_window_ms").min_f64(),
                            pk(PN, "analysis_window_ms").max_f64(),
                            "ms",
                            1,
                            state.selected_param,
                            state.is_editing,
                            Some('w'),
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Smoothing",
                            state.drift_smoothing * 1000.0,
                            pk(PN, "drift_smoothing").min_f64() * 1000.0,
                            pk(PN, "drift_smoothing").max_f64() * 1000.0,
                            "",
                            2,
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        )),
                ),
        )
    // .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
