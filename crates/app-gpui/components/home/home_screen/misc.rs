use super::home_album_ext::HomeAlbumExt;
use crate::ui::{ALBUM_CARD_GAP_REMS, ALBUM_CARD_WIDTH_REMS, estimate_grid_dimensions};
use sotf_audio_player::Album;
use std::sync::Arc;

const HOME_SHELF_CONTENT_RESERVE_PX: f32 = 192.0;
const HOME_SHELF_BASE_REM_PX: f32 = 16.0;

pub(super) const EXPANDED_ALBUM_LIMIT: usize = 24;

pub(super) fn collapsed_album_limit_for_width(window_width: f32) -> usize {
    let card_width = ALBUM_CARD_WIDTH_REMS * HOME_SHELF_BASE_REM_PX;
    let card_gap = ALBUM_CARD_GAP_REMS * HOME_SHELF_BASE_REM_PX;
    let available = (window_width - HOME_SHELF_CONTENT_RESERVE_PX).max(card_width);
    let slot = card_width + card_gap;
    (((available + card_gap) / slot).floor() as usize).max(1)
}

/// Compute how many albums an expanded home shelf should display so that it
/// covers the available viewport. Mirrors the logic used by the library/search
/// grid (`crate::ui::estimate_grid_dimensions`) but tailored for the home
/// screen chrome (sidebar + shelf headers).
pub(super) fn expanded_album_limit_for_dimensions(
    window_width: f32,
    window_height: f32,
    font_scale: f32,
    min_font_size_px: Option<f32>,
    max_font_size_px: Option<f32>,
) -> usize {
    let (columns, rows) = estimate_grid_dimensions(
        window_width,
        window_height,
        font_scale,
        min_font_size_px,
        max_font_size_px,
    );

    // Show enough rows to fill the viewport plus one extra row of buffering.
    (columns * rows.saturating_add(1)).max(EXPANDED_ALBUM_LIMIT)
}

pub(super) fn sort_album_refs_by_listening(mut albums: Vec<&Album>) -> Vec<&Album> {
    albums.sort_by(|a, b| {
        b.play_count
            .cmp(&a.play_count)
            .then_with(|| a.artist().cmp(&b.artist()))
            .then_with(|| a.title.cmp(&b.title))
    });
    albums
}

pub(super) fn prioritize_cover_refs(mut albums: Vec<&Album>) -> Vec<&Album> {
    albums.sort_by(|a, b| {
        b.has_cover()
            .cmp(&a.has_cover())
            .then_with(|| b.play_count.cmp(&a.play_count))
            .then_with(|| a.artist().cmp(&b.artist()))
            .then_with(|| a.title.cmp(&b.title))
    });
    albums
}

pub(super) fn arc_album_refs(albums: &[&Album], limit: usize) -> Vec<Arc<Album>> {
    albums
        .iter()
        .take(limit)
        .map(|album| Arc::new((*album).clone()))
        .collect()
}

pub(super) fn stable_album_hash(album: &Album) -> u64 {
    let mut hash = 14_695_981_039_346_656_037u64;
    for byte in format!("{}:{}:{:?}", album.artist(), album.title, album.year).bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

pub(super) fn slug(label: &str) -> String {
    label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
