use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, Select, SelectOption, StackSpacing, Text,
    TextSize, TextWeight, Toggle, VStack,
};

use super::render::{
    CrossoverResponseOverlay, is_room_eq_sub_or_lfe_channel, render_channel_result_card,
};

impl PlayerView {
    pub(crate) fn render_room_eq_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let translations = state.app.ui_state.translations.clone();
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;

        let pre_score = room_eq.average_pre_score();
        let post_score = room_eq.average_post_score();
        let smoothing_octaves = room_eq.review_smoothing_octaves;
        let smoothing_dropdown_open = room_eq.dropdowns.review_smoothing_open;
        let selected_channel_idx = room_eq.review_selected_channel;
        let channel_results = room_eq.channel_results.clone();

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
        let y_axis_auto = room_eq.review_y_axis_auto;
        let normalize_to_target = room_eq.review_normalize_to_target;

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
            // Channel selection buttons
            .when(channel_results.len() > 1, |vstack| {
                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new(translations.roomeq_select_channel)
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(HStack::new().spacing(StackSpacing::Xs).children(
                            channel_results.iter().enumerate().map(|(idx, result)| {
                                let is_selected = idx == selected_channel_idx;
                                let channel_name = result.channel_name.clone();

                                Button::new(
                                    SharedString::from(format!("channel_select_{}", idx)),
                                    channel_name,
                                )
                                .variant(if is_selected {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(move |view, _, _, cx| {
                                    view.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .measurement_state
                                            .room_eq_state
                                            .review_selected_channel = idx;
                                    });
                                    cx.notify();
                                }))
                            }),
                        )),
                )
            })
            // Graph settings card
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new(translations.roomeq_graph_settings)
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new(translations.roomeq_smoothing_label)
                                            .size(TextSize::Xs)
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
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new(translations.roomeq_y_axis_auto)
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .child(
                                        Toggle::new("review_y_axis_auto")
                                            .checked(y_axis_auto)
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
                                                                .review_y_axis_auto = checked;
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
                                        Text::new(translations.roomeq_normalize_to_target)
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .child(
                                        Toggle::new("review_normalize_to_target")
                                            .checked(normalize_to_target)
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
                                                                .review_normalize_to_target =
                                                                checked;
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
                        Text::new(translations.roomeq_score_summary)
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new().spacing(StackSpacing::Xs).child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
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
            // Selected channel result
            .child(self.render_selected_channel_result(cx))
    }

    /// Render the selected channel's optimization result
    fn render_selected_channel_result(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        use crate::app::types::room_eq::{CustomTargetCurve, InteractiveChartStateWrapper};

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
                            .with_size(1200.0, 400.0),
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
        let normalize_to_target = room_eq.review_normalize_to_target;
        let chart_state = room_eq.review_chart_state.as_ref().map(|w| w.inner());

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

        // Use the backend's effective target curve (mean_spl + tilt) when available.
        // This shows what the optimizer actually aimed for instead of a misleading 0dB line.
        // Falls back to the UI-generated target curve if the backend didn't provide one.
        let is_sub_or_lfe = is_room_eq_sub_or_lfe_channel(&result.channel_name);
        let target_curve_data = if result.target_curve.is_some() {
            result.target_curve.clone()
        } else if is_sub_or_lfe {
            None
        } else if room_eq.optimizer_config.target_curve == "custom" {
            Some(room_eq.custom_target_curve.generate_curve())
        } else if room_eq.optimizer_config.target_curve == "flat" {
            Some(CustomTargetCurve::new_flat().generate_curve())
        } else {
            None
        };

        // Detect whether this channel's DSP chain contains an FIR /
        // convolution block so the filter plot can flag it in the legend.
        // We can't decompose the FIR magnitude into parametric bands, but
        // the user needs to know an FIR correction is active — otherwise
        // the "Corrected" curve will show changes that no individual IIR
        // line accounts for.
        let has_fir = room_eq
            .dsp_output
            .as_ref()
            .and_then(|out| out.channels.get(&result.channel_name))
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
        let crossover_overlay = room_eq.dsp_output.as_ref().and_then(|out| {
            build_crossover_response_overlay(
                result,
                channel_results,
                out,
                room_eq.optimizer_config.sample_rate as f64,
            )
        });

        render_channel_result_card(
            d,
            result,
            &theme,
            smoothing_octaves,
            y_axis_auto,
            normalize_to_target,
            chart_state,
            target_curve_data.as_deref(),
            has_fir,
            crossover_overlay.as_ref(),
        )
        .into_any_element()
    }
}

