use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Card, HStack, StackSpacing, Text, TextSize, TextWeight, VStack};

impl PlayerView {
    // ========================================================================
    // Step 3: Review
    // ========================================================================

    pub(crate) fn render_spinorama_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;
        let result = spinorama.result.as_ref();
        let full_result = spinorama.full_result.as_ref();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Review Results")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Review the optimized EQ results and frequency response.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            // Graphs card (if full_result is available)
            .when_some(full_result.cloned(), |vstack, full_res| {
                let theme_for_graphs = theme.clone();
                let initial_loss = full_res.initial_loss;
                let final_loss = full_res.final_loss;
                let loss_improvement = initial_loss - final_loss;

                vstack
                    .child(
                        Card::new()
                            .background(theme_for_graphs.surface)
                            .header_background(theme_for_graphs.background_secondary)
                            .border(theme_for_graphs.border)
                            .header(
                                Text::new("Optimization Results")
                                    .color(theme_for_graphs.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Lg)
                                            .child(
                                                Text::new(format!(
                                                    "Loss Before: {:.4}",
                                                    initial_loss
                                                ))
                                                .color(theme_for_graphs.text_primary),
                                            )
                                            .child(
                                                Text::new(format!("Loss After: {:.4}", final_loss))
                                                    .color(theme_for_graphs.text_primary),
                                            )
                                            .child(
                                                Text::new(format!(
                                                    "Improvement: {:.4}",
                                                    loss_improvement
                                                ))
                                                .color(if loss_improvement > 0.0 {
                                                    theme_for_graphs.success
                                                } else {
                                                    theme_for_graphs.error
                                                }),
                                            ),
                                    )
                                    .child(
                                        Text::new(format!(
                                            "{} filters generated",
                                            full_res.biquads.len()
                                        ))
                                        .size(TextSize::Sm)
                                        .color(theme_for_graphs.text_secondary),
                                    ),
                            ),
                    )
                    .child(
                        Card::new()
                            .background(theme_for_graphs.surface)
                            .header_background(theme_for_graphs.background_secondary)
                            .border(theme_for_graphs.border)
                            .header(
                                Text::new("Frequency Response")
                                    .color(theme_for_graphs.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(self.render_speaker_optimization_result_graphs(
                                &full_res,
                                &theme_for_graphs,
                                1200.0,
                            )),
                    )
            })
            .when_some(result, |vstack, result| {
                let theme = theme.clone();
                let biquads = result.biquads.clone();

                vstack.child(
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
                                .id("spinorama-filter-list-scroll")
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
            })
            .when(result.is_none() && full_result.is_none(), |vstack| {
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
