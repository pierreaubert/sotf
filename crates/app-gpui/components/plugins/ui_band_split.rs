//! Band Split Plugin UI Component
//!
//! Controls for frequency band splitting with:
//! - Crossover frequency (Hz)
//! - Crossover type (LR24/LR48)

use super::common::{render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::band_split::*;

/// State for rendering the BandSplit plugin
pub struct BandSplitRenderState {
    pub frequency: f64,
    pub crossover_type: String,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the BandSplit plugin
pub fn render_band_split_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: BandSplitRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title("CROSSOVER", theme))
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .items_center()
                        .justify_around()
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Frequency",
                            state.frequency,
                            FREQUENCY_MIN,
                            FREQUENCY_MAX,
                            "Hz",
                            0,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .items_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("Type")
                                )
                                .child(render_toggle(
                                    entity.clone(),
                                    plugin_idx,
                                    if state.crossover_type == "LR48" { "LR48 (48dB)" } else { "LR24 (24dB)" },
                                    state.crossover_type == "LR48",
                                    1,
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                ))
                        ),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .italic()
                .child("Splits audio into low and high frequency bands for parallel processing.")
        )
}
