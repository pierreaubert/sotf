//! Saved HTTP/SOTF stream screen.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, Input, InputSize, StackAlign, StackSpacing, Text,
    TextSize, TextWeight, Toggle, ToggleSize, ToggleStyle, VStack,
};

use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::PlayerView;

impl PlayerView {
    pub(crate) fn render_streams_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (theme, streams, name, url, format_hint, seekable, last_error, last_status) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.stream_state.store.streams.clone(),
                state.app.stream_state.name_input.clone(),
                state.app.stream_state.url_input.clone(),
                state.app.stream_state.format_hint_input.clone(),
                state.app.stream_state.seekable_input,
                state.app.stream_state.last_error.clone(),
                state.app.stream_state.last_status.clone(),
            )
        };

        div()
            .id("streams-screen")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .gap(d.section_lg)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(
                        Icon::new(IconName::ListMusic)
                            .size(IconSize::Lg)
                            .color(theme.accent),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(
                                Text::new("Streams")
                                    .size(TextSize::Lg)
                                    .weight(TextWeight::Bold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                Text::new(
                                    "HTTPS streams, local SOTF media URLs, HLS, Spotify, and Tidal",
                                )
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                            ),
                    ),
            )
            .child(self.render_stream_editor(name, url, format_hint, seekable, cx))
            .when_some(last_error, |el, err| {
                el.child(
                    div()
                        .p(d.pad_y)
                        .rounded(d.r_md)
                        .bg(theme.toast_error_bg)
                        .text_size(d.text_sm)
                        .text_color(theme.error)
                        .child(err),
                )
            })
            .when_some(last_status, |el, status| {
                el.child(
                    div()
                        .p(d.pad_y)
                        .rounded(d.r_md)
                        .bg(theme.toast_success_bg)
                        .text_size(d.text_sm)
                        .text_color(theme.success)
                        .child(status),
                )
            })
            .when(streams.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .min_h(rems(12.0))
                        .rounded(d.r_md)
                        .border_1()
                        .border_color(theme.border)
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child("No saved streams"),
                )
            })
            .children(
                streams
                    .into_iter()
                    .enumerate()
                    .map(|(index, stream)| self.render_stream_row(index, stream, cx)),
            )
    }

    fn render_stream_editor(
        &self,
        name: String,
        url: String,
        format_hint: String,
        seekable: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let state_for_name = self.state.clone();
        let state_for_url = self.state.clone();
        let state_for_hint = self.state.clone();
        let state_for_seekable = self.state.clone();
        let state_for_save = self.state.clone();
        let state_for_play = self.state.clone();

        div()
            .id("streams-editor")
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .p(d.card)
            .bg(theme.background_secondary)
            .rounded(d.r_md)
            .border_1()
            .border_color(theme.border)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(
                        div().flex_1().child(
                            Input::new("stream-name-input")
                                .value(name)
                                .placeholder("Name")
                                .size(InputSize::Sm)
                                .on_text_change(move |value, _window, cx| {
                                    state_for_name.update(cx, |state, _cx| {
                                        state.app.stream_state.name_input = value;
                                    });
                                }),
                        ),
                    )
                    .child(
                        div().w(rems(8.0)).child(
                            Input::new("stream-format-input")
                                .value(format_hint)
                                .placeholder("mp3")
                                .size(InputSize::Sm)
                                .on_text_change(move |value, _window, cx| {
                                    state_for_hint.update(cx, |state, _cx| {
                                        state.app.stream_state.format_hint_input = value;
                                    });
                                }),
                        ),
                    )
                    .child(
                        Toggle::new("stream-seekable-toggle")
                            .size(ToggleSize::Sm)
                            .checked(seekable)
                            .label("Seekable")
                            .style(ToggleStyle::Segmented)
                            .theme(theme.to_toggle_theme())
                            .on_change(move |enabled, _window, cx| {
                                state_for_seekable.update(cx, |state, _cx| {
                                    state.app.stream_state.seekable_input = enabled;
                                });
                            }),
                    ),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(
                        div().flex_1().child(
                            Input::new("stream-url-input")
                                .value(url)
                                .placeholder("https://example.com/live.m3u8 or spotify:track:id")
                                .size(InputSize::Sm)
                                .on_text_change(move |value, _window, cx| {
                                    state_for_url.update(cx, |state, _cx| {
                                        state.app.stream_state.url_input = value;
                                    });
                                }),
                        ),
                    )
                    .child(
                        Button::new("stream-save", "Save")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click(move |_, cx| {
                                state_for_save.update(cx, |state, _cx| {
                                    if let Err(err) = state.app.save_stream_from_inputs() {
                                        state.app.record_stream_error(err);
                                    }
                                });
                            }),
                    )
                    .child(
                        Button::new("stream-play-input", "Play")
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click(move |_, cx| {
                                state_for_play.update(cx, |state, _cx| {
                                    match sotf_audio_player::SavedStream::new(
                                        state.app.stream_state.name_input.clone(),
                                        state.app.stream_state.url_input.clone(),
                                        state.app.stream_state.format_hint(),
                                        state.app.stream_state.seekable_input,
                                    ) {
                                        Ok(stream) => match state.app.play_stream_now(stream) {
                                            Ok(Some(source)) => {
                                                PlayerView::play_track(state, source)
                                            }
                                            Ok(None) => {}
                                            Err(err) => state.app.record_stream_error(err),
                                        },
                                        Err(err) => state.app.record_stream_error(err.to_string()),
                                    }
                                });
                            }),
                    ),
            )
            .into_any_element()
    }

    fn render_stream_row(
        &self,
        index: usize,
        stream: sotf_audio_player::SavedStream,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let state_for_select = self.state.clone();
        let state_for_play = self.state.clone();
        let state_for_queue = self.state.clone();
        let state_for_remove = self.state.clone();
        let stream_for_play = stream.clone();
        let stream_for_queue = stream.clone();

        div()
            .id(SharedString::from(format!("stream-row-{index}")))
            .flex()
            .items_center()
            .gap(d.gap_md)
            .p(d.card)
            .bg(theme.surface)
            .rounded(d.r_md)
            .border_1()
            .border_color(theme.border)
            .cursor_pointer()
            .hover({
                let theme = theme.clone();
                move |style| style.bg(theme.surface_hover)
            })
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_for_select.update(cx, |state, _cx| {
                    state.app.set_stream_inputs_from_selected(index);
                });
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .min_w_0()
                    .flex_1()
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .align(StackAlign::Center)
                            .child(
                                Text::new(stream.name.clone())
                                    .size(TextSize::Sm)
                                    .weight(TextWeight::Bold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                Text::new(if stream.seekable { "seekable" } else { "live" })
                                    .size(TextSize::Xs)
                                    .color(theme.text_muted),
                            ),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(stream.url.clone()),
                    ),
            )
            .child(
                Button::new(SharedString::from(format!("stream-play-{index}")), "Play")
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click(move |_, cx| {
                        state_for_play.update(cx, |state, _cx| {
                            match state.app.play_stream_now(stream_for_play.clone()) {
                                Ok(Some(source)) => PlayerView::play_track(state, source),
                                Ok(None) => {}
                                Err(err) => state.app.record_stream_error(err),
                            }
                        });
                    }),
            )
            .child(
                Button::new(SharedString::from(format!("stream-queue-{index}")), "Queue")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click(move |_, cx| {
                        state_for_queue.update(cx, |state, _cx| {
                            match state.app.add_stream_to_queue(stream_for_queue.clone()) {
                                Ok(Some(source)) => PlayerView::play_track(state, source),
                                Ok(None) => {}
                                Err(err) => state.app.record_stream_error(err),
                            }
                        });
                    }),
            )
            .child(
                Button::new(
                    SharedString::from(format!("stream-remove-{index}")),
                    "Remove",
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .theme(theme.to_button_theme())
                .on_click(move |_, cx| {
                    state_for_remove.update(cx, |state, _cx| {
                        if let Err(err) = state.app.remove_stream_at(index) {
                            state.app.record_stream_error(err);
                        }
                    });
                }),
            )
            .into_any_element()
    }
}
