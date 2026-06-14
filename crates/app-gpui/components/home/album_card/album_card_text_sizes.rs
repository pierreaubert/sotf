use super::misc::text_size_at_least;
use crate::components::design::Ds;
use gpui::*;

#[derive(Clone, Copy)]
pub(super) struct AlbumCardTextSizes {
    pub(super) title: Rems,
    pub(super) body: Rems,
    pub(super) metadata: Rems,
}

impl AlbumCardTextSizes {
    pub(super) fn from_design(d: Ds, min_font_size_px: f32, effective_rem_px: f32) -> Self {
        Self {
            title: text_size_at_least(d.text_base, min_font_size_px, effective_rem_px),
            body: text_size_at_least(d.text_base, min_font_size_px, effective_rem_px),
            metadata: text_size_at_least(d.text_sm, min_font_size_px, effective_rem_px),
        }
    }
}
