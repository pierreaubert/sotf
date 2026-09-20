use crate::components::design::Ds;
use crate::i18n::{HeadphoneEqTranslations, HeadphoneGraphTranslations};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, NumberInput, NumberInputSize, StackSpacing,
    Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    fn render_headphone_audition(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let headphone = &state.app.measurement_state.headphone_eq_state;
        let theme = &state.app.ui_state.theme;
        let text = crate::app::i18n::HeadphoneAuditionTranslations::for_language(
            state.app.ui_state.language,
        );
        let session = headphone.audition.as_ref();
        let pending = session.is_some_and(|session| session.is_pending());
        let enabled = headphone.result_is_current() && !pending;
        let d = Ds::from_cx(cx);
        div()
            .flex()
            .flex_col()
            .gap(d.gap)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.gap)
                    .children([false, true].map(|corrected| {
                        let entity = self.state.clone();
                        let button = Button::new(
                            if corrected {
                                "headphone-audition-corrected"
                            } else {
                                "headphone-audition-original"
                            },
                            if corrected {
                                text.corrected
                            } else {
                                text.original
                            },
                        )
                        .size(ButtonSize::Sm)
                        .variant(
                            if session.is_some_and(|session| session.corrected == corrected) {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            },
                        )
                        .disabled(!enabled)
                        .theme(theme.to_button_theme())
                        .on_click_event(move |_, _, cx| {
                            entity.update(cx, |state, cx| {
                                if let Err(error) = state.app.preview_headphone_eq(corrected) {
                                    state.app.ui_state.toast_message =
                                        Some(crate::app::ToastMessage::error(error));
                                }
                                cx.notify();
                            });
                        });
                        #[cfg(feature = "dev-api")]
                        let button = {
                            use crate::app::dev_api::DevTrackExt;
                            button.dev_track(if corrected {
                                "headphone.audition.corrected"
                            } else {
                                "headphone.audition.original"
                            })
                        };
                        button
                    }))
                    .child({
                        let control = Button::new("headphone-audition-stop", text.stop)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .disabled(session.is_none())
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                view.state.update(cx, |state, cx| {
                                    if let Err(error) = state.app.stop_headphone_audition() {
                                        state.app.ui_state.toast_message =
                                            Some(crate::app::ToastMessage::error(error));
                                    }
                                    cx.notify();
                                });
                            }));
                        #[cfg(feature = "dev-api")]
                        let control = {
                            use crate::app::dev_api::DevTrackExt;
                            control.dev_track("headphone.audition.stop")
                        };
                        control
                    }),
            )
            .when_some(session, |view, session| {
                let entity = self.state.clone();
                view.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(d.gap)
                        .child(Text::label(text.preamp))
                        .child({
                            let control = NumberInput::new("headphone-audition-preamp")
                                .value(session.preamp_db)
                                .min(-120.0)
                                .max(session.max_preamp_db)
                                .step(0.1)
                                .decimals(1)
                                .unit("dB")
                                .size(NumberInputSize::Sm)
                                .disabled(pending)
                                .aria_label(text.preamp)
                                .on_change(move |value, _, cx| {
                                    entity.update(cx, |state, cx| {
                                        if let Err(error) =
                                            state.app.set_headphone_audition_preamp(value)
                                        {
                                            state.app.ui_state.toast_message =
                                                Some(crate::app::ToastMessage::error(error));
                                        }
                                        cx.notify();
                                    });
                                });
                            #[cfg(feature = "dev-api")]
                            let control = {
                                use crate::app::dev_api::DevTrackExt;
                                control.dev_track("headphone.audition.preamp")
                            };
                            control
                        })
                        .child(Text::caption(format!(
                            "{}: {:.1} dB",
                            text.headroom, -session.max_preamp_db
                        ))),
                )
            })
            .when(session.is_none(), |view| {
                view.child(Text::caption(text.prepare))
            })
            .when(pending, |view| view.child(Text::caption(text.pending)))
            .when(!state.app.playback.is_playing, |view| {
                view.child(Text::caption(text.paused))
            })
    }

    // ========================================================================
    // Step 3: Listen
    // ========================================================================

    pub(crate) fn render_headphone_eq_listen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let audition = self.render_headphone_audition(cx).into_any_element();
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = HeadphoneEqTranslations::for_language(state.app.ui_state.language);
        let graph_text = HeadphoneGraphTranslations::for_language(state.app.ui_state.language);
        let headphone_eq = &state.app.measurement_state.headphone_eq_state;
        let result = headphone_eq.result.as_ref();

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(translations.listen_preview)
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(translations.listen_preview_description)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(audition)
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
                                Text::new(translations.optimization_results)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Md)
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
                                            .size(TextSize::Xs)
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
                                Text::new(translations.response_visualization)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(self.render_optimization_result_graphs(
                                &d, result, graph_text, &theme, 1200.0,
                            )),
                    )
                    .child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new(translations.eq_filters)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                div()
                                    .id("filter-list-scroll")
                                    .flex()
                                    .flex_col()
                                    .gap(d.grid)
                                    .p(d.pad_y)
                                    .rounded(d.r_md)
                                    .bg(theme.surface)
                                    // intentional: fixed scroll container height (not a spacing token)
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
                                            .px(d.pad_y)
                                            .py(d.pad_y_half)
                                            .rounded(d.r_md)
                                            .bg(theme.background)
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(d.gap)
                                                    .child(
                                                        div()
                                                            .text_size(d.text_xs)
                                                            .text_color(theme.accent)
                                                            .child(format!("#{}", i + 1)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(d.text_xs)
                                                            .text_color(theme.text_secondary)
                                                            .child(filter_type),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(d.gap_md)
                                                    .child(
                                                        div()
                                                            .text_size(d.text_xs)
                                                            .text_color(theme.text_primary)
                                                            .child(format!("{:.0} Hz", freq)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(d.text_xs)
                                                            .text_color(theme.text_muted)
                                                            .child(format!("Q {:.2}", q)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(d.text_xs)
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
            .when(result.is_none(), |vstack| {
                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new(translations.no_results)
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            Text::new(translations.no_results_description)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
            })
    }
}
