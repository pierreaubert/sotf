use super::super::render::{
    render_channel_result_card, render_room_eq_bass_management_report, render_room_eq_epa_card,
    render_room_eq_filters_card, render_room_eq_report_channel, render_room_eq_report_overview,
    render_room_eq_report_summary, room_eq_report_channel_has_renderable_data,
    room_eq_report_data_from_dsp_output,
};
use super::room::room_eq_smoothing_options;
use super::room::room_eq_smoothing_value;
use crate::app::types::room_eq::{RoomEqReviewGraphId, RoomEqReviewGraphSettings};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Card, HStack, Select, SelectSize, StackSpacing, TabItem, TabVariant, Tabs, Text, TextSize,
    TextWeight, Toggle, VStack,
};

impl PlayerView {
    pub(crate) fn render_room_eq_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let d = Ds::from_cx(cx);
        let translations = state.app.ui_state.translations.clone();
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;

        let report = room_eq
            .dsp_output
            .as_ref()
            .map(room_eq_report_data_from_dsp_output);
        let pre_score = report
            .as_ref()
            .and_then(|report| report.pre_score)
            .unwrap_or_else(|| room_eq.average_pre_score());
        let post_score = report
            .as_ref()
            .and_then(|report| report.post_score)
            .unwrap_or_else(|| room_eq.average_post_score());
        let graph_settings = room_eq.review_graph_settings.clone();
        let view = cx.entity().clone();
        let window_width = state.app.ui_state.window_width;

