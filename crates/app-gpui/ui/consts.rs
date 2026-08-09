use super::misc::compute_responsive_scale;

/// Album card width in rems (~140px at default 16px rem). Used in album_card.rs grid rendering
/// and recalculate_pagination column estimation.
pub(crate) const ALBUM_CARD_WIDTH_REMS: f32 = 8.75;

/// Album card height in rems (~180px at 16px rem, thumbnail + text below).
pub(crate) const ALBUM_CARD_HEIGHT_REMS: f32 = 11.25;

/// Gap between album cards in rems (matches gap_4 = 1rem in library.rs grid).
pub(crate) const ALBUM_CARD_GAP_REMS: f32 = 1.0;

/// Footer height in rems (~100px at 16px rem). Used for footer sizing and positioning
/// popups (device popup, studio menu) above the footer.
pub(crate) const FOOTER_HEIGHT_REMS: f32 = 6.25;

/// Total vertical chrome height in rems, used by recalculate_pagination to estimate the
/// available grid area. Breakdown:
///   Header ~2.5rem + Stats ~6.25rem + Filter ~2.5rem + Pagination ~3.125rem + Footer ~3.625rem
pub(crate) const CHROME_HEIGHT_REMS: f32 = 18.0;

/// Estimate the number of album grid columns and rows that fit in the given window.
///
/// This is the pure computation behind `recalculate_pagination` — extracted so it can be
/// unit-tested without constructing a full `PlayerView`.
pub fn estimate_grid_dimensions(
    window_width: f32,
    window_height: f32,
    font_scale: f32,
    min_font_size_px: Option<f32>,
    max_font_size_px: Option<f32>,
) -> (usize, usize) {
    let responsive_scale = compute_responsive_scale(window_width, window_height);
    let (scale_min, scale_max) = combined_scale_bounds(min_font_size_px, max_font_size_px);
    let combined_scale = (font_scale * responsive_scale).clamp(scale_min, scale_max);
    let effective_rem = 16.0 * combined_scale;

    let card_with_gap = (ALBUM_CARD_WIDTH_REMS + ALBUM_CARD_GAP_REMS) * effective_rem;
    // Approximate total horizontal chrome: grid p_2 (0.5rem × 2 sides) + parent padding
    let available_width = window_width - 2.0 * effective_rem;
    let columns = (available_width / card_with_gap).floor().max(1.0) as usize;

    let chrome_height = CHROME_HEIGHT_REMS * effective_rem;
    let available_height = (window_height - chrome_height).max(16.0 * effective_rem);
    let card_height = ALBUM_CARD_HEIGHT_REMS * effective_rem;
    let rows = (available_height / card_height).floor().max(1.0) as usize;

    (columns, rows)
}

/// Default minimum font size in pixels.
/// Keep the responsive baseline large enough that 0.625-rem captions remain
/// readable at the minimum supported desktop window size.
pub const DEFAULT_MIN_FONT_SIZE_PX: f32 = 12.0;

/// Default maximum font size in pixels.
pub const DEFAULT_MAX_FONT_SIZE_PX: f32 = 32.0;

/// Convert min/max font size in pixels to combined scale bounds.
/// Uses defaults when `None` is provided.
pub fn combined_scale_bounds(min_px: Option<f32>, max_px: Option<f32>) -> (f32, f32) {
    let min = min_px.unwrap_or(DEFAULT_MIN_FONT_SIZE_PX) / 16.0;
    let max = max_px.unwrap_or(DEFAULT_MAX_FONT_SIZE_PX) / 16.0;
    (min, max)
}
