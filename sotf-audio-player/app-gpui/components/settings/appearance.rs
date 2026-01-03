//! Appearance settings content (Theme and Language)

use crate::i18n::Language;
use crate::theme::ThemeId;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Toggle, ToggleStyle};

impl PlayerView {
    /// Render theme settings content
    pub(crate) fn render_theme_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme_id = state.app.theme_id;
        let theme = state.app.theme.clone();
        let translations = state.app.translations.clone();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(translations.settings_theme),
                    )
                    .child({
                        let mut container = div().flex().flex_wrap().gap_4();

                        for id in ThemeId::all().iter() {
                            let is_selected = theme_id == *id;
                            let preview_theme = crate::theme::Theme::from_id(*id);

                            container = container.child(self.render_theme_preview_card(
                                *id,
                                preview_theme,
                                is_selected,
                                theme.clone(),
                                translations.settings_active,
                                cx,
                            ));
                        }

                        container
                    }),
            )
    }

    /// Render language settings content
    pub(crate) fn render_language_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let language = state.app.language;
        let theme = state.app.theme.clone();
        let translations = state.app.translations.clone();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(translations.settings_language),
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

    /// Render a visual preview card for a theme showing its color scheme
    fn render_theme_preview_card(
        &self,
        theme_id: ThemeId,
        preview_theme: crate::theme::Theme,
        is_selected: bool,
        current_theme: crate::theme::Theme,
        active_label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(px(200.0))
            .rounded_md()
            .overflow_hidden()
            .cursor_pointer()
            .border_2()
            .border_color(if is_selected {
                current_theme.accent
            } else {
                current_theme.border
            })
            .bg(preview_theme.surface)
            .shadow_md()
            .hover(|style| {
                style.border_color(if is_selected {
                    current_theme.accent_hover
                } else {
                    current_theme.border_focused
                })
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.set_theme(theme_id);
                    });
                    cx.notify();
                }),
            )
            .child(
                // Theme name header
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(40.0))
                    .bg(preview_theme.background)
                    .border_b_1()
                    .border_color(preview_theme.border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(preview_theme.text_primary)
                            .child(theme_id.name()),
                    ),
            )
            .child(
                // Color swatches grid
                div()
                    .flex()
                    .flex_col()
                    .p_3()
                    .gap_2()
                    .child(
                        // Background colors row
                        div()
                            .flex()
                            .gap_1()
                            .child(self.render_color_swatch(
                                "BG",
                                preview_theme.background,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                "Surf",
                                preview_theme.surface,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                "Hover",
                                preview_theme.surface_hover,
                                preview_theme.text_primary,
                            )),
                    )
                    .child(
                        // Accent and text colors row
                        div()
                            .flex()
                            .gap_1()
                            .child(self.render_color_swatch(
                                "Accent",
                                preview_theme.accent,
                                preview_theme.text_on_accent,
                            ))
                            .child(self.render_color_swatch(
                                "Text",
                                preview_theme.background,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                "Muted",
                                preview_theme.background,
                                preview_theme.text_muted,
                            )),
                    )
                    .child(
                        // Semantic colors row
                        div()
                            .flex()
                            .gap_1()
                            .child(self.render_color_swatch(
                                "✓",
                                preview_theme.success,
                                preview_theme.text_on_accent,
                            ))
                            .child(self.render_color_swatch(
                                "⚠",
                                preview_theme.warning,
                                preview_theme.text_on_accent,
                            ))
                            .child(self.render_color_swatch(
                                "✗",
                                preview_theme.error,
                                preview_theme.text_on_accent,
                            )),
                    )
                    .child(
                        // Button variants preview
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .pt_2()
                            .border_t_1()
                            .border_color(preview_theme.border)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(preview_theme.text_muted)
                                    .child("Buttons"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_1()
                                    .child(
                                        Button::new("preview-primary", "Pri")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-secondary", "Sec")
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-destructive", "Del")
                                            .variant(ButtonVariant::Destructive)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-ghost", "Gho")
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-outline", "Out")
                                            .variant(ButtonVariant::Outline)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(
                                        Toggle::new("preview-toggle-off")
                                            .checked(false)
                                            .label("Off".to_string())
                                            .style(ToggleStyle::Segmented)
                                            .theme(preview_theme.to_toggle_theme()),
                                    )
                                    .child(
                                        Toggle::new("preview-toggle-on")
                                            .checked(true)
                                            .label("On".to_string())
                                            .style(ToggleStyle::Segmented)
                                            .theme(preview_theme.to_toggle_theme()),
                                    ),
                            ),
                    ),
            )
            .when(is_selected, |this| {
                this.child(
                    // Selected indicator
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(30.0))
                        .bg(current_theme.accent)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(current_theme.text_on_accent)
                                .child(format!("✓ {}", active_label)),
                        ),
                )
            })
    }

    /// Render a small color swatch with label
    fn render_color_swatch(
        &self,
        label: &'static str,
        bg_color: gpui::Rgba,
        text_color: gpui::Rgba,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .h(px(32.0))
            .rounded_sm()
            .bg(bg_color)
            .border_1()
            .border_color(gpui::Rgba {
                r: text_color.r,
                g: text_color.g,
                b: text_color.b,
                a: 0.2,
            })
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(label),
            )
    }
}
