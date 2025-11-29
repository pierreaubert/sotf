//! Album Card Component
//!
//! A reusable RenderOnce component for displaying album information.
//! Used in both grid and list views.

use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::Album;
use std::sync::Arc;

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
    fn render_grid(self) -> AnyElement {
        let thumbnail_size = 120.0;
        let card_width = 150.0;
        let theme = self.theme;
        let album = self.album;
        let has_thumbnail = album.album_art_thumbnail.is_some();

        div()
            .id(SharedString::from(format!("album-card-{}", self.index)))
            .w(px(card_width))
            .flex()
            .flex_col()
            .items_center()
            .p_2()
            .rounded_lg()
            .when(self.is_selected, |d| d.bg(theme.accent))
            .when(!self.is_selected, |d| d.bg(theme.surface))
            .hover(|style| style.bg(theme.surface_hover))
            .cursor_pointer()
            // Album art thumbnail or placeholder
            .child(
                div()
                    .w(px(thumbnail_size))
                    .h(px(thumbnail_size))
                    .rounded_md()
                    .overflow_hidden()
                    .bg(theme.background_secondary)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(has_thumbnail, |d| {
                        if let Some(ref path) = album.album_art_path {
                            d.child(
                                img(path.clone())
                                    .w(px(thumbnail_size))
                                    .h(px(thumbnail_size))
                                    .object_fit(ObjectFit::Cover),
                            )
                        } else {
                            d.child(div().text_3xl().text_color(theme.text_muted).child("♪"))
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
                    .mt_2()
                    .text_sm()
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
                    .text_xs()
                    .text_color(theme.text_secondary)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(album.artist()),
            )
            // Track count
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(format!("{} tracks", album.tracks.len())),
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
            .when(self.is_selected, |d| d.bg(theme.accent))
            .when(!self.is_selected, |d| d.bg(theme.surface))
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
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(format!("{} tracks", album.tracks.len())),
                    ),
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
            .when(self.is_selected, |d| d.bg(theme.accent))
            .when(!self.is_selected, |d| d.bg(theme.background_secondary))
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
        AlbumCardMode::Grid => 200.0,   // thumbnail + text
        AlbumCardMode::List => 80.0,    // full row
        AlbumCardMode::Compact => 56.0, // compact row
    }
}
