use super::TARGET_CURVE_OPTIONS;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 1: Measurement & Target
    // ========================================================================

    pub(crate) fn render_headphone_eq_measurement_target(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let headphone_eq = &state.app.headphone_eq_state;

        let measurement_path = headphone_eq.measurement_path.clone().unwrap_or_default();
        let target_preset = headphone_eq.target_preset.clone();
        let custom_target_path = headphone_eq.custom_target_path.clone().unwrap_or_default();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Select Measurement & Target")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Choose your headphone measurement file and target curve.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
			Text::new("Measurement File")
			    .color(theme.text_primary)
			    .weight(TextWeight::Semibold)
		    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Select a CSV file with your headphone's frequency response measurement.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        div()
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(theme.background_secondary)
                                            .text_sm()
                                            .text_color(if measurement_path.is_empty() {
                                                theme.text_muted
                                            } else {
                                                theme.text_primary
                                            })
                                            .child(if measurement_path.is_empty() {
                                                "No file selected".to_string()
                                            } else {
                                                measurement_path.clone()
                                            }),
                                    )
                                    .child(
                                        Button::new("browse-measurement", "Browse...")
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Md)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.browse_headphone_eq_measurement(cx);
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
                    .header(Text::new("Target Curve").color(theme.text_primary).weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Select a target curve for your headphone EQ.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .wrap(true)
                                    .children(TARGET_CURVE_OPTIONS.iter().map(|(value, label)| {
                                        let is_selected = target_preset == *value;
                                        let value = value.to_string();
                                        let is_custom = value == "custom";

                                        Button::new(
                                            SharedString::from(format!("hp-target-{}", value)),
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
                                            cx.listener(move |view, _, _, cx| {
                                                if is_custom {
                                                    view.browse_headphone_eq_target(cx);
                                                } else {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.headphone_eq_state.target_preset =
                                                            value.clone();
                                                    });
                                                    cx.notify();
                                                }
                                            }),
                                        )
                                    })),
                            )
                            .when(target_preset == "custom", |vstack| {
                                let theme = theme.clone();
                                vstack.child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            div()
                                                .flex_1()
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(theme.background_secondary)
                                                .text_sm()
                                                .text_color(theme.text_muted)
                                                .child(if custom_target_path.is_empty() {
                                                    "No custom target file selected".to_string()
                                                } else {
                                                    custom_target_path.clone()
                                                }),
                                        )
                                        .child(
                                            Button::new("browse-custom-target", "Change")
                                                .variant(ButtonVariant::Secondary)
                                                .size(ButtonSize::Sm)
                                                .build()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.browse_headphone_eq_target(cx);
                                                    }),
                                                ),
                                        ),
                                )
                            }),
                    ),
            )
    }

}
