use crate::app::i18n::FlatQueueTranslations;
use crate::app::{Screen, ToastMessage};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::queue_render::{FlatQueueRow, flat_queue_rows};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::{AnyElement, App, ClickEvent, Window, div, uniform_list};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize, IconButtonVariant, Text,
};
use sotf_audio_player::QueuePlaybackEffect;

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

impl PlayerView {
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    pub(super) fn render_flat_queue(&self, cx: &mut Context<Self>) -> AnyElement {
        let management_actions = self.render_queue_management_actions(cx).into_any_element();
        let state = self.state.read(cx);
        let app = &state.app;
        let labels = FlatQueueTranslations::for_language(app.ui_state.language);
        let translations = app.ui_state.translations.clone();
        let theme = app.ui_state.theme.clone();
        let d = Ds::from_cx(cx);
        let count = app.queue_state.upcoming_track_positions().len();
        let current = app
            .queue_state
            .current_index
            .and_then(|index| app.queue_state.get(index))
            .and_then(|item| item.current_track())
            .map(|track| {
                track
                    .title
                    .clone()
                    .unwrap_or_else(|| track.path.to_string_lossy().into_owned())
            });
        let mut root = div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(d.gap)
            .p(d.card)
            .child(Text::section_header(translations.queue_title))
            .child(management_actions);
        if let Some(title) = current {
            root = root.child(Text::label(format!(
                "{} · {title}",
                if app.playback.is_playing {
                    labels[2]
                } else {
                    labels[3]
                }
            )));
        }
        root = root.child(Text::caption(labels[1])).child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(d.gap)
                .child(Text::section_header(format!("{} ({count})", labels[0])))
                .child(tracked!(
                    Button::new("flat-queue-undo", translations.queue_undo_remove)
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .theme(theme.to_button_theme())
                        .disabled(!app.queue_state.can_undo_remove())
                        .on_click_event(cx.listener(|view, _, _, cx| {
                            view.state.update(cx, |state, cx| {
                                if let Err(error) = state.player.cancel_next() {
                                    state.app.ui_state.toast_message =
                                        Some(ToastMessage::error(error.to_string()));
                                } else {
                                    match state.app.undo_remove_from_queue() {
                                        QueuePlaybackEffect::Play(source)
                                        | QueuePlaybackEffect::Reload(source) => {
                                            Self::play_track(state, source);
                                        }
                                        QueuePlaybackEffect::Stop => {
                                            if let Err(error) = state.player.stop() {
                                                state.app.ui_state.toast_message =
                                                    Some(ToastMessage::error(error.to_string()));
                                            }
                                        }
                                        QueuePlaybackEffect::None => {}
                                    }
                                }
                                cx.notify();
                            });
                        })),
                    "queue.undo_remove"
                )),
        );
        if count == 0 {
            return root
                .child(Text::body(labels[8]))
                .child(tracked!(
                    Button::new("flat-queue-browse", labels[7])
                        .size(ButtonSize::Sm)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(|view, _, window, cx| {
                            view.state.update(cx, |state, cx| {
                                state.app.library_state.album_detail = None;
                                state.app.set_screen(Screen::Library, "QueueBrowseLibrary");
                                cx.notify();
                            });
                            view.focus_handle.focus(window, cx);
                        })),
                    "queue.browse_library"
                ))
                .into_any_element();
        }
        let entity = cx.entity().downgrade();
        let row_count = flat_queue_rows(
            &app.queue_state,
            &app.queue_state.upcoming_track_positions(),
        )
        .len();
        root.child(
            uniform_list("flat-queue-tracks", row_count, move |range, _, cx| {
                entity
                    .update(cx, |view, cx| {
                        let state = view.state.read(cx);
                        let queue = &state.app.queue_state;
                        let positions = queue.upcoming_track_positions();
                        let rows = flat_queue_rows(queue, &positions);
                        let labels =
                            FlatQueueTranslations::for_language(state.app.ui_state.language);
                        let theme = state.app.ui_state.theme.clone();
                        let d = Ds::from_cx(cx);
                        range
                            .filter_map(|row_index| {
                                let row = *rows.get(row_index)?;
                                let index = match row {
                                    FlatQueueRow::Album(item_index) => {
                                        let album = &queue.get(item_index)?.album;
                                        return Some(
                                            div()
                                                .w_full()
                                                .h(d.section_xl)
                                                .flex()
                                                .items_center()
                                                .overflow_hidden()
                                                .child(Text::label(format!(
                                                    "{} · {}",
                                                    album.title,
                                                    album.artist()
                                                ))),
                                        );
                                    }
                                    FlatQueueRow::Track(index) => index,
                                };
                                let position = *positions.get(index)?;
                                let item = queue.get(position.item)?;
                                let track = item.album.tracks.get(position.track)?;
                                let title = track
                                    .title
                                    .clone()
                                    .unwrap_or_else(|| track.path.to_string_lossy().into_owned());
                                let duration = track.duration_secs.map_or_else(
                                    || "—".into(),
                                    |seconds| format!("{}:{:02}", seconds / 60, seconds % 60),
                                );
                                let view_state = view.state.clone();
                                let mut buttons = Vec::new();
                                for (action, label, disabled) in [
                                    ("play", title, false),
                                    ("up", labels[4].to_owned(), index == 0),
                                    ("down", labels[5].to_owned(), index + 1 == positions.len()),
                                    ("remove", labels[6].to_owned(), false),
                                ] {
                                    let source = track.audio_source();
                                    // Row actions render as icon buttons; the
                                    // play button keeps its text label because
                                    // the track title doubles as the row title.
                                    let icon = match action {
                                        "up" => Some(IconName::ChevronUp),
                                        "down" => Some(IconName::ChevronDown),
                                        "remove" => Some(IconName::X),
                                        _ => None,
                                    };
                                    let state_entity = view_state.clone();
                                    let on_click =
                                        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                            state_entity.update(cx, |state, cx| {
                                                let Some(live_index) = state
                                                    .app
                                                    .queue_state
                                                    .resolve_upcoming_track(position, &source)
                                                else {
                                                    return;
                                                };
                                                if let Err(error) = state.player.cancel_next() {
                                                    state.app.ui_state.toast_message = Some(
                                                        ToastMessage::error(error.to_string()),
                                                    );
                                                    cx.notify();
                                                    return;
                                                }
                                                match action {
                                                    "play" => {
                                                        if let QueuePlaybackEffect::Play(source) =
                                                            state
                                                                .app
                                                                .play_upcoming_track(live_index)
                                                        {
                                                            Self::play_track(state, source);
                                                        }
                                                    }
                                                    "up" if live_index > 0 => {
                                                        state.app.move_upcoming_track(
                                                            live_index,
                                                            live_index - 1,
                                                        );
                                                    }
                                                    "down" => {
                                                        state.app.move_upcoming_track(
                                                            live_index,
                                                            live_index + 1,
                                                        );
                                                    }
                                                    "remove" => {
                                                        state.app.remove_upcoming_track(live_index);
                                                    }
                                                    _ => {}
                                                }
                                                cx.notify();
                                            });
                                        };
                                    let button = if let Some(icon) = icon {
                                        IconButton::with_child(
                                            format!("flat-queue-{index}-{action}"),
                                            Icon::new(icon).small().color(theme.text_primary),
                                        )
                                        .size(IconButtonSize::Sm)
                                        .variant(IconButtonVariant::Ghost)
                                        .theme(theme.to_icon_button_theme())
                                        .aria_label(label)
                                        .disabled(disabled)
                                        .on_click_event(on_click)
                                        .into_any_element()
                                    } else {
                                        Button::new(format!("flat-queue-{index}-{action}"), label)
                                            .size(ButtonSize::Sm)
                                            .variant(ButtonVariant::Ghost)
                                            .theme(theme.to_button_theme())
                                            .disabled(disabled)
                                            .on_click_event(on_click)
                                            .into_any_element()
                                    };
                                    buttons.push(tracked!(
                                        button,
                                        format!("queue.upcoming.{index}.{action}")
                                    ));
                                }
                                let play = buttons.remove(0);
                                Some(
                                    div()
                                        .w_full()
                                        .flex()
                                        .h(d.section_xl)
                                        .items_center()
                                        .gap(d.gap)
                                        .border_b_1()
                                        .border_color(theme.border)
                                        .child(
                                            div().flex_1().min_w_0().overflow_hidden().child(play),
                                        )
                                        .child(Text::caption(duration))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_shrink_0()
                                                .gap(d.gap)
                                                .children(buttons),
                                        ),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .size_full(),
        )
        .into_any_element()
    }
}
