use crate::components::design::Ds;
use crate::components::home::album_detail::AlbumAction;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Text};

#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;

impl PlayerView {
    pub(super) fn render_library_tracks(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let labels =
            crate::app::i18n::AlbumDetailTranslations::for_language(state.app.ui_state.language);
        let count = state.app.library_state.selection_filtered_tracks().len();
        let d = Ds::from_cx(cx);
        if count == 0 {
            return div()
                .p(d.card)
                .child(Text::body(labels[7]))
                .into_any_element();
        }
        let entity = cx.entity().downgrade();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                uniform_list("library-track-list", count, move |range, _, cx| {
                    entity
                        .update(cx, |view, cx| {
                            let state = view.state.read(cx);
                            let rows = state.app.library_state.selection_filtered_tracks();
                            let theme = state.app.ui_state.theme.clone();
                            let labels = crate::app::i18n::AlbumDetailTranslations::for_language(
                                state.app.ui_state.language,
                            );
                            let d = Ds::from_cx(cx);
                            range
                                .filter_map(|row_index| {
                                    let (album, track_index) = *rows.get(row_index)?;
                                    let track = &album.tracks[track_index];
                                    let title = track.title.clone().unwrap_or_else(|| {
                                        track
                                            .path
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .into_owned()
                                    });
                                    let artist =
                                        track.artist.clone().unwrap_or_else(|| album.artist());
                                    let duration = track
                                        .duration_secs
                                        .map(|seconds| {
                                            format!("{}:{:02}", seconds / 60, seconds % 60)
                                        })
                                        .unwrap_or_else(|| "—".into());
                                    let mut actions = Vec::new();
                                    for (suffix, label, action) in [
                                        ("play", title, AlbumAction::Play),
                                        ("add", labels[3].to_owned(), AlbumAction::Append),
                                    ] {
                                        let path = track.path.clone();
                                        let button = Button::new(
                                            format!("library-track-{row_index}-{suffix}"),
                                            label,
                                        )
                                        .size(ButtonSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .theme(theme.to_button_theme())
                                        .on_click_event(cx.listener(move |view, _, _, cx| {
                                            view.state.update(cx, |state, _| {
                                                // Resolve stable identity at activation, including after a rescan.
                                                let selection = state
                                                    .app
                                                    .library_state
                                                    .library
                                                    .albums
                                                    .iter()
                                                    .find_map(|album| {
                                                        album
                                                            .tracks
                                                            .iter()
                                                            .position(|t| t.path == path)
                                                            .map(|index| (album.clone(), index))
                                                    });
                                                if let Some((album, index)) = selection {
                                                    Self::apply_album_action(
                                                        state,
                                                        &album,
                                                        Some(index),
                                                        action,
                                                    );
                                                }
                                            });
                                            cx.notify();
                                        }));
                                        #[cfg(feature = "dev-api")]
                                        let button = button.dev_track(format!(
                                            "library.track.{row_index}.{suffix}"
                                        ));
                                        actions.push(button);
                                    }
                                    let add = actions.pop()?;
                                    let play = actions.pop()?;
                                    Some(
                                        div()
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .gap(d.gap)
                                            .px(d.pad_x)
                                            .py(d.pad_y)
                                            .border_b_1()
                                            .border_color(theme.border)
                                            .child(Text::caption((row_index + 1).to_string()))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .overflow_hidden()
                                                    .flex()
                                                    .flex_col()
                                                    .items_start()
                                                    .child(play)
                                                    .child(Text::caption(artist)),
                                            )
                                            .child(Text::caption(duration))
                                            .child(add),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .size_full(),
            )
            .into_any_element()
    }
}
