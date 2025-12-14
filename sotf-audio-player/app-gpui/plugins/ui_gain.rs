//! Gain Plugin UI Component
//!
//! Clean gain control with:
//! - Rotary knob control
//! - Large circular gain display
//! - Color-coded boost/cut/unity indication

use super::common::{render_edit_hints, render_knob, render_section_header};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::gain::*;

/// State for rendering the Gain plugin
pub struct GainRenderState {
    pub gain_db: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Gain plugin
pub fn render_gain_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: GainRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let is_boost = state.gain_db > 0.5;
    let is_cut = state.gain_db < -0.5;

    // Color based on gain direction
    let gain_color = if is_boost {
        theme.success // Green for boost
    } else if is_cut {
        theme.error // Red for cut
    } else {
        theme.text_primary // Neutral
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Large gain display and knob
        .child(
            div()
                .flex()
                .gap_4()
                .items_center()
                .justify_center()
                // Knob section
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
                        .items_center()
                        .child(render_section_header("GAIN CONTROL", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Gain",
                            state.gain_db,
                            GAIN_DB_MIN as f64,
                            GAIN_DB_MAX as f64,
                            "dB",
                            0,
                            state.selected_param,
                            state.is_editing,
                            Some('g'),
                            theme,
                        )),
                )
        )
}
