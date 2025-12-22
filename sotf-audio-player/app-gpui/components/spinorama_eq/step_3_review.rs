use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 4: Review
    // ========================================================================

    pub(crate) fn render_spinorama_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;
        let result = spinorama.result.as_ref();
        let full_result = spinorama.full_result.as_ref();
        let export_format = spinorama.export_format.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Review & Apply")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Review the optimized EQ and apply it to your playback.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            // Graphs card (if full_result is available)
            .when_some(full_result.cloned(), |vstack, full_res| {
                let theme_for_graphs = theme.clone();
                vstack.child(
                    Card::new()
                        .background(theme_for_graphs.surface)
                        .header_background(theme_for_graphs.background_secondary)
                        .border(theme_for_graphs.border)
                        .header(Text::new("Frequency Response").color(theme_for_graphs.text_primary).weight(TextWeight::Semibold))
                        .content(
                            self.render_speaker_optimization_result_graphs(&full_res, &theme_for_graphs, 1200.0)
                        ),
                )
            })
            .when_some(result, |vstack, result| {
                let theme = theme.clone();
                let num_filters = result.biquads.len();
                let biquads = result.biquads.clone();

                vstack
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(Text::new("Optimization Results").color(theme.text_primary).weight(TextWeight::Semibold))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Lg)
                                            .child(
                                                Text::new(format!("Before: {:.2}", result.pre_score))
                                                    .color(theme.text_primary),
                                            )
                                            .child(
                                                Text::new(format!("After: {:.2}", result.post_score))
                                                    .color(theme.text_primary),
                                            )
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
                            .header(Text::new("EQ Filters").color(theme.text_primary).weight(TextWeight::Semibold))
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
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(Text::new("Apply to Playback").color(theme.text_primary).weight(TextWeight::Semibold))
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
                                                    "apply-spinorama-eq",
                                                    "Apply to Playback",
                                                )
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Md)
                                                .build()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.apply_spinorama_eq_result(cx);
                                                    }),
                                                ),
                                            )
                                            .child(
                                                Button::new("clear-spinorama-eq", "Clear EQ")
                                                    .variant(ButtonVariant::Secondary)
                                                    .size(ButtonSize::Md)
                                                    .build()
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(|view, _, _, cx| {
                                                            view.clear_spinorama_eq_from_playback(cx);
                                                        }),
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
                            .header(Text::new("Export").color(theme.text_primary).weight(TextWeight::Semibold))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Text::new("Select export format and save your EQ.")
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .wrap(true)
                                            .children(
                                                sotf_audio_player::autoeq::EQ_EXPORT_FORMAT_OPTIONS.iter().map(
                                                    |(value, label, _ext)| {
                                                        let is_selected = export_format == *value;
                                                        let value = value.to_string();

                                                        Button::new(
                                                            SharedString::from(format!(
                                                                "spinorama-export-format-{}",
                                                                value
                                                            )),
                                                            *label,
                                                        )
                                                        .variant(if is_selected {
                                                            ButtonVariant::Primary
                                                        } else {
                                                            ButtonVariant::Secondary
                                                        })
                                                        .size(ButtonSize::Sm)
                                                        .build()
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view, _, _, cx| {
                                                                    view.state.update(
                                                                        cx,
                                                                        |state, _cx| {
                                                                            state
                                                                                .app
                                                                                .spinorama_eq_state
                                                                                .export_format =
                                                                                value.clone();
                                                                        },
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            ),
                                                        )
                                                    },
                                                ),
                                            ),
                                    )
                                    .child(
                                        Button::new("save-spinorama-eq", "Save EQ File")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Md)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.save_spinorama_eq_result(cx);
                                                }),
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
                        .header(Text::new("No Results").color(theme.text_primary).weight(TextWeight::Semibold))
                        .content(
                            Text::new("Go back and run optimization to generate an EQ curve.")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        ),
                )
            })
    }
}
