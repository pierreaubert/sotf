use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 4: Save
    // ========================================================================

    pub(crate) fn render_headphone_eq_save(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let headphone_eq = &state.app.headphone_eq_state;
        let result = headphone_eq.result.as_ref();
        let export_format = headphone_eq.export_format.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Save EQ")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Choose an export format and save your EQ configuration.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .when_some(result, |vstack, _result| {
                vstack
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(Text::new("Export Format").color(theme.text_primary).weight(TextWeight::Semibold))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Text::new("Select the format for your EQ file.")
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .wrap(true)
                                            .children(
                                                sotf_audio_player::autoeq::EQ_EXPORT_FORMAT_OPTIONS.iter().map(
                                                    |(value, label, _ext)| {
                                                        let is_selected = export_format == *value;
                                                        let value = value.to_string();

                                                        Button::new(
                                                            SharedString::from(format!(
                                                                "export-format-{}",
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
                                                                                .headphone_eq_state
                                                                                .export_format =
                                                                                value.clone();
                                                                        },
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            ),
                                                        )
                                                    },
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
                            .header(Text::new("Save").color(theme.text_primary).weight(TextWeight::Semibold))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Button::new("save-eq", "Save EQ File")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Lg)
                                            .full_width(true)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.save_headphone_eq_result(cx);
                                                }),
                                            ),
                                    )
                                    .child(
                                        Text::new(
                                            "Your EQ will be saved to ~/Library/Application Support/org.spinorama.sotf/EQ",
                                        )
                                        .size(TextSize::Xs)
                                        .color(theme.text_muted),
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
                        .header(Text::new("No Results").color(theme.text_primary).weight(TextWeight::Semibold))
                        .content(
                            Text::new("Go back and run optimization to generate an EQ curve.")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        ),
                )
            })
    }
}
