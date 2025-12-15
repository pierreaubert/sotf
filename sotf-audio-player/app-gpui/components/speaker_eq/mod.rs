//! Speaker settings content
//!
//! UI for Spinorama speaker optimization (Work in Progress)

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui_ui_kit::{Card, Text, TextSize, TextWeight, VStack};

impl PlayerView {
    pub(crate) fn render_speaker_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        VStack::new()
            .spacing(gpui_ui_kit::StackSpacing::Lg)
            .child(
                Card::new()
                    .header(Text::new("Speaker EQ Optimization").weight(TextWeight::Semibold))
                    .content(
                        VStack::new().spacing(gpui_ui_kit::StackSpacing::Md).child(
                            Text::new(
                                "Optimize speakers using Spinorama.org measurements. Coming soon.",
                            )
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                        ),
                    ),
            )
            .into_any_element()
    }
}
