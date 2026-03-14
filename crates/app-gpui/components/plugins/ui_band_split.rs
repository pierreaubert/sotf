//! Band Split Plugin UI Component
//!
//! Controls for frequency band splitting with:
//! - Crossover frequency (Hz)
//! - Crossover type (LR24/LR48)

use super::common::{render_knob, render_section_title};
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{band_split::PARAMS as BS, find_by_key as pk};

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
                            pk(BS, "frequency").min_f64(),
                            pk(BS, "frequency").max_f64(),
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
                                .child(div().text_xs().text_color(theme.text_muted).child("Type"))
                                .child({
                                    let is_lr24 = state.crossover_type != "LR48";
                                    let is_lr48 = state.crossover_type == "LR48";
                                    let accent = theme.accent;
                                    let surface = theme.surface;
                                    let text_on_accent = theme.text_on_accent;
                                    let text_muted = theme.text_muted;
                                    let entity_a = entity.clone();
                                    let entity_b = entity.clone();
                                    div()
                                        .flex()
                                        .gap(px(1.0))
                                        .rounded_md()
                                        .border_1()
                                        .border_color(theme.border)
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .id("xover-lr24")
                                                .px_2()
                                                .py_1()
                                                .text_xs()
                                                .cursor_pointer()
                                                .bg(if is_lr24 { accent } else { surface })
                                                .text_color(if is_lr24 { text_on_accent } else { text_muted })
                                                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                                                    entity_a.update(cx, |state, cx| {
                                                        state.app.set_plugin_param(plugin_idx, 1, 0.0);
                                                        cx.notify();
                                                    });
                                                })
                                                .child("LR24"),
                                        )
                                        .child(
                                            div()
                                                .id("xover-lr48")
                                                .px_2()
                                                .py_1()
                                                .text_xs()
                                                .cursor_pointer()
                                                .bg(if is_lr48 { accent } else { surface })
                                                .text_color(if is_lr48 { text_on_accent } else { text_muted })
                                                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                                                    entity_b.update(cx, |state, cx| {
                                                        state.app.set_plugin_param(plugin_idx, 1, 1.0);
                                                        cx.notify();
                                                    });
                                                })
                                                .child("LR48"),
                                        )
                                }),
                        ),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .italic()
                .child("Splits audio into low and high frequency bands for parallel processing."),
        )
}
