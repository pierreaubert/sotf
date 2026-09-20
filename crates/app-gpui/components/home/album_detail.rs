use crate::app::{AppState, Screen, ToastMessage};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Text};
use sotf_audio_player::{Album, QueuePlaybackEffect};
use std::sync::Arc;

#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
macro_rules! tracked {
    ($element:expr, $id:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            $element.dev_track($id)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

#[derive(Clone, Copy)]
pub(crate) enum AlbumAction {
    Play,
    Next,
    Append,
}

impl PlayerView {
    pub(crate) fn open_album_detail(
        &mut self,
        album: Arc<Album>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _| {
            let full = state
                .app
                .library_state
                .library
                .albums
                .iter()
                .find(|candidate| {
                    // Imported/demo albums may not have a database ID yet.
                    // Track identity still resolves a split queue item to its
                    // complete library album without consulting active filters.
                    (album.id.is_none() || candidate.id == album.id)
                        && candidate
                            .tracks
                            .iter()
                            .any(|track| album.tracks.iter().any(|shown| shown.path == track.path))
                })
                .cloned()
                .map(Arc::new)
                .unwrap_or(album);
            state.app.library_state.album_detail = Some(full);
            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
            state.app.set_screen(Screen::Library, "AlbumDetail");
        });
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn close_album_detail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            state.app.library_state.album_detail = None;
        });
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn apply_album_action(
        state: &mut AppState,
        album: &Album,
        track: Option<usize>,
        action: AlbumAction,
    ) {
        let mut selection = album.clone();
        if let Some(index) = track {
            let Some(track) = album.tracks.get(index) else {
                return;
            };
            selection.tracks = vec![track.clone()];
        }
        if matches!(action, AlbumAction::Next)
            && state.app.playback.is_playing
            && let Err(error) = state.player.cancel_next()
        {
            state.app.ui_state.toast_message = Some(ToastMessage::error(error.to_string()));
            return;
        }
        let result = match action {
            AlbumAction::Play => state.app.queue_state.play_album_now(selection),
            AlbumAction::Next => state
                .app
                .queue_state
                .enqueue_next(selection)
                .map(|_| QueuePlaybackEffect::None),
            AlbumAction::Append => state
                .app
                .queue_state
                .add_album(selection)
                .map(|_| QueuePlaybackEffect::None),
        };
        match result {
            Ok(QueuePlaybackEffect::Play(source)) => {
                state.app.playback.current_queue_index = state.app.queue_state.current_index();
                Self::play_track(state, source);
            }
            Ok(_) => {
                let text = crate::app::i18n::AlbumDetailTranslations::for_language(
                    state.app.ui_state.language,
                );
                state.app.ui_state.toast_message = Some(ToastMessage::success(text[8]));
            }
            Err(error) => state.app.ui_state.toast_message = Some(ToastMessage::error(error)),
        }
    }

    pub(crate) fn render_album_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let d = Ds::from_cx(cx);
        let theme = state.app.ui_state.theme.clone();
        let text =
            crate::app::i18n::AlbumDetailTranslations::for_language(state.app.ui_state.language);
        let Some(snapshot) = state.app.library_state.album_detail.as_ref() else {
            return div().into_any_element();
        };
        // A queued album may be split by Play next. Resolve the complete library
        // album using both its ID and track identity, never the current filter.
        let album = state
            .app
            .library_state
            .library
            .albums
            .iter()
            .find(|candidate| {
                candidate.id.is_some()
                    && candidate.id == snapshot.id
                    && candidate
                        .tracks
                        .iter()
                        .any(|track| snapshot.tracks.iter().any(|shown| shown.path == track.path))
            })
            .unwrap_or(snapshot);
        let mut display_album = album.clone();
        display_album.tracks.sort_by_key(|track| {
            (
                track.disc_number.unwrap_or(1),
                track.track_number.unwrap_or(u32::MAX),
            )
        });
        let album = Arc::new(display_album);
        let mut identity = div().flex().flex_wrap().gap(d.section);
        if let Some(path) = album.album_art_path.clone() {
            identity = identity.child(
                img(path)
                    .w(rems(12.0))
                    .h(rems(12.0))
                    .object_fit(ObjectFit::Cover),
            );
        }
        let duration = album
            .tracks
            .iter()
            .filter_map(|track| track.duration_secs)
            .sum::<u64>();
        identity = identity.child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .gap(d.gap)
                .child(Text::section_header(album.title.clone()))
                .child(Text::body(album.artist()))
                .child(Text::caption(format!(
                    "{} · {} {} · {}:{:02}",
                    album
                        .year
                        .map(|year| year.to_string())
                        .unwrap_or_else(|| "—".into()),
                    album.tracks.len(),
                    text[6],
                    duration / 60,
                    duration % 60
                ))),
        );
        let mut actions = div().flex().flex_wrap().gap(d.gap);
        for (suffix, label, action) in [
            ("play", text[1], AlbumAction::Play),
            ("next", text[2], AlbumAction::Next),
            ("add", text[3], AlbumAction::Append),
        ] {
            let album = Arc::clone(&album);
            actions = actions.child(tracked!(
                Button::new(format!("album-detail-{suffix}"), label)
                    .size(ButtonSize::Sm)
                    .variant(if matches!(action, AlbumAction::Play) {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .disabled(album.tracks.is_empty())
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(move |view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            Self::apply_album_action(state, &album, None, action)
                        });
                        cx.notify();
                    })),
                format!("library.detail.{suffix}")
            ));
        }
        if let Some(id) = album.id {
            actions = actions.child(tracked!(
                Button::new(
                    "album-detail-favorite",
                    if album.is_favorite { text[5] } else { text[4] }
                )
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Ghost)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(move |view, _, _, cx| {
                    view.state
                        .update(cx, |state, _| state.app.toggle_album_favorite(id));
                    cx.notify();
                })),
                "library.detail.favorite"
            ));
        }
        let mut tracks = div().flex().flex_col().gap(d.gap);
        for (index, track) in album.tracks.iter().enumerate() {
            let title = track.title.clone().unwrap_or_else(|| {
                track
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            let duration = track
                .duration_secs
                .map(|seconds| format!("{}:{:02}", seconds / 60, seconds % 60))
                .unwrap_or_else(|| "—".into());
            let play_album = Arc::clone(&album);
            let add_album = Arc::clone(&album);
            let row = div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(d.gap)
                .py(d.pad_y)
                .border_b_1()
                .border_color(theme.border)
                .child(Text::caption(format!(
                    "{}",
                    track.track_number.unwrap_or(index as u32 + 1)
                )))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .items_start()
                        .child(tracked!(
                            Button::new(format!("album-track-{index}-play"), title)
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Ghost)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(move |view, _, _, cx| {
                                    view.state.update(cx, |state, _| {
                                        Self::apply_album_action(
                                            state,
                                            &play_album,
                                            Some(index),
                                            AlbumAction::Play,
                                        )
                                    });
                                    cx.notify();
                                })),
                            format!("library.detail.track.{index}.play")
                        ))
                        .child(Text::caption(track.artist.clone().unwrap_or_default())),
                )
                .child(Text::caption(duration))
                .child(tracked!(
                    Button::new(format!("album-track-{index}-add"), text[3])
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(move |view, _, _, cx| {
                            view.state.update(cx, |state, _| {
                                Self::apply_album_action(
                                    state,
                                    &add_album,
                                    Some(index),
                                    AlbumAction::Append,
                                )
                            });
                            cx.notify();
                        })),
                    format!("library.detail.track.{index}.add")
                ));
            tracks = tracks.child(row);
        }
        div()
            .id("album-detail")
            .size_full()
            .overflow_y_scroll()
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section)
            .child(div().flex().child(tracked!(
                Button::new("album-detail-back", text[0])
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .theme(theme.to_button_theme())
                    .on_click_event(
                        cx.listener(|view, _, window, cx| view.close_album_detail(window, cx))
                    ),
                "library.detail.back"
            )))
            .child(identity)
            .child(actions)
            .child(Text::section_header(text[6]))
            .child(tracks)
            .when(album.tracks.is_empty(), |panel| {
                panel.child(Text::body(text[7]))
            })
            .into_any_element()
    }
}
