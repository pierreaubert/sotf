//! Gain Plugin UI Component
//!
//! Clean gain control with:
//! - Rotary knob control
//! - Large circular gain display
//! - Color-coded boost/cut/unity indication

use super::common::{render_knob, render_section_title};
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
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(render_section_title("GAIN CONTROL", theme))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
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
}
