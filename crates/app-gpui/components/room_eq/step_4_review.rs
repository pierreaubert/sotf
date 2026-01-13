use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui_ui_kit::{Card, HStack, StackSpacing, Text, TextSize, TextWeight, VStack};

use super::render::render_channel_result_card;

impl PlayerView {
    pub(crate) fn render_room_eq_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let pre_score = state.app.measurement_state.room_eq_state.average_pre_score();
        let post_score = state.app.measurement_state.room_eq_state.average_post_score();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Review Results")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Review the optimization results before applying.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Score Summary")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new().spacing(StackSpacing::Sm).child(
                            HStack::new()
                                .spacing(StackSpacing::Lg)
                                .child(
                                    Text::new(format!("Before: {:.2}", pre_score))
                                        .color(theme.text_primary),
                                )
                                .child(
                                    Text::new(format!("After: {:.2}", post_score))
                                        .color(theme.text_primary),
                                )
                                .child(
                                    Text::new(format!(
                                        "Improvement: {:.2}",
                                        pre_score - post_score
                                    ))
                                    .color(
                                        if post_score < pre_score {
                                            theme.success
                                        } else {
                                            theme.error
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
                    .header(
                        Text::new("Per-Channel Results")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(self.render_channel_results(cx)),
            )
    }

    /// Render per-channel optimization results
    fn render_channel_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let channel_results = state.app.measurement_state.room_eq_state.channel_results.clone();

        if channel_results.is_empty() {
            return VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("No optimization results yet. Run optimization first.")
                        .size(TextSize::Sm)
                        .color(theme.text_muted),
                )
                .into_any_element();
        }

        VStack::new()
            .spacing(StackSpacing::Lg)
            .children(
                channel_results
                    .iter()
                    .map(|result| render_channel_result_card(result, &theme)),
            )
            .into_any_element()
    }
}
