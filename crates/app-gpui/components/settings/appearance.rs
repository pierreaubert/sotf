//! Appearance settings content (Theme and Language)

use crate::components::design::Ds;
use crate::i18n::Language;
use crate::theme::ThemeId;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSet, ButtonSetOption, ButtonSize, ButtonVariant, Toggle, ToggleStyle,
};

impl PlayerView {
    /// Render theme settings content
    pub(crate) fn render_theme_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme_id = state.app.ui_state.theme_id;
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();

        div().flex().flex_col().gap(d.section_lg).child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap_md)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(translations.settings_theme),
                )
                .child({
                    let mut container = div().flex().flex_wrap().gap(d.section);

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
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let language = state.app.ui_state.language;
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();

        div().flex().flex_col().gap(d.section_lg).child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(translations.settings_language),
                )
                .child({
                    let state_entity = self.state.clone();
                    ButtonSet::new("language-select")
                        .options(
                            Language::all()
                                .iter()
                                .map(|lang| ButtonSetOption::new(lang.name(), lang.name()))
                                .collect(),
                        )
                        .selected(language.name())
                        .theme(theme.to_button_set_theme())
                        .on_change(move |value, _window, cx| {
                            let lang = Language::all()
                                .iter()
                                .find(|l| l.name() == value.as_ref())
                                .copied();
                            if let Some(lang) = lang {
                                state_entity.update(cx, |state, _cx| {
                                    state.app.set_language(lang);
                                });
                            }
                        })
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
        let d = Ds::from_cx(cx);
        div()
            .flex()
            .flex_col()
            .w(rems(12.5))
            .rounded(d.r_md)
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
                    .h(rems(2.5))
                    .bg(preview_theme.background)
                    .border_b_1()
                    .border_color(preview_theme.border)
                    .child(
                        div()
                            .text_size(d.text_sm)
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
                    .p(d.pad_x)
                    .gap(d.gap)
                    .child(
                        // Background colors row
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(self.render_color_swatch(
                                &d,
                                "BG",
                                preview_theme.background,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Surf",
                                preview_theme.surface,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Hover",
                                preview_theme.surface_hover,
                                preview_theme.text_primary,
                            )),
                    )
                    .child(
                        // Accent and text colors row
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(self.render_color_swatch(
                                &d,
                                "Accent",
                                preview_theme.accent,
                                preview_theme.text_on_accent,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Text",
                                preview_theme.background,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Muted",
                                preview_theme.background,
                                preview_theme.text_muted,
                            )),
                    )
                    .child(
                        // Semantic colors row
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(self.render_color_swatch(
                                &d,
                                "✓",
                                preview_theme.success,
                                preview_theme.text_on_accent,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "⚠",
                                preview_theme.warning,
                                preview_theme.text_on_accent,
                            ))
                            .child(self.render_color_swatch(
                                &d,
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
                            .gap(d.grid)
                            .pt(d.pad_y)
                            .border_t_1()
                            .border_color(preview_theme.border)
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .text_color(preview_theme.text_muted)
                                    .child("Buttons"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(d.grid)
                                    .child(
                                        Button::new("preview-primary", "Pri")
                                            .aria_label("Primary variant preview")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-secondary", "Sec")
                                            .aria_label("Secondary variant preview")
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-destructive", "Del")
                                            .aria_label("Destructive variant preview")
                                            .variant(ButtonVariant::Destructive)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-ghost", "Gho")
                                            .aria_label("Ghost variant preview")
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-outline", "Out")
                                            .aria_label("Outline variant preview")
                                            .variant(ButtonVariant::Outline)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(d.grid)
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
                        .h(rems(1.875))
                        .bg(current_theme.accent)
                        .child(
                            div()
                                .text_size(d.text_xs)
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
        d: &Ds,
        label: &'static str,
        bg_color: gpui::Rgba,
        text_color: gpui::Rgba,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .h(rems(2.0))
            .rounded(d.r_sm)
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
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(label),
            )
    }
}
