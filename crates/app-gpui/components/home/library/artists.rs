use crate::app::i18n::DesktopTranslations;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Text};
use std::sync::Arc;

#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;

impl PlayerView {
    pub(super) fn render_artist_album_rows(&self, cx: &mut Context<Self>) -> AnyElement {
        self.render_album_rows(true, cx)
    }

    pub(super) fn render_library_albums(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.state.read(cx).app.library_state.album_list_view
            && !self.state.read(cx).app.filtered_albums().is_empty()
        {
            self.render_album_rows(false, cx)
        } else {
            self.render_library_grid(cx).into_any_element()
        }
    }

    fn render_album_rows(&self, artist_first: bool, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let count = state.app.filtered_albums().len();
        if count == 0 {
            return div()
                .p(Ds::from_cx(cx).card)
                .child(Text::body(
                    crate::app::i18n::LibraryToolbarTranslations::for_language(
                        state.app.ui_state.language,
                    )[3],
                ))
                .into_any_element();
        }
        let entity = cx.entity().downgrade();
        uniform_list("library-artist-albums", count, move |range, _, cx| {
            entity
                .update(cx, |view, cx| {
                    let state = view.state.read(cx);
                    let albums = state.app.filtered_albums();
                    let theme = state.app.ui_state.theme.clone();
                    let text = DesktopTranslations::for_language(state.app.ui_state.language);
                    let d = Ds::from_cx(cx);
                    range
                        .filter_map(|index| {
                            let album = *albums.get(index)?;
                            let title = album.title.clone();
                            let artist = album.artist();
                            let (heading, detail) = if artist_first {
                                (artist, title)
                            } else {
                                let detail = album
                                    .year
                                    .map(|year| format!("{artist} · {year}"))
                                    .unwrap_or(artist);
                                (title, detail)
                            };
                            let identity = album.tracks.first().map(|track| track.path.clone());
                            let button =
                                Button::new(
                                    format!("library-artist-album-{index}"),
                                    text.view_album,
                                )
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .disabled(identity.is_none())
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(move |view, _, window, cx| {
                                    let album = identity.as_ref().and_then(|path| {
                                        view.state
                                            .read(cx)
                                            .app
                                            .library_state
                                            .library
                                            .albums
                                            .iter()
                                            .find(|album| {
                                                album.tracks.iter().any(|track| &track.path == path)
                                            })
                                            .cloned()
                                            .map(Arc::new)
                                    });
                                    if let Some(album) = album {
                                        view.open_album_detail(album, window, cx);
                                    }
                                }));
                            #[cfg(feature = "dev-api")]
                            let button = button.dev_track(if artist_first {
                                format!("library.artist.album.{index}")
                            } else {
                                format!("library.list.album.{index}")
                            });
                            Some(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap(d.gap)
                                    .p(d.pad_y)
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .flex()
                                            .flex_col()
                                            .child(Text::section_header(heading))
                                            .child(Text::caption(detail)),
                                    )
                                    .child(button),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .size_full()
        .into_any_element()
    }
}
