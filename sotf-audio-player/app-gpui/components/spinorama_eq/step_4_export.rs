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

    pub(crate) fn render_spinorama_export(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let theme_id = state.app.theme_id;
        let button_theme = ButtonTheme::from(&theme.to_ui_kit_theme(theme_id));
        let spinorama = &state.app.spinorama_eq_state;
        let result = spinorama.result.as_ref();
        let export_format = spinorama.export_format.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Apply & Export")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Apply the EQ to playback or export to various formats.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .when_some(result, |vstack, _result| {
                let theme = theme.clone();
                let button_theme = button_theme.clone();

                vstack
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new("Apply to Playback")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Text::new(
                                            "Apply the EQ to your current playback to hear the difference.",
                                        )
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .child(
                                                Button::new(
                                                    "apply-spinorama-eq",
                                                    "Apply to Playback",
                                                )
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Md)
                                                .theme(button_theme.clone())
                                                .build()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.apply_spinorama_eq_result(cx);
                                                    }),
                                                ),
                                            )
                                            .child(
                                                Button::new("clear-spinorama-eq", "Clear EQ")
                                                    .variant(ButtonVariant::Secondary)
                                                    .size(ButtonSize::Md)
                                                    .theme(button_theme.clone())
                                                    .build()
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(|view, _, _, cx| {
                                                            view.clear_spinorama_eq_from_playback(cx);
                                                        }),
                                                    ),
                                            ),
                                    ),
                            ),
                    )
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
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Text::new("Select export format and save your EQ.")
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                                    .child({
                                        let button_theme = button_theme.clone();
                                        HStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .wrap(true)
                                            .children(
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
                                                        .size(ButtonSize::Sm)
                                                        .theme(button_theme.clone())
                                                        .build()
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(move |view, _, _, cx| {
                                                                view.state.update(
                                                                    cx,
                                                                    |state, _cx| {
                                                                        state
                                                                            .app
                                                                            .spinorama_eq_state
                                                                            .export_format =
                                                                            value.clone();
                                                                    },
                                                                );
                                                                cx.notify();
                                                            }),
                                                        )
                                                    }),
                                            )
                                    })
                                    .child(
                                        Button::new("save-spinorama-eq", "Save EQ File")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Md)
                                            .theme(button_theme.clone())
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.save_spinorama_eq_result(cx);
                                                }),
                                            ),
                                    ),
                            ),
                    )
            })
            .when(result.is_none(), |vstack| {
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
                            Text::new("Go back and run optimization to generate an EQ curve.")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        ),
                )
            })
    }
}
