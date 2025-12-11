//! Album Card Component
//!
//! A reusable RenderOnce component for displaying album information.
//! Used in both grid and list views.

use crate::theme::Theme;
use crate::ui::components::icon::{Icon, IconName};
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
            .write_to(&mut std::io::Cursor::new(&mut png_bytes), ExternalImageFormat::Png)
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
        }
    }

    /// Set the display mode
    pub fn mode(mut self, mode: AlbumCardMode) -> Self {
        self.mode = mode;
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

    /// Build the metadata line: "FLAC 24/44.1k [DR icon]14 #20"
    fn build_metadata_line(album: &Album, theme: &Theme) -> impl IntoElement {
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
            // Format (e.g., FLAC)
            .when_some(format, |d, fmt| d.child(div().child(fmt)))
            // Sample rate info (e.g., 24/44.1k)
            .when_some(sample_info, |d, info| d.child(div().child(info)))
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
            .child(format!("#{}", track_count))
    }

    fn render_grid(self) -> AnyElement {
        let thumbnail_size = 140.0;
        let card_width = 140.0;
        let theme = self.theme;
        let album = self.album;
        let has_thumbnail = album.album_art_thumbnail.is_some();

        // Metadata for grid view (smaller font)
        let format = Self::get_format(&album);
        let sample_info = Self::format_sample_info(&album);
        let dr = Self::format_dr(album.dynamic_range);
        let track_count = album.tracks.len();

        div()
            .id(SharedString::from(format!("album-card-{}", self.index)))
            .w(px(card_width))
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
                    // Album art thumbnail or placeholder (no border/margin)
                    .child(
                        div()
                            .w(px(thumbnail_size))
                            .h(px(thumbnail_size))
                            .rounded_md()
                            .overflow_hidden()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(has_thumbnail, |d| {
                                if let Some(ref thumbnail_bytes) = album.album_art_thumbnail {
                                    d.child(
                                        img(image_from_jpeg_bytes(thumbnail_bytes))
                                            .w(px(thumbnail_size))
                                            .h(px(thumbnail_size))
                                            .object_fit(ObjectFit::Cover),
                                    )
                                } else {
                                    d.child(
                                        div().text_3xl().text_color(theme.text_muted).child("♪"),
                                    )
                                }
                            })
                            .when(!has_thumbnail, |d| {
                                d.child(div().text_3xl().text_color(theme.text_muted).child("♪"))
                            }),
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
                            .text_size(px(10.0))
                            .text_color(theme.text_secondary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(album.artist()),
                    )
                    // Metadata line: FORMAT [DR icon]DR #count
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_size(px(9.0))
                            .text_color(theme.text_muted)
                            // Format (e.g., FLAC)
                            .when_some(format, |d, fmt| d.child(div().child(fmt)))
                            // Sample rate info (e.g., 24/44.1k)
                            .when_some(sample_info, |d, info| d.child(div().child(info)))
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
                            .child(format!("#{}", track_count)),
                    ),
            )
            .into_any_element()
    }

    fn render_list(self) -> AnyElement {
        let theme = self.theme;
        let album = self.album;

        div()
            .id(SharedString::from(format!("album-row-{}", self.index)))
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
                    // Metadata line: FORMAT [DR icon]DR #count
                    .child(Self::build_metadata_line(&album, &theme)),
            )
            .into_any_element()
    }

    fn render_compact(self) -> AnyElement {
        let theme = self.theme;
        let album = self.album;

        div()
            .id(SharedString::from(format!("album-compact-{}", self.index)))
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
                            .text_xs()
                            .text_color(theme.text_secondary)
                            .child(format!("#{}", album.tracks.len())),
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
