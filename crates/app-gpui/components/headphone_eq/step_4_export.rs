use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 4: Export
    // ========================================================================

    pub(crate) fn render_headphone_eq_export(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let theme_id = state.app.ui_state.theme_id;
        let button_theme = ButtonTheme::from(&theme.to_ui_kit_theme(theme_id));
        let headphone_eq = &state.app.measurement_state.headphone_eq_state;
        let has_result = headphone_eq.result.is_some();
        let export_format = headphone_eq.export_format.clone();

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new("Apply & Export")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new("Apply the EQ to playback or export to various formats.")
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .when(has_result, |vstack| {
                let theme = theme.clone();
                let button_theme = button_theme.clone();

                vstack
                    .child(self.render_apply_to_playback_card(
                        cx,
                        "headphone",
                        &theme,
                        &button_theme,
                        Self::apply_headphone_eq_result,
                        Self::clear_eq_from_playback,
                    ))
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new("Export")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        Text::new("Select export format and save your EQ.")
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .child({
                                        let button_theme = button_theme.clone();
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .wrap(true)
                                            .children(
                                                sotf_audio_player::autoeq::EQ_EXPORT_FORMAT_OPTIONS
                                                    .iter()
                                                    .map(|(value, label, _ext)| {
                                                        let is_selected =
                                                            export_format == *value;
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
                                                        .build()
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view, _, _, cx| {
                                                                    view.state.update(
                                                                        cx,
                                                                        |state, _cx| {
                                                                            state
                                                                                .app
                                                                                .measurement_state
                                                                                .headphone_eq_state
                                                                                .export_format =
                                                                                value.clone();
                                                                        },
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            ),
                                                        )
                                                    }),
                                            )
                                    })
                                    .child(
                                        Button::new("save-headphone-eq", "Save EQ File")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Sm)
                                            .theme(button_theme.clone())
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
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
                            Text::new("No Results")
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            Text::new(
                                "Go back and run optimization to generate an EQ curve.",
                            )
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                        ),
                )
            })
    }
}
