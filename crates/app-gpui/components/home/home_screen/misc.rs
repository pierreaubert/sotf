use super::home_album_ext::HomeAlbumExt;
use crate::ui::PlayerView;
use crate::ui::{
    ALBUM_CARD_GAP_REMS, ALBUM_CARD_HEIGHT_REMS, ALBUM_CARD_WIDTH_REMS, CHROME_HEIGHT_REMS,
    combined_scale_bounds, compute_responsive_scale,
};
use sotf_audio_player::Album;
use std::sync::Arc;

const HOME_SHELF_BASE_REM_PX: f32 = 16.0;

pub(super) const EXPANDED_ALBUM_LIMIT: usize = 24;

pub(super) fn collapsed_album_limit_for_width(window_width: f32) -> usize {
    let card_width = ALBUM_CARD_WIDTH_REMS * HOME_SHELF_BASE_REM_PX;
    let card_gap = ALBUM_CARD_GAP_REMS * HOME_SHELF_BASE_REM_PX;
    let available = (window_width - 2.0 * HOME_SHELF_BASE_REM_PX).max(card_width);
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
    let responsive_scale = compute_responsive_scale(window_width, window_height);
    let (scale_min, scale_max) = combined_scale_bounds(min_font_size_px, max_font_size_px);
    let combined_scale = (font_scale * responsive_scale).clamp(scale_min, scale_max);
    let effective_rem = 16.0 * combined_scale;

    let card_with_gap = (ALBUM_CARD_WIDTH_REMS + ALBUM_CARD_GAP_REMS) * effective_rem;
    let available_width = (window_width - 2.0 * effective_rem).max(card_with_gap);
    let columns = (available_width / card_with_gap).floor().max(1.0) as usize;

    // Home screen has less chrome than the library view (no stats/filter bar),
    // but shelf titles and the footer still consume space. Use the same chrome
    // estimate as the library grid as a conservative lower bound.
    let chrome_height = CHROME_HEIGHT_REMS * effective_rem;
    let available_height = (window_height - chrome_height).max(16.0 * effective_rem);
    let card_height = ALBUM_CARD_HEIGHT_REMS * effective_rem;
    let rows = (available_height / card_height).floor().max(1.0) as usize;

    // Show enough rows to fill the viewport plus one extra row of buffering.
    (columns * rows.saturating_add(1)).max(EXPANDED_ALBUM_LIMIT)
}

pub(super) fn add_home_album_to_queue(
    state: &mut crate::app::AppState,
    album: &Album,
    play_now: bool,
) {
    if let Some(id) = album.id
        && let Some(filtered_idx) = state
            .app
            .filtered_albums()
            .iter()
            .position(|candidate| candidate.id == Some(id))
    {
        state.app.library_state.selected_index = filtered_idx;
        let result = if play_now {
            state.app.play_album_now()
        } else {
            state.app.add_album_to_queue()
        };

        match result {
            Ok(Some(path)) => PlayerView::play_track(state, path),
            Ok(None) => {}
            Err(e) => {
                if e.starts_with("None of the files") {
                    remove_home_album_from_view(state, album);
                    state.app.ui_state.toast_message = Some(
                        crate::app::ToastMessage::persistent(e, crate::app::ToastType::Warning)
                            .with_action(crate::app::ToastAction::new("Rescan", "rescan-library")),
                    );
                } else {
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(e));
                }
            }
        }
    }
}

fn remove_home_album_from_view(state: &mut crate::app::AppState, album: &Album) {
    let before = state.app.library_state.library.albums.len();
    state.app.library_state.library.albums.retain(|candidate| {
        if let (Some(candidate_id), Some(album_id)) = (candidate.id, album.id) {
            candidate_id != album_id
        } else {
            candidate.title != album.title || candidate.artist() != album.artist()
        }
    });

    if state.app.library_state.library.albums.len() != before {
        state.app.library_state.invalidate_cache();
        let len = state.app.filtered_albums().len();
        if len == 0 {
            state.app.library_state.selected_index = 0;
        } else if state.app.library_state.selected_index >= len {
            state.app.library_state.selected_index = len - 1;
        }
        state.app.invalidate_library_stats();
    }
}

pub(super) fn sort_album_refs_by_listening<'a>(mut albums: Vec<&'a Album>) -> Vec<&'a Album> {
    albums.sort_by(|a, b| {
        b.play_count
            .cmp(&a.play_count)
            .then_with(|| a.artist().cmp(&b.artist()))
            .then_with(|| a.title.cmp(&b.title))
    });
    albums
}

pub(super) fn prioritize_cover_refs<'a>(mut albums: Vec<&'a Album>) -> Vec<&'a Album> {
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
