//! Band Merge Plugin UI Component
//!
//! Controls for frequency band merging with:
//! - Number of bands to merge

use super::common::{render_knob, render_section_title};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{find_by_key as pk, band_merge::PARAMS as BM};

/// State for rendering the BandMerge plugin
pub struct BandMergeRenderState {
    pub bands: usize,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the BandMerge plugin
pub fn render_band_merge_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: BandMergeRenderState,
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
                .child(render_section_title("MERGE CONFIG", theme))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Number of Bands",
                            state.bands as f64,
                            pk(BM, "bands").min_f64(),
                            pk(BM, "bands").max_f64(),
                            "",
                            0,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .italic()
                .child("Merges multiple frequency bands back together by summation."),
        )
}
