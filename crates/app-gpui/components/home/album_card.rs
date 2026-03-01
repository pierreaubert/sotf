//! Album Card Component
//!
//! A reusable RenderOnce component for displaying album information.
//! Used in both grid and list views.

use crate::app::AppState;
use crate::components::icons::{Icon, IconName};
use crate::theme::Theme;
use crate::ui::ALBUM_CARD_WIDTH_REMS;
use gpui::prelude::*;
use gpui::*;

use sotf_audio_player::Album;
use std::sync::Arc;

/// Create an image from thumbnail bytes
///
/// Thumbnails are stored as PNG in the database for optimal rendering.
/// This function handles both PNG (new format) and JPEG (legacy format) for backward compatibility.
fn image_from_jpeg_bytes(bytes: &[u8]) -> Arc<Image> {
    use image::ImageFormat as ExternalImageFormat;

    // Try PNG first (new default format)
    if let Ok(_img) = image::load_from_memory_with_format(bytes, ExternalImageFormat::Png) {
        return Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec()));
    }

    // Fallback: try JPEG (legacy format) and convert to PNG
    if let Ok(img) = image::load_from_memory_with_format(bytes, ExternalImageFormat::Jpeg) {
        let mut png_bytes = Vec::new();
        if img
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                ExternalImageFormat::Png,
            )
            .is_ok()
        {
            return Arc::new(Image::from_bytes(ImageFormat::Png, png_bytes));
        }
    }

    // Last resort: pass through as-is
    Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec()))
}

/// Album card display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumCardMode {
    /// Grid card with thumbnail (150px wide)
    Grid,
    /// List row (full width)
    List,
    /// Compact list row (for tree view children)
    Compact,
}

/// A single album card for the library view
#[derive(IntoElement)]
pub struct AlbumCard {
    /// Album data
    album: Arc<Album>,
    /// Index in the list
    index: usize,
    /// Whether this card is selected
    is_selected: bool,
    /// Display mode
    mode: AlbumCardMode,
    /// Theme reference
    theme: Theme,
    /// App state entity for favorite toggling
    state: Option<Entity<AppState>>,
}

impl AlbumCard {
    /// Create a new album card
    pub fn new(album: Arc<Album>, index: usize, is_selected: bool, theme: Theme) -> Self {
        Self {
            album,
            index,
            is_selected,
            mode: AlbumCardMode::Grid,
            theme,
            state: None,
        }
    }

    /// Set the display mode
    pub fn mode(mut self, mode: AlbumCardMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the app state entity for favorite toggling
    pub fn state(mut self, state: Entity<AppState>) -> Self {
        self.state = Some(state);
        self
    }
}

impl RenderOnce for AlbumCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        match self.mode {
            AlbumCardMode::Grid => self.render_grid(),
            AlbumCardMode::List => self.render_list(),
            AlbumCardMode::Compact => self.render_compact(),
        }
    }
}

impl AlbumCard {
    /// Get the audio format (e.g., "FLAC", "MP3") from the first track
    fn get_format(album: &Album) -> Option<String> {
        album.tracks.first().and_then(|t| {
            t.path
                .extension()
                .and_then(|ext| ext.to_str().map(|s| s.to_uppercase()))
        })
    }

    /// Format the sample rate and bit depth for display (e.g., "24/44.1k", "16/48k")
    fn format_sample_info(album: &Album) -> Option<String> {
        album.tracks.first().and_then(|t| {
            match (t.bit_depth, t.sample_rate) {
                (Some(bits), Some(rate)) => {
                    // Format sample rate: 44100 -> "44.1k", 48000 -> "48k", 96000 -> "96k"
                    let rate_str = if rate % 1000 == 0 {
                        format!("{}k", rate / 1000)
                    } else {
                        format!("{:.1}k", rate as f64 / 1000.0)
                    };
                    Some(format!("{}/{}", bits, rate_str))
                }
                (Some(bits), None) => Some(format!("{}bit", bits)),
                (None, Some(rate)) => {
                    let rate_str = if rate % 1000 == 0 {
                        format!("{}k", rate / 1000)
                    } else {
                        format!("{:.1}k", rate as f64 / 1000.0)
                    };
                    Some(rate_str)
                }
                (None, None) => None,
            }
        })
    }

    /// Format the dynamic range for display
    fn format_dr(dr: Option<f64>) -> Option<String> {
        dr.map(|d| format!("{:.0}", d))
    }