        // Score Summary card is intentionally omitted here: the
        // "Optimization Summary" card below already shows Score Before /
        // Score After / Improvement, so this would duplicate information.
        let _ = (pre_score, post_score);

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(translations.roomeq_review_results)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(translations.roomeq_review_desc)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .when_some(report.as_ref(), |vstack, report| {
                vstack.child(render_room_eq_report_summary(d, report, &theme))
            })
            .when_some(report.as_ref(), |vstack, report| {
                let original_id = RoomEqReviewGraphId::OverviewOriginal;
                let eq_id = RoomEqReviewGraphId::OverviewEq;
                let corrected_id = RoomEqReviewGraphId::OverviewCorrected;
                vstack.child(render_room_eq_report_overview(
                    d,
                    report,
                    &theme,
                    *graph_settings.get(original_id),
                    *graph_settings.get(eq_id),
                    *graph_settings.get(corrected_id),
                    Some(render_review_graph_controls(
                        original_id,
                        *graph_settings.get(original_id),
                        true,
                        view.clone(),
                        &theme,
                    )),
                    Some(render_review_graph_controls(
                        eq_id,
                        *graph_settings.get(eq_id),
                        false,
                        view.clone(),
                        &theme,
                    )),
                    Some(render_review_graph_controls(
                        corrected_id,
                        *graph_settings.get(corrected_id),
                        true,
                        view.clone(),
                        &theme,
                    )),
                    window_width,
                ))
            })
            .when_some(
                report
                    .as_ref()
                    .and_then(|report| report.bass_management.as_ref()),
                |vstack, bass| vstack.child(render_room_eq_bass_management_report(d, bass, &theme)),
            )
            .when_some(room_eq.dsp_output.as_ref(), |vstack, dsp_output| {
                vstack.child(render_fir_temporal_masking_summary(dsp_output, &theme))
            })
            // Selected channel result
            .child(self.render_selected_channel_result(cx))
            // EPA + EQ filter cards: lifted out of the per-channel
            // panel so each channel is visible without tab switching.
            .when_some(report.as_ref(), |vstack, report| {
                vstack.child(render_room_eq_epa_card(d, report, &theme))
            })
            .when_some(report.as_ref(), |vstack, report| {
                vstack.child(render_room_eq_filters_card(d, report, &theme))
            })
    }

    /// Render the selected channel's optimization result
    pub(super) fn render_selected_channel_result(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        use crate::app::types::room_eq::InteractiveChartStateWrapper;

        // Initialize interactive chart state if needed
        {
            let state = self.state.read(cx);
            if state
                .app
                .measurement_state
                .room_eq_state
                .review_chart_state
                .is_none()
            {
                // Drop read borrow before update
                let _ = state;
                self.state.update(cx, |state, _| {
                    // Create interactive state for frequency response chart
                    // X: 20 Hz to 20 kHz (log scale), Y: -40 to +10 dB (50dB zoom-out range)
                    state.app.measurement_state.room_eq_state.review_chart_state = Some(
                        InteractiveChartStateWrapper::new(20.0, 20000.0, -40.0, 10.0)
                            .with_log_x(true)
                            .with_size(800.0, 400.0),
                    );
                });
            }
        }

        let state = self.state.read(cx);
        let translations = state.app.ui_state.translations.clone();
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;
        let channel_results = &room_eq.channel_results;
        let selected_idx = room_eq.review_selected_channel;
        let smoothing_octaves = room_eq.review_smoothing_octaves;
        let y_axis_auto = room_eq.review_y_axis_auto;
        let graph_settings = room_eq.review_graph_settings.clone();
        let chart_state = room_eq.review_chart_state.as_ref().map(|w| w.inner());
        let window_width = state.app.ui_state.window_width;
        let report = room_eq
            .dsp_output
            .as_ref()
            .map(room_eq_report_data_from_dsp_output);
        let channel_names: Vec<String> = report
            .as_ref()
            .map(|report| {
                report
                    .channels
                    .iter()
                    .map(|channel| channel.name.clone())
                    .collect()
            })
            .unwrap_or_else(|| {
                channel_results
                    .iter()
                    .map(|result| result.channel_name.clone())
                    .collect()
            });
        let selected_idx_for_tabs = selected_idx.min(channel_names.len().saturating_sub(1));
        let channel_tabs = render_room_eq_channel_tabs(
            channel_names,
            selected_idx_for_tabs,
            cx.entity().clone(),
            &theme,
        );

        if let Some(report) = report.as_ref()
            && !report.channels.is_empty()
        {
            let idx = selected_idx.min(report.channels.len().saturating_sub(1));
            let channel = &report.channels[idx];
            if room_eq_report_channel_has_renderable_data(channel) {
                let full_id = RoomEqReviewGraphId::ChannelFull;
                let zoom_id = RoomEqReviewGraphId::ChannelZoom;
                let eq_id = RoomEqReviewGraphId::ChannelEq;
                return render_room_eq_channel_panel(
                    d,
                    &theme,
                    translations.roomeq_select_channel,
                    channel_tabs,
                    render_room_eq_report_channel(
                        d,
                        channel,
                        &theme,
                        *graph_settings.get(full_id),
                        *graph_settings.get(zoom_id),
                        *graph_settings.get(eq_id),
                        Some(render_review_graph_controls(
                            full_id,
                            *graph_settings.get(full_id),
                            true,
                            cx.entity().clone(),
                            &theme,
                        )),
                        Some(render_review_graph_controls(
                            zoom_id,
                            *graph_settings.get(zoom_id),
                            true,
                            cx.entity().clone(),
                            &theme,
                        )),
                        Some(render_review_graph_controls(
                            eq_id,
                            *graph_settings.get(eq_id),
                            false,
                            cx.entity().clone(),
                            &theme,
                        )),
                        chart_state,
                        window_width,
                    )
                    .into_any_element(),
                )
                .into_any_element();
            }
        }

        if channel_results.is_empty() {
            return VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new(translations.roomeq_channel_result)
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(Text::caption(
                            "No optimization results yet. Run optimization first.",
                        )),
                )
                .into_any_element();
        }

        // Clamp selected index to valid range
        let idx = selected_idx.min(channel_results.len().saturating_sub(1));
        let result = &channel_results[idx];

        // Detect whether this channel's DSP chain contains an FIR /
        // convolution block so the filter plot can flag it in the legend.
        // We can't decompose the FIR magnitude into parametric bands, but
        // the user needs to know an FIR correction is active — otherwise
        // the "Corrected" curve will show changes that no individual IIR
        // line accounts for.
        let has_fir = room_eq
            .dsp_output
            .as_ref()
            .and_then(|out| {
                super::super::room_eq_channel_chain_by_name(&out.channels, &result.channel_name)
            })
            .map(|chain| {
                chain.plugins.iter().any(|p| {
                    let t = p.plugin_type.to_ascii_lowercase();
                    t == "fir"
                        || t == "convolution"
                        || t == "convolve"
                        || t == "firfilter"
                        || t == "fir_filter"
                })
            })
            .unwrap_or(false);

        let mut display_result = result.clone();
        if let Some(chain) = room_eq.dsp_output.as_ref().and_then(|out| {
            super::super::room_eq_channel_chain_by_name(&out.channels, &result.channel_name)
        }) {
            if let Some(points) = super::super::room_eq_initial_response_points(Some(chain), None) {
                display_result.original_response = Some(points);
            }
            if let Some(points) = super::super::room_eq_display_response_points(Some(chain), None) {
                display_result.corrected_response = Some(points.clone());
                display_result.normalized_response = Some(points);
            }
            display_result.target_curve = chain.target_curve.as_ref().map(|tc| {
                tc.freq
                    .iter()
                    .zip(tc.spl.iter())
                    .map(|(&f, &db)| (f, db))
                    .collect()
            });
        }

        render_room_eq_channel_panel(
            d,
            &theme,
            translations.roomeq_select_channel,
            channel_tabs,
            render_channel_result_card(
                d,
                display_result,
                &theme,
                smoothing_octaves,
                y_axis_auto,
                chart_state,
                has_fir,
            )
            .into_any_element(),
        )
        .into_any_element()
    }
}

