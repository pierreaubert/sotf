use crate::i18n::{HeadphoneEasyTranslations, HeadphoneEqTranslations};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};
use sotf_audio_player::autoeq::DetailLevel;

impl PlayerView {
    // ========================================================================
    // Step 4: Export
    // ========================================================================

    pub(crate) fn render_headphone_eq_export(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = HeadphoneEqTranslations::for_language(state.app.ui_state.language);
        let discovery_text =
            crate::app::i18n::EqDiscoveryTranslations::for_language(state.app.ui_state.language);
        let theme_id = state.app.ui_state.theme_id;
        let button_theme = ButtonTheme::from(&theme.to_ui_kit_theme(theme_id, cx));
        let headphone_eq = &state.app.measurement_state.headphone_eq_state;
        let has_result = headphone_eq.result.is_some();
        let easy_mode = headphone_eq.detail_level == DetailLevel::Simple;
        let export_format = headphone_eq.export_format.clone();

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(translations.apply_export)
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(translations.apply_export_description)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .when(has_result, |vstack| {
                let theme = theme.clone();
                let button_theme = button_theme.clone();

                vstack
                    .child(if easy_mode {
                        self.render_headphone_easy_apply_card(cx, &theme, &button_theme)
                    } else {
                        self.render_apply_to_playback_card(
                            cx,
                            "headphone",
                            &theme,
                            &button_theme,
                            Self::apply_headphone_eq_result,
                            Self::clear_eq_from_playback,
                        )
                    })
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new(translations.export)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        Text::new(translations.export_description)
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .child({
                                        let button_theme = button_theme.clone();
                                        HStack::new().spacing(StackSpacing::Xs).wrap(true).children(
                                            sotf_audio_player::autoeq::EQ_EXPORT_FORMAT_OPTIONS
                                                .iter()
                                                .map(|(value, label, _ext)| {
                                                    let is_selected = export_format == *value;
                                                    let value = value.to_string();

                                                    Button::new(
                                                        SharedString::from(format!(
                                                            "headphone-export-format-{}",
                                                            value
                                                        )),
                                                        *label,
                                                    )
                                                    .variant(if is_selected {
                                                        ButtonVariant::Primary
                                                    } else {
                                                        ButtonVariant::Secondary
                                                    })
                                                    .size(ButtonSize::Xs)
                                                    .theme(button_theme.clone())
                                                    .on_click_event(cx.listener(
                                                        move |view, _, _, cx| {
                                                            view.state.update(cx, |state, _cx| {
                                                                state
                                                                    .app
                                                                    .measurement_state
                                                                    .headphone_eq_state
                                                                    .export_format = value.clone();
                                                            });
                                                            cx.notify();
                                                        },
                                                    ))
                                                }),
                                        )
                                    })
                                    .child(
                                        Button::new(
                                            "save-headphone-eq",
                                            discovery_text.save_eq_file,
                                        )
                                        .variant(ButtonVariant::Primary)
                                        .size(ButtonSize::Sm)
                                        .theme(button_theme.clone())
                                        .on_click_event(
                                            cx.listener(|view, _, _, cx| {
                                                view.save_headphone_eq_result(cx);
                                            }),
                                        ),
                                    ),
                            ),
                    )
            })
            .when(!has_result, |vstack| {
                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new(translations.no_results)
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            Text::new(translations.no_results_description)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
            })
    }

    fn render_headphone_easy_apply_card(
        &self,
        cx: &mut Context<Self>,
        theme: &crate::theme::Theme,
        button_theme: &ButtonTheme,
    ) -> Card {
        let headphone_eq = &self.state.read(cx).app.measurement_state.headphone_eq_state;
        let translations =
            HeadphoneEasyTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let can_undo = headphone_eq.easy_mode_undo_graph.is_some();
        let summary = headphone_eq.easy_mode_last_apply;

        Card::new()
            .background(theme.surface)
            .header_background(theme.background_secondary)
            .border(theme.border)
            .header(
                Text::new(translations.title)
                    .color(theme.text_primary)
                    .weight(TextWeight::Semibold),
            )
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new(translations.description)
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    .child(
                        Text::new(translations.safety)
                            .size(TextSize::Xs)
                            .color(theme.warning),
                    )
                    .when_some(summary, |stack, summary| {
                        stack.child(
                            Text::new(
                                translations.summary(summary.active_filters, summary.preamp_db),
                            )
                            .size(TextSize::Xs)
                            .color(theme.success),
                        )
                    })
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Xs)
                            .wrap(true)
                            .child(
                                Button::new("apply-headphone-easy-chain", translations.apply)
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Sm)
                                    .theme(button_theme.clone())
                                    .on_click_event(cx.listener(|view, _, _, cx| {
                                        view.apply_headphone_easy_result(cx);
                                    })),
                            )
                            .child(
                                Button::new("undo-headphone-easy-chain", translations.undo)
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .disabled(!can_undo)
                                    .theme(button_theme.clone())
                                    .on_click_event(cx.listener(|view, _, _, cx| {
                                        view.undo_headphone_easy_chain(cx);
                                    })),
                            )
                            .child(
                                Button::new(
                                    "edit-headphone-easy-chain",
                                    translations.edit_in_studio,
                                )
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .disabled(summary.is_none())
                                .theme(button_theme.clone())
                                .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.edit_headphone_easy_chain(cx);
                                })),
                            ),
                    ),
            )
    }
}
