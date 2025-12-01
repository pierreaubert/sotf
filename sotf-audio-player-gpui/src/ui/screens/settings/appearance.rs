//! Appearance settings content

use crate::i18n::Language;
use crate::theme::ThemeId;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};

impl PlayerView {
    pub(crate) fn render_appearance_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme_id = state.app.theme_id;
        let language = state.app.language;
        let theme = state.app.theme.clone();

        div()
            .flex()
            .flex_col()
            .gap_6()
            // Theme selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Theme"),
                    )
                    .child({
                        let button_theme = theme.to_button_theme();
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(ThemeId::all().iter().map(|id| {
                                let is_selected = theme_id == *id;
                                let id = *id;
                                let btn_theme = button_theme.clone();
                                Button::new(
                                    SharedString::from(format!("theme-{}", id.name())),
                                    id.name(),
                                )
                                .variant(if is_selected {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .size(ButtonSize::Sm)
                                .selected(is_selected)
                                .theme(btn_theme)
                                .build()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            state.app.set_theme(id);
                                        });
                                        cx.notify();
                                    }),
                                )
                            }))
                    }),
            )
            // Language selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Language"),
                    )
                    .child({
                        let button_theme = theme.to_button_theme();
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(Language::all().iter().map(|lang| {
                                let is_selected = language == *lang;
                                let lang = *lang;
                                let btn_theme = button_theme.clone();
                                Button::new(
                                    SharedString::from(format!("language-{}", lang.name())),
                                    lang.name(),
                                )
                                .variant(if is_selected {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .size(ButtonSize::Sm)
                                .selected(is_selected)
                                .theme(btn_theme)
                                .build()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            state.app.set_language(lang);
                                        });
                                        cx.notify();
                                    }),
                                )
                            }))
                    }),
            )
    }
}
