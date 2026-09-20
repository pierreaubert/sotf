//! Saved HTTP/SOTF stream screen.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, Heading, Input, InputSize, StackAlign, StackSpacing,
    Text, TextSize, TextWeight, Toggle, ToggleSize, ToggleStyle, VStack,
};

use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::i18n::StreamsTranslations;
use crate::ui::PlayerView;

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            use crate::app::dev_api::DevTrackExt;
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

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

        let translations =
            StreamsTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let editor_open = self.state.read(cx).app.stream_state.editor_open;
        let language = self.state.read(cx).app.ui_state.language;

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
                            .child(Heading::h3(translations.title))
                            .child(
                                Text::new(translations.subtitle)
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary),
                            ),
                    ),
            )
            .when(!editor_open, |el| {
                el.child(dev_track!(
                    Button::new("stream-add", StreamsTranslations::add_station(language))
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Sm)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(|view, _, window, cx| {
                            view.state
                                .update(cx, |state, _| state.app.stream_state.begin_add());
                            view.focus_handle.focus(window, cx);
                            cx.notify();
                        })),
                    "streams.add"
                ))
            })
            .when(editor_open, |el| {
                el.child(self.render_stream_editor(name, url, format_hint, seekable, cx))
            })
            .when_some(last_error, |el, err| {
                el.child(
                    div()
                        .p(d.pad_y)
                        .rounded(d.r_md)
                        .bg(theme.feedback.toast_error_bg)
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
                        .bg(theme.feedback.toast_success_bg)
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
                        .child(translations.no_saved_streams),
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
        let translations =
            StreamsTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let state_for_name = self.state.clone();
        let state_for_url = self.state.clone();
        let state_for_hint = self.state.clone();
        let state_for_seekable = self.state.clone();
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
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.gap_md)
                    .items_center()
                    .child(div().flex_1().min_w(rems(12.0)).child(dev_track!(
                            Input::new("stream-name-input")
                                .value(name)
                                .placeholder(translations.name)
                                .size(InputSize::Sm)
                                .on_text_change(move |value, _window, cx| {
                                    state_for_name.update(cx, |state, _cx| {
                                        state.app.stream_state.name_input = value.to_string();
                                    });
                                }),
                            "streams.name_input"
                        )))
                    .child(div().w(rems(8.0)).child(dev_track!(
                            Input::new("stream-format-input")
                                .value(format_hint)
                                .placeholder("mp3")
                                .size(InputSize::Sm)
                                .on_text_change(move |value, _window, cx| {
                                    state_for_hint.update(cx, |state, _cx| {
                                        state.app.stream_state.format_hint_input =
                                            value.to_string();
                                    });
                                }),
                            "streams.format_input"
                        )))
                    .child(dev_track!(
                        Toggle::new("stream-seekable-toggle")
                            .size(ToggleSize::Sm)
                            .checked(seekable)
                            .label(translations.seekable)
                            .style(ToggleStyle::Segmented)
                            .theme(theme.to_toggle_theme())
                            .on_change(move |enabled, _window, cx| {
                                state_for_seekable.update(cx, |state, _cx| {
                                    state.app.stream_state.seekable_input = enabled;
                                });
                            }),
                        "streams.seekable"
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.gap_md)
                    .items_center()
                    .child(div().flex_1().min_w(rems(12.0)).child(dev_track!(
                            Input::new("stream-url-input")
                                .value(url)
                                .placeholder(translations.url_placeholder)
                                .size(InputSize::Sm)
                                .on_text_change(move |value, _window, cx| {
                                    state_for_url.update(cx, |state, _cx| {
                                        state.app.stream_state.url_input = value.to_string();
                                    });
                                }),
                            "streams.url_input"
                        )))
                    .child(dev_track!(
                        Button::new("stream-save", translations.save)
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    if let Err(err) = state.app.save_stream_from_inputs() {
                                        state.app.record_stream_error(err);
                                    }
                                });
                                if !view.state.read(cx).app.stream_state.editor_open {
                                    view.focus_handle.focus(window, cx);
                                }
                                cx.notify();
                            })),
                        "streams.save"
                    ))
                    .child(dev_track!(
                        Button::new("stream-play-input", translations.play)
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
                        "streams.play_input"
                    )),
            )
            .child(dev_track!(
                Button::new(
                    "stream-cancel",
                    StreamsTranslations::cancel_station(self.state.read(cx).app.ui_state.language),
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(|view, _, window, cx| {
                    view.state
                        .update(cx, |state, _| state.app.stream_state.clear_editor());
                    view.focus_handle.focus(window, cx);
                    cx.notify();
                })),
                "streams.cancel"
            ))
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
        let translations =
            StreamsTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let state_for_select = self.state.clone();
        let state_for_edit = self.state.clone();
        let state_for_play = self.state.clone();
        let state_for_queue = self.state.clone();
        let state_for_remove = self.state.clone();
        let stream_for_play = stream.clone();
        let stream_for_queue = stream.clone();

        dev_track!(
            div()
                .id(SharedString::from(format!("stream-row-{index}")))
                .flex()
                .flex_wrap()
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
                                .child(Text::caption(if stream.seekable {
                                    translations.seekable
                                } else {
                                    translations.live
                                })),
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
                .child(dev_track!(
                    Button::new(
                        SharedString::from(format!("stream-edit-{index}")),
                        StreamsTranslations::edit_station(
                            self.state.read(cx).app.ui_state.language
                        ),
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click(move |_, cx| {
                        state_for_edit.update(cx, |state, _| {
                            state.app.set_stream_inputs_from_selected(index);
                        });
                    }),
                    format!("streams.edit.{index}")
                ))
                .child(dev_track!(
                    Button::new(
                        SharedString::from(format!("stream-play-{index}")),
                        translations.play,
                    )
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
                    format!("streams.play.{index}")
                ))
                .child(dev_track!(
                    Button::new(
                        SharedString::from(format!("stream-queue-{index}")),
                        translations.queue,
                    )
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
                    format!("streams.queue.{index}")
                ))
                .child(dev_track!(
                    Button::new(
                        SharedString::from(format!("stream-remove-{index}")),
                        translations.remove,
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
                    format!("streams.remove.{index}")
                ))
                .into_any_element(),
            format!("streams.row.{index}")
        )
        .into_any_element()
    }
}
