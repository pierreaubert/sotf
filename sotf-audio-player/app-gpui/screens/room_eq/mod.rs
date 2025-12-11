use crate::app::types::MeasureState;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonVariant, Card, Text, TextSize, TextWeight, VStack};

impl PlayerView {
    pub(crate) fn render_roomeq_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        VStack::new()
            .spacing(gpui_ui_kit::StackSpacing::Lg)
            .child(
                Card::new()
                    .header(Text::new("Data Acquisition").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                             .spacing(gpui_ui_kit::StackSpacing::Md)
                             .child(
                                 Text::new("Measure your room impulse response to calculate correction filters.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary)
                             )
                             .child(
                                 Button::new("meas_btn", "Measure Room Response")
                                     .variant(ButtonVariant::Primary)
                                     .build()
                                     .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, cx| {
                                         view.state.update(cx, |state, _cx| {
                                             state.app.measure_state = Some(MeasureState::default());
                                         });
                                     }))
                             )
                    )
            )
            .child(
                 Card::new()
                    .header(Text::new("Room Correction (Coming Soon)").weight(TextWeight::Semibold))
                    .content(
                         Text::new("Optimization logic will be integrated after measurements are available.")
                            .size(TextSize::Sm)
                            .color(theme.text_secondary)
                    )
            )
            .into_any_element()
    }
}
