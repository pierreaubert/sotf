//! Plugins settings content

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    /// Render plugins settings content
    pub(crate) fn render_plugins_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();

        div().flex().flex_col().gap_6().child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(translations.settings_tab_plugins),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("Plugin configuration coming soon."),
                ),
        )
    }
}
