use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Card, Text, TextSize, TextWeight, VStack};

impl PlayerView {
    pub(crate) fn render_spinorama_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        VStack::new()
            .spacing(gpui_ui_kit::StackSpacing::Lg)
            .child(
                Card::new()
                    .header(Text::new("Room Mode Removal").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                             .spacing(gpui_ui_kit::StackSpacing::Md)
                             .child(
                                 Text::new("Remove room effects from measurements using multiple distances.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary)
                             )
                             // TODO: Add file pickers for Room Correction Input
                    )
            )
            .into_any_element()
    }
}
