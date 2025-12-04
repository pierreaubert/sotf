//! Room EQ settings content

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_roomeq_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        div()
            .text_sm()
            .text_color(theme.text_secondary)
            .child("Room equalization and speaker correction settings will be added here.")
    }
}