fn build_crossover_response_overlay(
    result: &sotf_audio_player::room_eq_types::ChannelOptResult,
    channel_results: &[sotf_audio_player::room_eq_types::ChannelOptResult],
    dsp_output: &sotf_audio_player::room_eq_types::DspChainOutput,
    sample_rate: f64,
) -> Option<CrossoverResponseOverlay> {
    let result_role = autoeq::roomeq::home_cinema::role_for_channel(&result.channel_name);
    if result_role.is_sub_or_lfe() {
        return None;
    }

    let graph = dsp_output
        .metadata
        .as_ref()?
        .bass_management
        .as_ref()?
        .routing_graph
        .as_ref()?;

    let highpass_route = matching_bass_management_route(
        &graph.routes,
        &result.channel_name,
        result_role,
        "main_highpass_to_self",
    )?;
    let lowpass_route = matching_bass_management_route(
        &graph.routes,
        &result.channel_name,
        result_role,
        "redirected_bass_lowpass_to_sub",
    )?;
    let crossover_hz = highpass_route.high_pass_hz.or(lowpass_route.low_pass_hz)?;
    let crossover_label = format!("{} {:.0}Hz", highpass_route.crossover_type, crossover_hz);

    let mut sub_channel_names = Vec::new();
    push_unique_name(&mut sub_channel_names, lowpass_route.destination.as_str());
    if let Some(post_chain_channel) = lowpass_route.post_chain_channel.as_deref() {
        push_unique_name(&mut sub_channel_names, post_chain_channel);
    }
    push_unique_name(&mut sub_channel_names, graph.physical_sub_output.as_str());
    if let Some(pre_chain_channel) = lowpass_route.pre_chain_channel.as_deref() {
        push_unique_name(&mut sub_channel_names, pre_chain_channel);
    }
    for candidate in channel_results {
        let role = autoeq::roomeq::home_cinema::role_for_channel(&candidate.channel_name);
        if role.is_sub_or_lfe() {
            push_unique_name(&mut sub_channel_names, &candidate.channel_name);
        }
    }
    for chain in dsp_output.channels.values() {
        let role = autoeq::roomeq::home_cinema::role_for_channel(&chain.channel);
        if role.is_sub_or_lfe() {
            push_unique_name(&mut sub_channel_names, &chain.channel);
        }
    }

    let sub_result = sub_channel_names
        .iter()
        .find_map(|name| {
            channel_results
                .iter()
                .find(|candidate| candidate.channel_name == *name)
        })
        .or_else(|| {
            channel_results.iter().find(|candidate| {
                let role = autoeq::roomeq::home_cinema::role_for_channel(&candidate.channel_name);
                role.is_sub_or_lfe()
            })
        });

    let sub_corrected = sub_result
        .and_then(|sub| {
            sub.corrected_response
                .as_ref()
                .or(sub.normalized_response.as_ref())
                .cloned()
        })
        .or_else(|| {
            sub_channel_names
                .iter()
                .find_map(|name| corrected_response_from_dsp_output(dsp_output, name))
        })?;

    let main_corrected = result
        .corrected_response
        .as_ref()
        .or(result.normalized_response.as_ref())?;
    let main_highpass = apply_crossover_route(main_corrected, highpass_route, sample_rate)?;
    let sub_lowpass = apply_crossover_route(&sub_corrected, lowpass_route, sample_rate)?;

    let sub_label = sub_result
        .map(|sub| sub.channel_name.clone())
        .or_else(|| sub_channel_names.first().cloned())
        .unwrap_or_else(|| "Sub".to_string());

    Some(CrossoverResponseOverlay {
        main_highpass_label: format!(
            "{} HP {} {:.0}Hz",
            result.channel_name, highpass_route.crossover_type, crossover_hz
        ),
        main_highpass,
        sub_lowpass_label: format!(
            "{sub_label} LP {} {:.0}Hz",
            lowpass_route.crossover_type, crossover_hz
        ),
        sub_lowpass,
        crossover_label,
        crossover_hz,
    })
}

