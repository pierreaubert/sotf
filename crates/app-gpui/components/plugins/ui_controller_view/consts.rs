use gpui::prelude::*;
use gpui::*;

pub(super) const CELL_W: f32 = 92.0;

pub(super) const CELL_H: f32 = 110.0;

pub(super) const FADER_CELL_H: f32 = 180.0;

pub(super) const BUTTON_SIZE: f32 = 38.0;

pub(super) fn empty_cell(cell_width: f32, row_height: f32) -> impl IntoElement {
    div()
        .w(px(cell_width))
        .h(px(row_height))
        // Keep the physical grid width intact so the controller viewport can
        // scroll horizontally instead of compressing its empty slots.
        .flex_shrink_0()
}
