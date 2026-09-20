#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::SpeakerGraphTranslations;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Accordion, AccordionItem, Card, HStack, StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 3: Review
    // ========================================================================

    pub(crate) fn render_spinorama_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let graph_text = SpeakerGraphTranslations::for_language(state.app.ui_state.language);
        let spinorama = &state.app.measurement_state.spinorama_eq_state;
        let ui = &state.app.ui_state;
        let sizing = crate::ui::resolve_sizing_context(
            ui.window_width,
            ui.window_height,
            ui.font_scale,
            ui.min_font_size_px,
            ui.max_font_size_px,
        );
        let content_rems = sizing.desktop_content_width_rems(ui.primary_nav_collapsed);
        let workflow_rail_rems = if self.workflow_is_compact(cx) {
            0.0
        } else {
            15.0
        };
        // Workflow body and graph card each inset their content.
        let graph_area_width =
            ((content_rems - workflow_rail_rems - 4.0 * d.card.0) * sizing.effective_rem).max(1.0);
        let result = spinorama.result.as_ref();
        let full_result = spinorama.full_result.as_ref();
        let (directivity_title, directivity_help) =
            SpeakerGraphTranslations::directivity_review(state.app.ui_state.language);
        let owner = cx.entity().downgrade();
        let stale_text =
            crate::app::i18n::DesktopTranslations::for_language(state.app.ui_state.language)
                .stale_result;

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(translations.spinorama_review_results)
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(translations.spinorama_review_desc)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .when(
                result.is_some() && !spinorama.result_is_current(),
                |stack| stack.child(Text::body(stale_text).color(theme.warning)),
            )
            // Use the captured result identity, never the editable selection: a
            // stale result must continue to identify the dataset it actually used.
            .when_some(spinorama.result_inputs.as_ref(), |stack, inputs| {
                let config = inputs.get("config");
                let mode = config
                    .and_then(|config| config.get("mode"))
                    .and_then(|value| {
                        serde_json::from_value::<crate::app::types::SpinoramaOptimizationMode>(
                            value.clone(),
                        )
                        .ok()
                    });
                let target = config
                    .and_then(|config| config.get("target_curve"))
                    .and_then(|value| {
                        serde_json::from_value::<crate::app::types::SpinoramaTargetCurve>(
                            value.clone(),
                        )
                        .ok()
                    });
                let identity = div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap(d.gap)
                    .children(
                        [
                            (translations.spinorama_selected_speaker, "speaker"),
                            (graph_text.origin_version, "version"),
                            (translations.spinorama_frequency_response, "curve"),
                        ]
                        .into_iter()
                        .filter_map(|(label, key)| {
                            inputs
                                .get(key)
                                .and_then(serde_json::Value::as_str)
                                .map(|value| {
                                    let value = if key == "version" {
                                        match inputs
                                            .get("measurement")
                                            .and_then(serde_json::Value::as_str)
                                        {
                                            Some(measurement) => format!("{value} · {measurement}"),
                                            None => value.to_string(),
                                        }
                                    } else if key == "curve" {
                                        // Dispatch defaults an empty curve to PIR; otherwise
                                        // both supported objectives use the configured target.
                                        if value.is_empty() {
                                            "Estimated In-Room Response".to_string()
                                        } else {
                                            target
                                                .map(|target| target.api_name())
                                                .unwrap_or(value)
                                                .to_string()
                                        }
                                    } else {
                                        value.to_string()
                                    };
                                    div()
                                        .min_w_0()
                                        .child(Text::caption(label).color(theme.text_secondary))
                                        .child(Text::body(value).color(theme.text_primary))
                                })
                        }),
                    )
                    .when_some(mode, |identity, mode| {
                        identity.child(
                            div()
                                .min_w_0()
                                .child(
                                    Text::caption(translations.spinorama_optimization_mode)
                                        .color(theme.text_secondary),
                                )
                                .child(Text::body(mode.as_str()).color(theme.text_primary)),
                        )
                    });
                #[cfg(feature = "dev-api")]
                let identity = identity.dev_track("spinorama.review.identity");
                stack.child(identity)
            })
            .when(result.is_some() || full_result.is_some(), |stack| {
                stack.child(
                    Accordion::new()
                        .aria_label(directivity_title)
                        .bordered(false)
                        .expanded(spinorama.expanded_sections.clone())
                        .item(
                            AccordionItem::new("spinorama-directivity-review", directivity_title)
                                .content(Text::body(directivity_help).color(theme.text_secondary)),
                        )
                        .on_change(move |id, expanded, _, cx| {
                            let _ = owner.update(cx, |view, cx| {
                                view.state.update(cx, |state, _| {
                                    let sections = &mut state
                                        .app
                                        .measurement_state
                                        .spinorama_eq_state
                                        .expanded_sections;
                                    sections.retain(|section| section != id);
                                    if expanded {
                                        sections.push(id.clone());
                                    }
                                });
                                cx.notify();
                            });
                        }),
                )
            })
            // Graphs card (if full_result is available)
            .when_some(full_result.cloned(), |vstack, full_res| {
                let theme_for_graphs = theme.clone();
                let initial_loss = full_res.initial_loss;
                let final_loss = full_res.final_loss;
                let loss_improvement = initial_loss - final_loss;

                // Get scores from progress_history (first and last entries with score)
                let progress_history = &spinorama.progress_history;
                let initial_score = progress_history.iter().find_map(|(_, _, score)| *score);
                let final_score = progress_history
                    .iter()
                    .rev()
                    .find_map(|(_, _, score)| *score);
                let score_improvement = match (initial_score, final_score) {
                    (Some(init), Some(fin)) => Some(fin - init),
                    _ => None,
                };

                vstack
                    .child(
                        Card::new()
                            .background(theme_for_graphs.surface)
                            .header_background(theme_for_graphs.background_secondary)
                            .border(theme_for_graphs.border)
                            .header(
                                Text::new(translations.spinorama_optimization_results)
                                    .color(theme_for_graphs.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Md)
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
                                    .when(
                                        initial_score.is_some() || final_score.is_some(),
                                        |vstack| {
                                            vstack.child(
                                                HStack::new()
                                                    .spacing(StackSpacing::Md)
                                                    .when_some(initial_score, |hstack, score| {
                                                        hstack.child(
                                                            Text::new(format!(
                                                                "Score Before: {:.2}",
                                                                score
                                                            ))
                                                            .color(theme_for_graphs.text_primary),
                                                        )
                                                    })
                                                    .when_some(final_score, |hstack, score| {
                                                        hstack.child(
                                                            Text::new(format!(
                                                                "Score After: {:.2}",
                                                                score
                                                            ))
                                                            .color(theme_for_graphs.text_primary),
                                                        )
                                                    })
                                                    .when_some(
                                                        score_improvement,
                                                        |hstack, improvement| {
                                                            hstack.child(
                                                                Text::new(format!(
                                                                    "Improvement: {:+.2}",
                                                                    improvement
                                                                ))
                                                                .color(if improvement > 0.0 {
                                                                    theme_for_graphs.success
                                                                } else {
                                                                    theme_for_graphs.error
                                                                }),
                                                            )
                                                        },
                                                    ),
                                            )
                                        },
                                    )
                                    .child(
                                        Text::new(format!(
                                            "{} filters generated",
                                            full_res.biquads.len()
                                        ))
                                        .size(TextSize::Xs)
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
                                Text::new(translations.spinorama_frequency_response)
                                    .color(theme_for_graphs.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(self.render_speaker_optimization_result_graphs(
                                &d,
                                &full_res,
                                graph_text,
                                &theme_for_graphs,
                                graph_area_width,
                                sizing.effective_rem,
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
                            Text::new(translations.spinorama_eq_filters)
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            div()
                                .id("spinorama-filter-list-scroll")
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
            .when(result.is_none() && full_result.is_none(), |vstack| {
                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new(translations.spinorama_no_results)
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            Text::new(translations.spinorama_go_back_optimize)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
            })
    }
}
