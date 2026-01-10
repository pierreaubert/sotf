use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 1: Measurement & Target
    // ========================================================================

    pub(crate) fn render_headphone_eq_measurement_target(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let theme_id = state.app.ui_state.theme_id;
        let button_theme = ButtonTheme::from(&theme.to_ui_kit_theme(theme_id));
        let headphone_eq = &state.app.headphone_eq_state;

        let measurement_path = headphone_eq.measurement_path.clone().unwrap_or_default();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Select Measurement")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Choose your headphone measurement file.")
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
                            .weight(TextWeight::Semibold),
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
                                            .theme(button_theme.clone())
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
    }
}
