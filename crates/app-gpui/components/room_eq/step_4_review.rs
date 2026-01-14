use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui_ui_kit::{
    Card, HStack, Select, SelectOption, StackSpacing, Text, TextSize, TextWeight, VStack,
};

use super::render::render_channel_result_card;

impl PlayerView {
    pub(crate) fn render_room_eq_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let pre_score = state
            .app
            .measurement_state
            .room_eq_state
            .average_pre_score();
        let post_score = state
            .app
            .measurement_state
            .room_eq_state
            .average_post_score();
        let smoothing_octaves = state
            .app
            .measurement_state
            .room_eq_state
            .review_smoothing_octaves;
        let smoothing_dropdown_open = state
            .app
            .measurement_state
            .room_eq_state
            .dropdowns
            .review_smoothing_open;

        let view = cx.entity().clone();

        // Smoothing options
        let smoothing_options = vec![
            SelectOption::new("0", "None"),
            SelectOption::new("0.25", "1/4 Oct"),
            SelectOption::new("0.5", "1/2 Oct"),
            SelectOption::new("1", "1 Oct"),
            SelectOption::new("2", "2 Oct"),
            SelectOption::new("3", "3 Oct"),
        ];

        let selected_smoothing = format!("{}", smoothing_octaves);

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
            // Graph settings card
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Graph Settings")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        HStack::new().spacing(StackSpacing::Lg).child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(
                                    Text::new("Smoothing:")
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Select::new("review_smoothing_select")
                                        .options(smoothing_options)
                                        .selected(selected_smoothing)
                                        .placeholder("Smoothing")
                                        .is_open(smoothing_dropdown_open)
                                        .theme(theme.to_select_theme())
                                        .on_toggle({
                                            let view = view.clone();
                                            move |open, _window, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.state.update(cx, |state, _| {
                                                        state
                                                            .app
                                                            .measurement_state
                                                            .room_eq_state
                                                            .dropdowns
                                                            .review_smoothing_open = open;
                                                    });
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .on_change({
                                            let view = view.clone();
                                            move |value, _window, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.state.update(cx, |state, _| {
                                                        if let Ok(oct) = value.parse::<f64>() {
                                                            state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .review_smoothing_octaves = oct;
                                                        }
                                                        state
                                                            .app
                                                            .measurement_state
                                                            .room_eq_state
                                                            .dropdowns
                                                            .review_smoothing_open = false;
                                                    });
                                                    cx.notify();
                                                });
                                            }
                                        }),
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
        let channel_results = state
            .app
            .measurement_state
            .room_eq_state
            .channel_results
            .clone();
        let smoothing_octaves = state
            .app
            .measurement_state
            .room_eq_state
            .review_smoothing_octaves;

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
                    .map(|result| render_channel_result_card(result, &theme, smoothing_octaves)),
            )
            .into_any_element()
    }
}
