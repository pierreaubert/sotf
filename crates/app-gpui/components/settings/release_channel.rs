use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
use sotf_audio_player::ReleaseChannel;

impl PlayerView {
    pub(crate) fn render_release_channel_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let current_channel = state.app.ui_state.release_channel;
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();

        div().flex().flex_col().gap_6().child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(translations.settings_release_channel_title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_secondary)
                        .child(translations.settings_release_channel_description),
                )
                .child({
                    let mut container = div().flex().flex_wrap().gap_4();

                    for channel in ReleaseChannel::all() {
                        let is_selected = current_channel == *channel;
                        let channel_val = *channel;
                        let accent = theme.accent;
                        let border = theme.border;
                        let surface = theme.surface;
                        let surface_selected = theme.surface_selected;
                        let text_primary = theme.text_primary;
                        let text_secondary = theme.text_secondary;

                        container = container.child(
                            div()
                                .id(SharedString::from(format!(
                                    "release-channel-{}",
                                    channel.name()
                                )))
                                .flex()
                                .flex_col()
                                .w(px(220.0))
                                .p_4()
                                .rounded_md()
                                .border_2()
                                .border_color(if is_selected { accent } else { border })
                                .bg(if is_selected {
                                    surface_selected
                                } else {
                                    surface
                                })
                                .cursor_pointer()
                                .hover(move |s| {
                                    s.border_color(accent)
                                })
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(if is_selected {
                                            accent
                                        } else {
                                            text_primary
                                        })
                                        .child(channel.name()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_secondary)
                                        .mt_1()
                                        .child(channel.description()),
                                )
                                .child(
                                    div().mt_3().child(
                                        Button::new(
                                            SharedString::from(format!(
                                                "select-channel-{}",
                                                channel.name()
                                            )),
                                            if is_selected { "Active" } else { "Select" },
                                        )
                                        .variant(if is_selected {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        })
                                        .size(ButtonSize::Sm)
                                        .full_width(true)
                                        .theme(theme.to_button_theme())
                                        .build()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(
                                                move |view, _: &MouseUpEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .set_release_channel(channel_val);
                                                    });
                                                    cx.notify();
                                                },
                                            ),
                                        ),
                                    ),
                                ),
                        );
                    }

                    container
                }),
        )
    }
}