fn render_room_eq_channel_panel(
    d: Ds,
    theme: &crate::theme::Theme,
    title: impl Into<SharedString>,
    tabs: gpui::AnyElement,
    body: gpui::AnyElement,
) -> impl IntoElement {
    Card::new()
        .background(theme.surface)
        .header_background(theme.background_secondary)
        .border(theme.border)
        .header(
            Text::new(title.into())
                .color(theme.text_primary)
                .weight(TextWeight::Semibold),
        )
        .content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(tabs)
                .child(div().p(d.card).child(body)),
        )
}

fn render_room_eq_channel_tabs(
    channel_names: Vec<String>,
    selected_idx: usize,
    view: Entity<PlayerView>,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    if channel_names.is_empty() {
        return div().into_any_element();
    }

    Tabs::new("room-eq-review-channel-tabs")
        .tabs(
            channel_names
                .into_iter()
                .enumerate()
                .map(|(idx, name)| TabItem::new(format!("channel-{idx}"), name))
                .collect(),
        )
        .selected_index(selected_idx)
        .variant(TabVariant::Pills)
        .theme(theme.to_tabs_theme())
        .on_change(move |idx, _window, cx| {
            view.update(cx, |this, cx| {
                this.state.update(cx, |state, _| {
                    state
                        .app
                        .measurement_state
                        .room_eq_state
                        .review_selected_channel = idx;
                });
                cx.notify();
            });
        })
        .into_any_element()
}

fn render_review_graph_controls(
    graph_id: RoomEqReviewGraphId,
    settings: RoomEqReviewGraphSettings,
    allow_trend_controls: bool,
    view: Entity<PlayerView>,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    HStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Select::new(SharedString::from(format!(
                "room-eq-review-smoothing-{graph_id:?}"
            )))
            .options(room_eq_smoothing_options())
            .selected(room_eq_smoothing_value(settings.smoothing_octaves))
            .placeholder("Smoothing")
            .size(SelectSize::Sm)
            .is_open(settings.smoothing_open)
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
                                .review_graph_settings
                                .get_mut(graph_id)
                                .smoothing_open = open;
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
                            if let Ok(octaves) = value.as_ref().parse::<f64>() {
                                let settings = state
                                    .app
                                    .measurement_state
                                    .room_eq_state
                                    .review_graph_settings
                                    .get_mut(graph_id);
                                settings.smoothing_octaves = octaves;
                                settings.smoothing_open = false;
                            }
                        });
                        cx.notify();
                    });
                }
            }),
        )
        .child(
            HStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new("Auto")
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                )
                .child(
                    Toggle::new(SharedString::from(format!(
                        "room-eq-review-auto-{graph_id:?}"
                    )))
                    .checked(settings.y_axis_auto)
                    .theme(theme.to_toggle_theme())
                    .on_change({
                        let view = view.clone();
                        move |checked, _window, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    state
                                        .app
                                        .measurement_state
                                        .room_eq_state
                                        .review_graph_settings
                                        .get_mut(graph_id)
                                        .y_axis_auto = checked;
                                });
                                cx.notify();
                            });
                        }
                    }),
                ),
        )
        .when(allow_trend_controls, |controls| {
            controls
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xs)
                        .child(
                            Text::new("Trend")
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Toggle::new(SharedString::from(format!(
                                "room-eq-review-trend-{graph_id:?}"
                            )))
                            .checked(settings.show_trend)
                            .theme(theme.to_toggle_theme())
                            .on_change({
                                let view = view.clone();
                                move |checked, _window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            state
                                                .app
                                                .measurement_state
                                                .room_eq_state
                                                .review_graph_settings
                                                .get_mut(graph_id)
                                                .show_trend = checked;
                                        });
                                        cx.notify();
                                    });
                                }
                            }),
                        ),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xs)
                        .child(
                            Text::new("Normalize")
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Toggle::new(SharedString::from(format!(
                                "room-eq-review-normalize-{graph_id:?}"
                            )))
                            .checked(settings.normalize_to_trend)
                            .theme(theme.to_toggle_theme())
                            .on_change({
                                let view = view.clone();
                                move |checked, _window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            state
                                                .app
                                                .measurement_state
                                                .room_eq_state
                                                .review_graph_settings
                                                .get_mut(graph_id)
                                                .normalize_to_trend = checked;
                                        });
                                        cx.notify();
                                    });
                                }
                            }),
                        ),
                )
        })
        .into_any_element()
}