    /// Build a clickable heart icon for favorite toggling
    fn build_heart_icon(
        album: &Album,
        index: usize,
        theme: &Theme,
        state: &Option<Entity<AppState>>,
    ) -> Stateful<Div> {
        let is_fav = album.is_favorite;
        let album_id = album.id;
        let heart = div()
            .id(SharedString::from(format!("album-fav-{}", index)))
            .cursor_pointer()
            .child(
                Icon::new(if is_fav {
                    IconName::HeartFilled
                } else {
                    IconName::Heart
                })
                .xs()
                .color(if is_fav {
                    theme.accent
                } else {
                    theme.text_muted
                }),
            );

        if let (Some(aid), Some(state)) = (album_id, state.clone()) {
            heart.on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state.update(cx, |state, _cx| {
                    state.app.toggle_album_favorite(aid);
                });
            })
        } else {
            heart
        }
    }

    /// Build the metadata line: "FLAC 24/44.1k [DR icon]14 #20 [heart]"
    fn build_metadata_line(
        album: &Album,
        index: usize,
        theme: &Theme,
        state: &Option<Entity<AppState>>,
    ) -> impl IntoElement {
        let format = Self::get_format(album);
        let sample_info = Self::format_sample_info(album);
        let dr = Self::format_dr(album.dynamic_range);
        let track_count = album.tracks.len();

        div()
            .flex()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(theme.text_muted)
            // Format (e.g., FLAC) - no wrapper div needed
            .when_some(format, |d, fmt| d.child(fmt))
            // Sample rate info (e.g., 24/44.1k)
            .when_some(sample_info, |d, info| d.child(info))
            // Dynamic range with icon
            .when_some(dr, |d, dr_val| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_px()
                        .child(
                            Icon::new(IconName::AudioWaveform)
                                .xs()
                                .color(theme.text_muted),
                        )
                        .child(dr_val),
                )
            })
            // Track count
            .child("#")
            .child(track_count.to_string())
            // Favorite heart (always visible)
            .child(Self::build_heart_icon(album, index, theme, state))
    }

    fn render_grid(self) -> AnyElement {
        // Card and thumbnail share the same width (card is sized by its thumbnail)
        let size_rems = ALBUM_CARD_WIDTH_REMS;
        let theme = self.theme;
        let album = self.album;
        let index = self.index;
        let state = self.state;

        // Metadata for grid view (smaller font)
        let format = Self::get_format(&album);
        let sample_info = Self::format_sample_info(&album);
        let dr = Self::format_dr(album.dynamic_range);
        let track_count = album.tracks.len();

        div()
            .id(("album-card", index))
            .w(rems(size_rems))
            .rounded_lg()
            .cursor_pointer()
            .when(self.is_selected, |d| {
                // Glow effect for selected state
                let mut bg = theme.accent;
                bg.a = 0.1;
                d.shadow_md().border_1().border_color(theme.accent).bg(bg)
            })
            // No background for unselected cards (transparent)
            .hover(|style| style.bg(theme.surface_hover))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    // Album art thumbnail or placeholder (scales with rem)
                    .child(
                        div()
                            .w(rems(size_rems))
                            .h(rems(size_rems))
                            .rounded_md()
                            .overflow_hidden()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                if let Some(ref thumbnail_bytes) = album.album_art_thumbnail {
                                    img(image_from_jpeg_bytes(thumbnail_bytes))
                                        .w(rems(size_rems))
                                        .h(rems(size_rems))
                                        .object_fit(ObjectFit::Cover)
                                        .into_any_element()
                                } else {
                                    div()
                                        .text_3xl()
                                        .text_color(theme.text_muted)
                                        .child("♪")
                                        .into_any_element()
                                },
                            ),
                    )
                    // Album title
                    .child(
                        div()
                            .w_full()
                            .mt_1()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(album.title.clone()),
                    )
                    // Artist name
                    .child(
                        div()
                            .w_full()
                            .text_size(rems(0.625))
                            .text_color(theme.text_secondary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(album.artist()),
                    )
                    // Metadata line: FORMAT SAMPLE DR #count [heart]
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_size(rems(0.5625))
                            .text_color(theme.text_muted)
                            // Format (e.g., FLAC) - pass string directly, no wrapper div
                            .when_some(format, |d, fmt| d.child(fmt))
                            // Sample rate info (e.g., 24/44.1k)
                            .when_some(sample_info, |d, info| d.child(info))
                            // Dynamic range with icon - keep wrapper for flex layout
                            .when_some(dr, |d, dr_val| {
                                d.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_px()
                                        .child(
                                            Icon::new(IconName::AudioWaveform)
                                                .xs()
                                                .color(theme.text_muted),
                                        )
                                        .child(dr_val),
                                )
                            })
                            // Track count - use static prefix
                            .child("#")
                            .child(track_count.to_string())
                            // Favorite heart (always visible, clickable)
                            .child(Self::build_heart_icon(&album, index, &theme, &state)),
                    ),
            )
            .into_any_element()
    }

    fn render_list(self) -> AnyElement {
        let theme = self.theme;
        let album = self.album;
        let index = self.index;
        let state = self.state;

        div()
            .id(("album-row", index))
            .w_full()
            .p_3()
            .rounded_md()
            .when(self.is_selected, |d| {
                // Glow effect for selected state
                let mut bg = theme.accent;
                bg.a = 0.1;
                d.shadow_md().border_1().border_color(theme.accent).bg(bg)
            })
            // No background for unselected cards (transparent)
            .hover(|style| style.bg(theme.surface_hover))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(album.title.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_secondary)
                            .child(album.artist()),
                    )
                    // Metadata line: FORMAT [DR icon]DR #count [heart]
                    .child(Self::build_metadata_line(&album, index, &theme, &state)),
            )
            .into_any_element()
    }

    fn render_compact(self) -> AnyElement {
        let theme = self.theme;
        let album = self.album;
        let index = self.index;
        let state = self.state;

        div()
            .id(("album-compact", index))
            .w_full()
            .pl_8()
            .p_2()
            .rounded_md()
            .when(self.is_selected, |d| {
                // Glow effect for selected state
                let mut bg = theme.accent;
                bg.a = 0.1;
                d.shadow_md().border_1().border_color(theme.accent).bg(bg)
            })
            // No background for unselected cards (transparent)
            .hover(|style| style.bg(theme.surface_hover))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .w_full()
                    .child(
                        div().flex().flex_col().child(album.title.clone()).child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(album.artist()),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .text_color(theme.text_secondary)
                            .child("#")
                            .child(album.tracks.len().to_string())
                            .child(Self::build_heart_icon(&album, index, &theme, &state)),
                    ),
            )
            .into_any_element()
    }
}

/// Render height in pixels for a given album card mode
pub fn album_card_height(mode: AlbumCardMode) -> f32 {
    match mode {
        AlbumCardMode::Grid => 180.0,   // thumbnail + text
        AlbumCardMode::List => 80.0,    // full row
        AlbumCardMode::Compact => 56.0, // compact row
    }
}