fn matching_bass_management_route<'a>(
    routes: &'a [autoeq::roomeq::BassManagementRoute],
    channel_name: &str,
    channel_role: autoeq::roomeq::HomeCinemaRole,
    route_kind: &str,
) -> Option<&'a autoeq::roomeq::BassManagementRoute> {
    routes
        .iter()
        .find(|route| route.route_kind == route_kind && route.source_channel == channel_name)
        .or_else(|| {
            routes.iter().find(|route| {
                route.route_kind == route_kind
                    && autoeq::roomeq::home_cinema::role_for_channel(&route.source_channel)
                        == channel_role
            })
        })
}

fn apply_crossover_route(
    points: &[(f64, f64)],
    route: &autoeq::roomeq::BassManagementRoute,
    sample_rate: f64,
) -> Option<Vec<(f64, f64)>> {
    let crossover_hz = route.low_pass_hz.or(route.high_pass_hz)?;
    let filters = create_crossover_filters(
        &route.crossover_type,
        crossover_hz,
        sample_rate,
        route.low_pass_hz.is_some(),
    );
    if filters.is_empty() {
        return None;
    }

    let freqs = ndarray::Array1::from_iter(points.iter().map(|(f, _)| *f));
    let response = autoeq::response::compute_peq_complex_response(&filters, &freqs, sample_rate);
    let routed: Vec<(f64, f64)> = points
        .iter()
        .zip(response.iter())
        .filter_map(|(&(freq, db), h)| {
            if freq.is_finite() && freq > 0.0 && db.is_finite() {
                let filter_db = 20.0 * h.norm().max(1.0e-12).log10();
                Some((freq, db + route.gain_db + filter_db))
            } else {
                None
            }
        })
        .collect();
    (!routed.is_empty()).then_some(routed)
}

fn create_crossover_filters(
    type_str: &str,
    freq: f64,
    sample_rate: f64,
    is_lowpass: bool,
) -> Vec<math_audio_iir_fir::Biquad> {
    use math_audio_iir_fir::*;

    let peq = match type_str.to_lowercase().as_str() {
        "lr24" | "lr4" => {
            if is_lowpass {
                peq_linkwitzriley_lowpass(4, freq, sample_rate)
            } else {
                peq_linkwitzriley_highpass(4, freq, sample_rate)
            }
        }
        "lr48" | "lr8" => {
            if is_lowpass {
                peq_linkwitzriley_lowpass(8, freq, sample_rate)
            } else {
                peq_linkwitzriley_highpass(8, freq, sample_rate)
            }
        }
        "bw12" | "butterworth12" => {
            if is_lowpass {
                peq_butterworth_lowpass(2, freq, sample_rate)
            } else {
                peq_butterworth_highpass(2, freq, sample_rate)
            }
        }
        "bw24" | "butterworth24" => {
            if is_lowpass {
                peq_butterworth_lowpass(4, freq, sample_rate)
            } else {
                peq_butterworth_highpass(4, freq, sample_rate)
            }
        }
        _ => {
            if is_lowpass {
                peq_linkwitzriley_lowpass(4, freq, sample_rate)
            } else {
                peq_linkwitzriley_highpass(4, freq, sample_rate)
            }
        }
    };
    peq.into_iter().map(|(_, b)| b).collect()
}

fn push_unique_name(names: &mut Vec<String>, name: &str) {
    if !name.is_empty() && !names.iter().any(|existing| existing == name) {
        names.push(name.to_string());
    }
}

fn corrected_response_from_dsp_output(
    dsp_output: &sotf_audio_player::room_eq_types::DspChainOutput,
    channel_name: &str,
) -> Option<Vec<(f64, f64)>> {
    let chain = dsp_output.channels.get(channel_name).or_else(|| {
        dsp_output
            .channels
            .values()
            .find(|chain| chain.channel == channel_name)
    })?;
    let curve = chain.final_curve.as_ref()?;
    let points: Vec<(f64, f64)> = curve
        .freq
        .iter()
        .zip(curve.spl.iter())
        .filter(|(_, db)| db.is_finite() && **db > -150.0)
        .map(|(&f, &db)| (f, db))
        .collect();
    (!points.is_empty()).then_some(points)
}
