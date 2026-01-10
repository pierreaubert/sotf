use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 3: Listen
    // ========================================================================

    pub(crate) fn render_headphone_eq_listen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let theme_id = state.app.ui_state.theme_id;
        let button_theme = ButtonTheme::from(&theme.to_ui_kit_theme(theme_id));
        let headphone_eq = &state.app.headphone_eq_state;
        let result = headphone_eq.result.as_ref();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Listen & Preview")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Preview the optimized EQ and apply it to your playback.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .when_some(result, |vstack, result| {
                let theme = theme.clone();
                // Filter out near-zero gain filters (|gain| < 0.1 dB are effectively disabled)
                let active_biquads: Vec<_> = result
                    .biquads
                    .iter()
                    .filter(|b| b.db_gain.abs() >= 0.1)
                    .cloned()
                    .collect();
                let num_filters = active_biquads.len();
                let biquads = active_biquads;

                vstack
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new("Optimization Results")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Lg)
                                            .child(Text::new(format!(
                                                "Before: {:.2}",
                                                result.pre_score
                                            )))
                                            .child(Text::new(format!(
                                                "After: {:.2}",
                                                result.post_score
                                            )))
                                            .child(
                                                Text::new(format!(
                                                    "Improvement: {:.2}",
                                                    result.pre_score - result.post_score
                                                ))
                                                .color(if result.post_score < result.pre_score {
                                                    theme.success
                                                } else {
                                                    theme.error
                                                }),
                                            ),
                                    )
                                    .child(
                                        Text::new(format!("{} filters generated", num_filters))
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    ),
                            ),
                    )
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new("Response Visualization")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(self.render_optimization_result_graphs(result, &theme, 1200.0)),
                    )
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new("EQ Filters")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                div()
                                    .id("filter-list-scroll")
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .p_2()
                                    .rounded_md()
                                    .bg(theme.surface)
                                    .max_h(px(200.0))
                                    .overflow_y_scroll()
                                    .children(biquads.iter().enumerate().map(|(i, biquad)| {
                                        let filter_type = biquad.filter_type.clone();
                                        let freq = biquad.freq;
                                        let q = biquad.q;
                                        let gain = biquad.db_gain;

                                        div()
                                            .flex()
                                            .justify_between()
                                            .items_center()
                                            .px_2()
                                            .py_1()
                                            .rounded(px(4.0))
                                            .bg(theme.background)
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.accent)
                                                            .child(format!("#{}", i + 1)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_secondary)
                                                            .child(filter_type),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_primary)
                                                            .child(format!("{:.0} Hz", freq)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_muted)
                                                            .child(format!("Q {:.2}", q)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(if gain >= 0.0 {
                                                                theme.success
                                                            } else {
                                                                theme.error
                                                            })
                                                            .child(format!("{:+.1} dB", gain)),
                                                    ),
                                            )
                                    })),
                            ),
                    )
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new("Playback Preview")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Text::new(
                                            "Apply the EQ to your current playback to hear the difference.",
                                        )
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .child(
                                                Button::new(
                                                    "apply-to-playback",
                                                    "Apply to Playback",
                                                )
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Md)
                                                .theme(button_theme.clone())
                                                .build()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.apply_headphone_eq_result(cx);
                                                    }),
                                                ),
                                            )
                                            .child(
                                                Button::new("clear-eq", "Clear EQ")
                                                    .variant(ButtonVariant::Secondary)
                                                    .size(ButtonSize::Md)
                                                    .theme(button_theme.clone())
                                                    .build()
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(|view, _, _, cx| {
                                                            view.clear_headphone_eq_from_playback(
                                                                cx,
                                                            );
                                                        }),
                                                    ),
                                            ),
                                    ),
                            ),
                    )
            })
            .when(result.is_none(), |vstack| {
                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new("No Results")
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            Text::new("Go back and run optimization to generate an EQ curve.")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        ),
                )
            })
    }
}
