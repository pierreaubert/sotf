use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

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
    // ========================================================================
    // Step 4: Export
    // ========================================================================

    pub(crate) fn render_spinorama_export(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let discovery_text =
            crate::app::i18n::EqDiscoveryTranslations::for_language(state.app.ui_state.language);
        let theme_id = state.app.ui_state.theme_id;
        let button_theme = ButtonTheme::from(&theme.to_ui_kit_theme(theme_id, cx));
        let spinorama = &state.app.measurement_state.spinorama_eq_state;
        let has_result = spinorama.result.is_some();
        let export_format = spinorama.export_format.clone();

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(translations.spinorama_apply_export)
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(translations.spinorama_apply_desc)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .when(has_result, |vstack| {
                let theme = theme.clone();
                let button_theme = button_theme.clone();

                vstack
                    .child(self.render_apply_to_playback_card(
                        cx,
                        "spinorama",
                        &theme,
                        &button_theme,
                        Self::apply_spinorama_eq_result,
                        Self::clear_eq_from_playback,
                    ))
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new(translations.spinorama_export)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        Text::new(translations.spinorama_select_export)
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
                                                            "spinorama-export-format-{}",
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
                                                                    .spinorama_eq_state
                                                                    .export_format = value.clone();
                                                            });
                                                            cx.notify();
                                                        },
                                                    ))
                                                }),
                                        )
                                    })
                                    .child(dev_track!(
                                        Button::new(
                                            "save-spinorama-eq",
                                            discovery_text.save_eq_file,
                                        )
                                        .variant(ButtonVariant::Primary)
                                        .size(ButtonSize::Sm)
                                        .theme(button_theme.clone())
                                        .on_click_event(
                                            cx.listener(|view, _, _, cx| {
                                                view.save_spinorama_eq_result(cx);
                                            }),
                                        ),
                                        "spinorama.export_save"
                                    )),
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
                            Text::new(translations.spinorama_no_results)
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            Text::new(translations.spinorama_go_back_optimize)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
            })
    }
}