/// Render a per-channel "FIR Temporal Masking" summary card.
///
/// The optimizer populates `ChannelDspChain.fir_temporal_masking` only when
/// FIR / linear-phase correction coefficients were actually exported, so the
/// card lists exactly the channels with measurable pre/post-ringing data.
/// When no channel has the metrics, the function still returns an empty card
/// stub that simply collapses to nothing — callers chain it unconditionally.
fn render_fir_temporal_masking_summary(
    dsp_output: &sotf_audio_player::room_eq_types::DspChainOutput,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let mut rows: Vec<(String, autoeq::loss::epa::score::TemporalIrMaskingMetrics)> = Vec::new();

    for (name, chain) in dsp_output.channels.iter() {
        if let Some(metrics) = chain.fir_temporal_masking.as_ref() {
            rows.push((name.clone(), metrics.clone()));
        }
    }

    if rows.is_empty() {
        return div().into_any_element();
    }

    // Stable channel ordering so the table doesn't shuffle between renders.
    rows.sort_by(|(a, _), (b, _)| {
        crate::components::room_eq::render::room_eq_channel_sort_key(a)
            .cmp(&crate::components::room_eq::render::room_eq_channel_sort_key(b))
    });

    // Compact, content-sized table that doesn't stretch full-width.
    let header_row = HStack::new()
        .spacing(StackSpacing::Sm)
        .child(Text::label("Channel"))
        .child(Text::label("Main (ms)"))
        .child(Text::label("Pre peak (dB)"))
        .child(Text::label("Post peak (dB)"))
        .child(Text::label("Pre audible (dB)"))
        .child(Text::label("Post audible (dB)"))
        .child(Text::label("Penalty"));

    let mut content = VStack::new().spacing(StackSpacing::Xs).child(header_row);

    for (name, m) in rows {
        content = content.child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new(name).size(TextSize::Sm).color(theme.text_primary))
                .child(Text::new(format!("{:.2}", m.main_time_ms)).size(TextSize::Sm))
                .child(Text::new(format!("{:.1}", m.pre_ringing_peak_db)).size(TextSize::Sm))
                .child(Text::new(format!("{:.1}", m.post_ringing_peak_db)).size(TextSize::Sm))
                .child(Text::new(format!("{:.1}", m.pre_ringing_audible_db)).size(TextSize::Sm))
                .child(Text::new(format!("{:.1}", m.post_ringing_audible_db)).size(TextSize::Sm))
                .child(Text::new(format!("{:.3}", m.penalty)).size(TextSize::Sm)),
        );
    }

    Card::new()
        .background(theme.surface)
        .header_background(theme.background_secondary)
        .border(theme.border)
        .header(
            Text::new("FIR Temporal Masking")
                .color(theme.text_primary)
                .weight(TextWeight::Semibold),
        )
        .content(content)
        .into_any_element()
}
