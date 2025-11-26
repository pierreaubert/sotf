//! Spectrum screen rendering functions

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_spectrum_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        let content = if let Some(info) = &state.app.spectrum_info {
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap_1()
                        .h_64()
                        .w_full()
                        .bg(rgb(0x000000))
                        .p_2()
                        .children(info.magnitudes.iter().enumerate().map(|(i, &mag)| {
                            let normalized = ((mag + 100.0) / 100.0).clamp(0.0, 1.0);
                            let color = if normalized > 0.9 {
                                rgb(0xff0000)
                            } else if normalized > 0.7 {
                                rgb(0xffff00)
                            } else {
                                rgb(0x00ff00)
                            };

                            div()
                                .w_full()
                                .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                    normalized,
                                )))
                                .bg(color)
                                .rounded_t_sm()
                        })),
                )
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(rgb(0x999999))
                        .child("20 Hz")
                        .child("1 kHz")
                        .child("20 kHz"),
                )
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(rgb(0x666666))
                .child("No spectrum data available. Play audio to see visualization.")
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Spectrum Analyzer"),
            )
            .child(content)
    }
}
