use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, StackSpacing, Text, TextSize, TextWeight, VStack,
};
use sotf_audio::signal_analysis as dsp;

/// Interpolate a target curve (control points) at the given frequency values using log-frequency interpolation
fn interpolate_target_at_frequencies(frequencies: &[f64], target: &[(f64, f64)]) -> Vec<f64> {
    frequencies
        .iter()
        .map(|&f| {
            let mut lower = (20.0, 0.0);
            let mut upper = (20000.0, 0.0);
            if let Some(first) = target.first()
                && f < first.0
            {
                return first.1;
            }
            if let Some(last) = target.last()
                && f > last.0
            {
                return last.1;
            }
            for win in target.windows(2) {
                if f >= win[0].0 && f <= win[1].0 {
                    lower = win[0];
                    upper = win[1];
                    break;
                }
            }
            let denom = upper.0.ln() - lower.0.ln();
            if denom.abs() < 1e-12 {
                return lower.1;
            }
            let t = (f.ln() - lower.0.ln()) / denom;
            let result = lower.1 + t * (upper.1 - lower.1);
            if result.is_finite() { result } else { 0.0 }
        })
        .collect()
}

// === Free functions for channel configuration UI ===

/// Render a single channel configuration row
pub(crate) fn render_channel_config_row(
    idx: usize,
    config: &crate::app::types::RoomEqSpeakerConfig,
    theme: &crate::theme::Theme,
    view: &Entity<PlayerView>,
    d: Ds,
) -> impl IntoElement {
    use crate::app::types::SpeakerConfigType;

    let channel_name = config.channel_name.clone();
    let is_multi = config.config_type == SpeakerConfigType::MultiDriver;
    let crossover_type = config.crossover_type;

    div()
        .flex()
        .gap(d.section)
        .items_center()
        .w_full()
        .p(d.pad_x)
        .bg(theme.surface)
        .rounded(d.r_lg)
        .border_1()
        .border_color(theme.border)
        // Channel name
        .child(
            div().w(rems(5.0)).child(
                Text::new(channel_name)
                    .weight(TextWeight::Bold)
                    .color(theme.text_primary),
            ),
        )
        // Speaker type toggle
        .child(
            div()
                .flex()
                .gap(d.gap)
                .items_center()
                .child(
                    Text::new("Type:")
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                )
                .child(
                    Button::new(SharedString::from(format!("single-{}", idx)), "Single")
                        .variant(if !is_multi {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .on_click({
                            let view = view.clone();
                            move |_, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        if let Some(cfg) = state
                                            .app
                                            .measurement_state
                                            .room_eq_state
                                            .speaker_configs
                                            .get_mut(idx)
                                        {
                                            cfg.config_type = SpeakerConfigType::Single;
                                        }
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    Button::new(SharedString::from(format!("multi-{}", idx)), "Multi-Driver")
                        .variant(if is_multi {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .on_click({
                            let view = view.clone();
                            move |_, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        if let Some(cfg) = state
                                            .app
                                            .measurement_state
                                            .room_eq_state
                                            .speaker_configs
                                            .get_mut(idx)
                                        {
                                            cfg.config_type = SpeakerConfigType::MultiDriver;
                                        }
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                ),
        )
        // Crossover type selector (only shown for multi-driver)
        .when(is_multi, |el| {
            el.child(
                div()
                    .flex()
                    .gap(d.gap)
                    .items_center()
                    .child(
                        Text::new("Crossover:")
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    .child(render_crossover_dropdown(idx, crossover_type, view, theme)),
            )
        })
}

/// Render crossover type dropdown as a cycling button
fn render_crossover_dropdown(
    channel_idx: usize,
    current: crate::app::types::CrossoverType,
    view: &Entity<PlayerView>,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::app::types::CrossoverType;

    let crossover_types = CrossoverType::all();
    let current_label = current.as_str();

    Button::new(
        SharedString::from(format!("xover-{}", channel_idx)),
        current_label,
    )
    .variant(ButtonVariant::Secondary)
    .size(ButtonSize::Xs)
    .theme(theme.to_button_theme())
    .on_click({
        let view = view.clone();
        let crossover_types = crossover_types.to_vec();
        move |_, cx| {
            view.update(cx, |this, cx| {
                this.state.update(cx, |state, _| {
                    if let Some(cfg) = state
                        .app
                        .measurement_state
                        .room_eq_state
                        .speaker_configs
                        .get_mut(channel_idx)
                    {
                        // Find current index and cycle to next
                        let current_idx = crossover_types
                            .iter()
                            .position(|&ct| ct == cfg.crossover_type)
                            .unwrap_or(0);
                        let next_idx = (current_idx + 1) % crossover_types.len();
                        cfg.crossover_type = crossover_types[next_idx];
                    }
                });
                cx.notify();
            });
        }
    })
}

// === Review Step UI Free Functions ===

/// Render a single channel result card with plots and filter details
/// If interactive_state is provided, the chart will support pan/zoom interactions
pub(crate) fn render_channel_result_card(
    d: Ds,
    result: &crate::app::types::ChannelOptResult,
    theme: &crate::theme::Theme,
    smoothing_octaves: f64,
    y_axis_auto: bool,
    normalize_to_target: bool,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
    target_curve: Option<&[(f64, f64)]>,
) -> impl IntoElement {
    use crate::components::graphs::format_frequency;

    let channel_name = result.channel_name.clone();
    let score_improvement = result.pre_score - result.post_score;
    // Use normalized_response as the primary display (level-normalized optimized result)
    let has_response_data =
        result.original_response.is_some() && result.normalized_response.is_some();

    div()
        .flex()
        .flex_col()
        .gap(d.gap_md)
        .p(d.card)
        .w_full()
        .bg(theme.surface)
        .rounded(d.r_lg)
        .border_1()
        .border_color(theme.border)
        // Header with channel name and scores
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    Text::new(channel_name)
                        .weight(TextWeight::Bold)
                        .size(TextSize::Md)
                        .color(theme.text_primary),
                )
                .child(
                    div()
                        .flex()
                        .gap(d.section)
                        .child(
                            Text::new(format!("Before: {:.2}", result.pre_score))
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(format!("After: {:.2}", result.post_score))
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(format!("{:+.2}", score_improvement))
                                .weight(TextWeight::Bold)
                                .color(if score_improvement > 0.0 {
                                    theme.success
                                } else {
                                    theme.error
                                }),
                        ),
                ),
        )
        // Filter plot: each filter and the sum (if available)
        .when(has_response_data && !result.eq_filters.is_empty(), |div| {
            let original = result.original_response.as_ref().unwrap();
            let normalized = result.normalized_response.as_ref().unwrap();
            div.child(render_filter_plot(
                original,
                normalized,
                &result.eq_filters,
                theme,
                smoothing_octaves,
                y_axis_auto,
                interactive_state,
            ))
        })
        // Original vs Corrected with trendlines (if available)
        .when(has_response_data, |div| {
            let original = result.original_response.as_ref().unwrap();
            let normalized = result.normalized_response.as_ref().unwrap();
            div.child(render_response_comparison_graph(
                original,
                normalized,
                theme,
                smoothing_octaves,
                y_axis_auto,
                normalize_to_target,
                interactive_state,
                target_curve,
            ))
        })
        // Histogram (if trend data available)
        .when(
            result.group_delay_before.is_some() || result.group_delay_after.is_some(),
            |div| {
                let original = result.original_response.as_ref().unwrap();
                let normalized = result.normalized_response.as_ref().unwrap();
                div.child(render_tonal_histogram(
                    original,
                    normalized,
                    theme,
                    smoothing_octaves,
                ))
            },
        )
        // Phase response plot (if phase data available)
        .when(
            result.phase_response_before.is_some() || result.phase_response_after.is_some(),
            |div| {
                div.child(render_phase_graph(
                    result.phase_response_before.as_deref(),
                    result.phase_response_after.as_deref(),
                    theme,
                ))
            },
        )
        // Group delay graph (if phase data available)
        .when(
            result.group_delay_before.is_some() || result.group_delay_after.is_some(),
            |div| {
                div.child(render_group_delay_graph(
                    result.group_delay_before.as_deref(),
                    result.group_delay_after.as_deref(),
                    theme,
                ))
            },
        )
        // Impulse response plot (if IR data available)
        .when(result.impulse_response.is_some(), |div| {
            div.child(render_impulse_response_graph(
                result.impulse_response.as_ref().unwrap(),
                theme,
            ))
        })
        // EQ Filter details
        .child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new("EQ Filters")
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Xs)
                        .color(theme.text_primary),
                )
                .child(render_filter_table(d, &result.eq_filters, theme)),
        )
        // Crossover info (if multi-driver)
        .when(result.crossover_freqs.is_some(), |el| {
            let xover_freqs = result.crossover_freqs.as_ref().unwrap();
            el.child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Text::new("Crossover Frequencies")
                            .weight(TextWeight::Semibold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    )
                    .child(
                        gpui::div()
                            .flex()
                            .gap(d.gap)
                            .children(xover_freqs.iter().map(|f| {
                                gpui::div()
                                    .px(d.pad_y)
                                    .py(d.pad_y_half)
                                    .bg(theme.background_secondary)
                                    .rounded(d.r_md)
                                    .child(
                                        Text::new(format_frequency(*f))
                                            .size(TextSize::Xs)
                                            .color(theme.text_primary),
                                    )
                            })),
                    ),
            )
        })
}

/// Render the frequency response comparison graph: Original vs Corrected with trendlines
/// If interactive_state is provided, the chart will support pan/zoom interactions
fn render_response_comparison_graph(
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    theme: &crate::theme::Theme,
    smoothing_octaves: f64,
    y_axis_auto: bool,
    normalize_to_target: bool,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
    target_curve: Option<&[(f64, f64)]>,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};

    const GRAPH_WIDTH: f32 = 1200.0;
    const GRAPH_HEIGHT: f32 = 400.0;

    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;
    const RED: u32 = 0xd62728;

    let frequencies: Vec<f64> = original.iter().map(|(f, _)| *f).collect();
    let original_values_raw: Vec<f64> = original.iter().map(|(_, db)| *db).collect();
    let corrected_values_raw: Vec<f64> = corrected.iter().map(|(_, db)| *db).collect();

    let offset = crate::app::types::RoomEqState::calculate_normalization_offset(
        &frequencies,
        &original_values_raw,
    );
    let mut original_values: Vec<f64> = original_values_raw.iter().map(|&db| db - offset).collect();
    let mut corrected_values: Vec<f64> =
        corrected_values_raw.iter().map(|&db| db - offset).collect();

    // When normalizing to target, subtract the interpolated target curve from all series
    // so the target becomes a flat 0dB reference line and deviations are clearly visible
    let target_interpolated =
        target_curve.map(|target| interpolate_target_at_frequencies(&frequencies, target));

    if normalize_to_target {
        if let Some(ref target_vals) = target_interpolated {
            // Normalize target with same 1-2kHz method, then subtract from
            // original/corrected (which already had their own offset subtracted)
            let target_offset = crate::app::types::RoomEqState::calculate_normalization_offset(
                &frequencies,
                target_vals,
            );
            for (i, v) in original_values.iter_mut().enumerate() {
                *v -= target_vals[i] - target_offset;
            }
            for (i, v) in corrected_values.iter_mut().enumerate() {
                *v -= target_vals[i] - target_offset;
            }
        }
    }

    let original_smooth =
        dsp::smooth_response_f64(&frequencies, &original_values, smoothing_octaves);
    let corrected_smooth =
        dsp::smooth_response_f64(&frequencies, &corrected_values, smoothing_octaves);

    let sanitize = |v: &[f64]| -> Vec<f64> {
        v.iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .collect()
    };
    let original_smooth = sanitize(&original_smooth);
    let corrected_smooth = sanitize(&corrected_smooth);

    let mean_spl = if !original_smooth.is_empty() {
        original_smooth.iter().sum::<f64>() / original_smooth.len() as f64
    } else {
        0.0
    };

    let (y_min_auto, y_max_auto) = {
        let mut min_val = f64::INFINITY;
        let mut max_val = f64::NEG_INFINITY;
        for &v in original_smooth.iter().chain(corrected_smooth.iter()) {
            min_val = min_val.min(v);
            max_val = max_val.max(v);
        }
        let max = if max_val.is_finite() {
            ((max_val / 5.0).ceil() * 5.0).max(5.0)
        } else {
            5.0
        };
        let min = if min_val.is_finite() {
            (min_val / 5.0).floor() * 5.0
        } else {
            -15.0
        };
        (min, max)
    };

    let (y_min_fixed, y_max_fixed) = if mean_spl > 30.0 {
        (mean_spl - 40.0, mean_spl + 10.0)
    } else {
        (-40.0, 10.0)
    };

    let (y_min, y_max) = if y_axis_auto {
        (y_min_auto, y_max_auto)
    } else {
        (y_min_fixed, y_max_fixed)
    };

    if frequencies.is_empty() {
        return div()
            .child(
                Text::new("No data available")
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            )
            .into_any_element();
    }

    let chart_theme = theme_to_chart_theme(theme);

    let calculate_trend = |freqs: &[f64], values: &[f64]| -> Option<(f64, f64)> {
        let min_freq = 100.0;
        let max_freq = 10000.0;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_xx = 0.0;
        let mut count = 0.0;

        for (i, &f) in freqs.iter().enumerate() {
            if f >= min_freq
                && f <= max_freq
                && let Some(&y) = values.get(i)
            {
                let x = f.log10();
                sum_x += x;
                sum_y += y;
                sum_xy += x * y;
                sum_xx += x * x;
                count += 1.0;
            }
        }

        if count < 2.0 {
            return None;
        }

        let mean_x = sum_x / count;
        let mean_y = sum_y / count;
        let denominator = sum_xx - count * mean_x * mean_x;
        if denominator.abs() < 1e-10 {
            return None;
        }

        let slope = (sum_xy - count * mean_x * mean_y) / denominator;
        let intercept = mean_y - slope * mean_x;
        Some((slope, intercept))
    };

    let orig_trend = calculate_trend(&frequencies, &original_smooth);
    let corr_trend = calculate_trend(&frequencies, &corrected_smooth);

    let (x_min, x_max) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.x_domain())
        .unwrap_or((20.0, 20000.0));
    let (y_min_domain, y_max_domain) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.y_domain())
        .unwrap_or((y_min, y_max));

    let y_min_domain = if y_min_domain.is_finite() {
        y_min_domain
    } else {
        -15.0
    };
    let y_max_domain = if y_max_domain.is_finite() {
        y_max_domain
    } else {
        5.0
    };
    let y_max_domain = if y_max_domain <= y_min_domain {
        y_min_domain + 1.0
    } else {
        y_max_domain
    };

    let mut chart_builder = line(&frequencies, &original_smooth)
        .x_scale(ScaleType::Log)
        .x_range(x_min, x_max)
        .y_range(y_min_domain, y_max_domain)
        .y_label(if normalize_to_target && target_interpolated.is_some() {
            "Deviation from Target (dB)"
        } else {
            "SPL (dB)"
        })
        .label("Original")
        .legend_position(LegendPosition::Bottom)
        .color(BLUE)
        .stroke_width(2.0)
        .opacity(1.0)
        .theme(chart_theme.clone())
        .size(GRAPH_WIDTH, GRAPH_HEIGHT)
        .add_series(&corrected_smooth, Some("Corrected"), ORANGE, 2.0, 1.0);

    if target_curve.is_some() {
        if normalize_to_target {
            // Target is now 0dB — draw a flat reference line
            let flat_target: Vec<f64> = vec![0.0; frequencies.len()];
            chart_builder =
                chart_builder.add_series(&flat_target, Some("Target (0 dB)"), RED, 1.5, 0.6);
        } else if let Some(ref target_vals) = target_interpolated {
            // Normalize target using same method (1-2kHz band mean) so it aligns
            // with original/corrected at the reference frequency range
            let target_offset = crate::app::types::RoomEqState::calculate_normalization_offset(
                &frequencies,
                target_vals,
            );
            let relative_target: Vec<f64> = target_vals.iter().map(|v| v - target_offset).collect();
            chart_builder =
                chart_builder.add_series(&relative_target, Some("Target"), RED, 2.0, 0.8);
        }
    }

    if let Some((slope, intercept)) = orig_trend {
        let trend: Vec<f64> = frequencies
            .iter()
            .map(|f| slope * f.log10() + intercept)
            .collect();
        chart_builder = chart_builder.add_series(
            &trend,
            Some(&format!("{:.2} dB/dec", slope)),
            BLUE,
            1.5,
            0.6,
        );
    }

    if let Some((slope, intercept)) = corr_trend {
        let trend: Vec<f64> = frequencies
            .iter()
            .map(|f| slope * f.log10() + intercept)
            .collect();
        chart_builder = chart_builder.add_series(
            &trend,
            Some(&format!("{:.2} dB/dec", slope)),
            ORANGE,
            1.5,
            0.6,
        );
    }

    let line_chart = chart_builder.build();

    let chart_element: Option<gpui::AnyElement> = line_chart.ok().map(|chart| {
        if let Some(state) = interactive_state {
            gpui_px::interaction::interactive("room-eq-response-chart", chart, state.clone())
                .build()
                .into_any_element()
        } else {
            chart.into_any_element()
        }
    });

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new(if normalize_to_target && target_interpolated.is_some() {
                "Original vs Corrected (Normalized to Target)"
            } else {
                "Original vs Corrected"
            })
            .weight(TextWeight::Semibold)
            .size(TextSize::Xs)
            .color(theme.text_primary),
        )
        .when_some(chart_element, |el, c| el.child(c))
        .into_any_element()
}

/// Render the filter plot showing each individual filter and their combined response
fn render_filter_plot(
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    eq_filters: &[crate::app::types::EqFilterConfig],
    theme: &crate::theme::Theme,
    _smoothing_octaves: f64,
    y_axis_auto: bool,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};
    use math_audio_iir_fir::{Biquad, BiquadFilterType};

    const GRAPH_WIDTH: f32 = 1200.0;
    const GRAPH_HEIGHT: f32 = 400.0;
    const SAMPLE_RATE: f64 = sotf_plugins::DEFAULT_PREVIEW_SAMPLE_RATE;

    const BLUE: u32 = 0x1f77b4;
    const GREEN: u32 = 0x2ca02c;
    const PURPLE: u32 = 0x9467bd;
    const CYAN: u32 = 0x17becf;
    const MAGENTA: u32 = 0xd62728;

    let frequencies: Vec<f64> = original.iter().map(|(f, _)| *f).collect();
    let original_values_raw: Vec<f64> = original.iter().map(|(_, db)| *db).collect();
    let corrected_values_raw: Vec<f64> = corrected.iter().map(|(_, db)| *db).collect();

    let offset = crate::app::types::RoomEqState::calculate_normalization_offset(
        &frequencies,
        &original_values_raw,
    );
    let _corrected_normalized: Vec<f64> =
        corrected_values_raw.iter().map(|&db| db - offset).collect();

    if frequencies.is_empty() || eq_filters.is_empty() {
        return div()
            .child(
                Text::new("No filter data available")
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            )
            .into_any_element();
    }

    let chart_theme = theme_to_chart_theme(theme);

    let filter_colors = [
        BLUE,
        GREEN,
        PURPLE,
        CYAN,
        MAGENTA,
        0x8c564bu32,
        0xe377c2u32,
        0x7f7f7fu32,
        0xbcbd22u32,
        0x1f77b4u32,
    ];

    let mut chart_builder = line(&frequencies, &vec![0.0; frequencies.len()])
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_range(-12.0, 6.0)
        .y_label("EQ (dB)")
        .label("Sum")
        .legend_position(LegendPosition::Bottom)
        .color(GREEN)
        .stroke_width(2.0)
        .opacity(1.0)
        .theme(chart_theme.clone())
        .size(GRAPH_WIDTH, GRAPH_HEIGHT);

    let eq_response: Vec<f64> = frequencies
        .iter()
        .map(|&freq| {
            eq_filters
                .iter()
                .map(|f| {
                    let filter_type = match f.filter_type.as_str() {
                        "peak" | "pk" => BiquadFilterType::Peak,
                        "lowshelf" | "ls" => BiquadFilterType::Lowshelf,
                        "highshelf" | "hs" => BiquadFilterType::Highshelf,
                        "lowpass" | "lp" => BiquadFilterType::Lowpass,
                        "highpass" | "hp" => BiquadFilterType::Highpass,
                        _ => BiquadFilterType::Peak,
                    };
                    let biquad = Biquad::new(filter_type, f.frequency, SAMPLE_RATE, f.q, f.gain_db);
                    biquad.log_result(freq)
                })
                .sum::<f64>()
        })
        .collect();

    let sanitize = |v: &[f64]| -> Vec<f64> {
        v.iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .collect()
    };
    let eq_response = sanitize(&eq_response);

    chart_builder = chart_builder.add_series(&eq_response, Some("Sum"), GREEN, 2.0, 1.0);

    for (i, filter) in eq_filters.iter().enumerate() {
        let filter_response: Vec<f64> = frequencies
            .iter()
            .map(|&freq| {
                let filter_type = match filter.filter_type.as_str() {
                    "peak" | "pk" => BiquadFilterType::Peak,
                    "lowshelf" | "ls" => BiquadFilterType::Lowshelf,
                    "highshelf" | "hs" => BiquadFilterType::Highshelf,
                    "lowpass" | "lp" => BiquadFilterType::Lowpass,
                    "highpass" | "hp" => BiquadFilterType::Highpass,
                    _ => BiquadFilterType::Peak,
                };
                let biquad = Biquad::new(
                    filter_type,
                    filter.frequency,
                    SAMPLE_RATE,
                    filter.q,
                    filter.gain_db,
                );
                biquad.log_result(freq)
            })
            .collect();
        let filter_response = sanitize(&filter_response);
        let color = filter_colors[i % filter_colors.len()];
        let label = format!(
            "F{} {} {:.0}Hz",
            i + 1,
            filter.filter_type,
            filter.frequency
        );
        chart_builder = chart_builder.add_series(&filter_response, Some(&label), color, 1.5, 0.7);
    }

    let (x_min, x_max) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.x_domain())
        .unwrap_or((20.0, 20000.0));
    let (y_min, y_max) = if y_axis_auto {
        let mut min_val = f64::INFINITY;
        let mut max_val = f64::NEG_INFINITY;
        for &v in eq_response.iter() {
            min_val = min_val.min(v);
            max_val = max_val.max(v);
        }
        for filter in eq_filters.iter() {
            let _ = filter;
        }
        let max = if max_val.is_finite() {
            ((max_val / 2.0).ceil() * 2.0).max(6.0)
        } else {
            6.0
        };
        let min = if min_val.is_finite() {
            (min_val / 2.0).floor() * 2.0
        } else {
            -12.0
        };
        (min, max)
    } else {
        (-12.0, 6.0)
    };

    let line_chart = chart_builder
        .x_range(x_min, x_max)
        .y_range(y_min, y_max)
        .build();

    let chart_element: Option<gpui::AnyElement> = line_chart.ok().map(|chart| {
        if let Some(state) = interactive_state {
            gpui_px::interaction::interactive("room-eq-filter-chart", chart, state.clone())
                .build()
                .into_any_element()
        } else {
            chart.into_any_element()
        }
    });

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("EQ Filters")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(chart_element, |el, c| el.child(c))
        .into_any_element()
}

/// Render the tonal balance histogram
fn render_tonal_histogram(
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    theme: &crate::theme::Theme,
    smoothing_octaves: f64,
) -> impl IntoElement {
    use gpui_px::{BarTheme, LegendPosition, bar};

    const GRAPH_WIDTH: f32 = 1200.0;
    const GRAPH_HEIGHT: f32 = 200.0;

    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;

    let frequencies: Vec<f64> = original.iter().map(|(f, _)| *f).collect();
    let original_values_raw: Vec<f64> = original.iter().map(|(_, db)| *db).collect();
    let corrected_values_raw: Vec<f64> = corrected.iter().map(|(_, db)| *db).collect();

    let offset = crate::app::types::RoomEqState::calculate_normalization_offset(
        &frequencies,
        &original_values_raw,
    );
    let original_values: Vec<f64> = original_values_raw.iter().map(|&db| db - offset).collect();
    let corrected_values: Vec<f64> = corrected_values_raw.iter().map(|&db| db - offset).collect();

    let original_smooth =
        dsp::smooth_response_f64(&frequencies, &original_values, smoothing_octaves);
    let corrected_smooth =
        dsp::smooth_response_f64(&frequencies, &corrected_values, smoothing_octaves);

    let sanitize = |v: &[f64]| -> Vec<f64> {
        v.iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .collect()
    };
    let original_smooth = sanitize(&original_smooth);
    let corrected_smooth = sanitize(&corrected_smooth);

    let calculate_trend = |freqs: &[f64], values: &[f64]| -> Option<(f64, f64)> {
        let min_freq = 100.0;
        let max_freq = 10000.0;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_xx = 0.0;
        let mut count = 0.0;

        for (i, &f) in freqs.iter().enumerate() {
            if f >= min_freq
                && f <= max_freq
                && let Some(&y) = values.get(i)
            {
                let x = f.log10();
                sum_x += x;
                sum_y += y;
                sum_xy += x * y;
                sum_xx += x * x;
                count += 1.0;
            }
        }

        if count < 2.0 {
            return None;
        }

        let mean_x = sum_x / count;
        let mean_y = sum_y / count;
        let denominator = sum_xx - count * mean_x * mean_x;
        if denominator.abs() < 1e-10 {
            return None;
        }

        let slope = (sum_xy - count * mean_x * mean_y) / denominator;
        let intercept = mean_y - slope * mean_x;
        Some((slope, intercept))
    };

    let orig_trend = calculate_trend(&frequencies, &original_smooth);
    let corr_trend = calculate_trend(&frequencies, &corrected_smooth);

    let hist_chart = if let (Some((slope_orig, int_orig)), Some((slope_corr, int_corr))) =
        (orig_trend, corr_trend)
    {
        let calculate_histogram =
            |freqs: &[f64], values: &[f64], slope: f64, intercept: f64| -> Vec<f64> {
                let min_freq = 100.0;
                let max_freq = 10000.0;
                let mut bins = vec![0.0; 9];

                for (i, &f) in freqs.iter().enumerate() {
                    if f >= min_freq
                        && f <= max_freq
                        && let Some(&y) = values.get(i)
                    {
                        let trend_y = slope * f.log10() + intercept;
                        let deviation = (y - trend_y).abs();

                        let bin_idx = (deviation / 0.5).floor() as usize;
                        if bin_idx < 8 {
                            bins[bin_idx] += 1.0;
                        } else {
                            bins[8] += 1.0;
                        }
                    }
                }
                bins
            };

        let hist_orig = calculate_histogram(&frequencies, &original_smooth, slope_orig, int_orig);
        let hist_corr = calculate_histogram(&frequencies, &corrected_smooth, slope_corr, int_corr);

        let labels = vec![
            "0-0.5", "0.5-1", "1-1.5", "1.5-2", "2-2.5", "2.5-3", "3-3.5", "3.5-4", ">4",
        ];

        let bar_theme = BarTheme {
            plot_background: theme.surface,
            title_color: theme.text_primary,
            legend_text_color: theme.text_secondary,
        };

        bar(&labels, &hist_orig)
            .color(BLUE)
            .label("Original")
            .theme(bar_theme)
            .size(GRAPH_WIDTH, GRAPH_HEIGHT)
            .bar_gap(4.0)
            .opacity(0.8)
            .legend_position(LegendPosition::Bottom)
            .add_series(&hist_corr, Some("Corrected"), ORANGE, 0.8)
            .build()
            .ok()
    } else {
        None
    };

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Tonal Balance")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(hist_chart, |el, c| el.child(c))
        .into_any_element()
}

/// Render the phase response graph
fn render_phase_graph(
    phase_before: Option<&[(f64, f64)]>,
    phase_after: Option<&[(f64, f64)]>,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};

    const GRAPH_WIDTH: f32 = 1200.0;
    const GRAPH_HEIGHT: f32 = 200.0;

    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;

    let chart_theme = theme_to_chart_theme(theme);

    let reference = phase_before.or(phase_after).unwrap();
    let frequencies: Vec<f64> = reference.iter().map(|(f, _)| *f).collect();

    let in_range = |f: f64| (20.0..=20000.0).contains(&f);

    let before_values: Option<Vec<f64>> = phase_before.map(|b| b.iter().map(|(_, p)| *p).collect());

    let after_values: Option<Vec<f64>> = phase_after.map(|after| {
        frequencies
            .iter()
            .map(|&f| {
                if let Some(pos) = after.windows(2).position(|w| w[0].0 <= f && f <= w[1].0) {
                    let (f0, p0) = after[pos];
                    let (f1, p1) = after[pos + 1];
                    let t = if (f1 - f0).abs() > 1e-12 {
                        (f - f0) / (f1 - f0)
                    } else {
                        0.0
                    };
                    p0 + t * (p1 - p0)
                } else {
                    after.last().map(|(_, p)| *p).unwrap_or(0.0)
                }
            })
            .collect()
    });

    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for (i, &f) in frequencies.iter().enumerate() {
        if in_range(f) {
            for vals in [&before_values, &after_values].into_iter().flatten() {
                if let Some(&v) = vals.get(i)
                    && v.is_finite()
                {
                    y_min = y_min.min(v);
                    y_max = y_max.max(v);
                }
            }
        }
    }
    if !y_min.is_finite() || !y_max.is_finite() || y_min >= y_max {
        y_min = -std::f64::consts::PI;
        y_max = std::f64::consts::PI;
    }
    let margin = (y_max - y_min).max(1.0) * 0.1;
    y_min = (y_min - margin).floor();
    y_max = (y_max + margin).ceil();

    let (primary_values, primary_label, primary_color) = if let Some(ref bv) = before_values {
        (bv.as_slice(), "Before", BLUE)
    } else if let Some(ref av) = after_values {
        (av.as_slice(), "After", ORANGE)
    } else {
        return div().into_any_element();
    };

    let mut chart_builder = line(&frequencies, primary_values)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_range(y_min, y_max)
        .y_label("Phase (rad)")
        .label(primary_label)
        .legend_position(LegendPosition::Bottom)
        .color(primary_color)
        .stroke_width(1.5)
        .opacity(0.7)
        .theme(chart_theme)
        .size(GRAPH_WIDTH, GRAPH_HEIGHT);

    if before_values.is_some()
        && let Some(ref av) = after_values
    {
        chart_builder = chart_builder.add_series(av, Some("After"), ORANGE, 1.5, 0.9);
    }

    let chart = match chart_builder.build() {
        Ok(c) => c,
        Err(_) => {
            return div()
                .child(
                    Text::new("Phase: chart error")
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                )
                .into_any_element();
        }
    };

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Phase Response")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .child(chart.into_any_element())
        .into_any_element()
}

/// Render the impulse response graph
fn render_impulse_response_graph(
    impulse_response: &[(f64, f64)],
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};

    const GRAPH_WIDTH: f32 = 1200.0;
    const GRAPH_HEIGHT: f32 = 200.0;

    const BLUE: u32 = 0x1f77b4;

    let chart_theme = theme_to_chart_theme(theme);

    let samples: Vec<f64> = impulse_response.iter().map(|(s, _)| *s).collect();
    let amplitudes: Vec<f64> = impulse_response.iter().map(|(_, a)| *a).collect();

    let sanitize: Vec<f64> = amplitudes
        .iter()
        .map(|&x| if x.is_finite() { x } else { 0.0 })
        .collect();

    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for &v in sanitize.iter() {
        y_min = y_min.min(v);
        y_max = y_max.max(v);
    }
    if !y_min.is_finite() || !y_max.is_finite() || y_min >= y_max {
        y_min = -1.0;
        y_max = 1.0;
    }
    let margin = (y_max - y_min).max(1.0) * 0.1;
    y_min -= margin;
    y_max += margin;

    let chart = match line(&samples, &sanitize)
        .x_scale(ScaleType::Linear)
        .x_range(
            samples.first().copied().unwrap_or(0.0),
            samples.last().copied().unwrap_or(1.0),
        )
        .y_range(y_min, y_max)
        .y_label("Amplitude")
        .label("IR")
        .legend_position(LegendPosition::Bottom)
        .color(BLUE)
        .stroke_width(1.5)
        .opacity(1.0)
        .theme(chart_theme)
        .size(GRAPH_WIDTH, GRAPH_HEIGHT)
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return div()
                .child(
                    Text::new("IR: chart error")
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                )
                .into_any_element();
        }
    };

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Impulse Response")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .child(chart.into_any_element())
        .into_any_element()
}

/// Render the EQ filter table
fn render_filter_table(
    d: Ds,
    filters: &[crate::app::types::EqFilterConfig],
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::format_frequency;

    if filters.is_empty() {
        return div()
            .child(
                Text::new("No filters")
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            )
            .into_any_element();
    }

    div()
        .flex()
        .flex_wrap()
        .justify_center()
        .gap(d.gap)
        .children(filters.iter().enumerate().map(|(i, f)| {
            let gain_color = if f.gain_db > 0.5 {
                theme.success
            } else if f.gain_db < -0.5 {
                theme.error
            } else {
                theme.text_muted
            };

            div()
                .px(d.pad_x)
                .py(d.pad_y)
                .bg(theme.background_secondary)
                .rounded(d.r_md)
                .border_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap(d.grid)
                .min_w(rems(5.0))
                // Filter number and type
                .child(
                    div()
                        .flex()
                        .gap(d.grid)
                        .items_center()
                        .child(
                            Text::new(format!("{}", i + 1))
                                .weight(TextWeight::Bold)
                                .size(TextSize::Xs)
                                .color(theme.text_primary),
                        )
                        .child(
                            Text::new(&f.filter_type)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
                // Frequency
                .child(
                    Text::new(format_frequency(f.frequency))
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Xs)
                        .color(theme.text_primary),
                )
                // Gain and Q
                .child(
                    div()
                        .flex()
                        .gap(d.gap)
                        .child(
                            Text::new(format!("{:+.1}dB", f.gain_db))
                                .weight(TextWeight::Bold)
                                .size(TextSize::Xs)
                                .color(gain_color),
                        )
                        .child(
                            Text::new(format!("Q:{:.1}", f.q))
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        ),
                )
        }))
        .into_any_element()
}

/// Render group delay comparison graph (before vs after optimization)
fn render_group_delay_graph(
    gd_before: Option<&[(f64, f64)]>,
    gd_after: Option<&[(f64, f64)]>,
    theme: &crate::theme::Theme,
) -> AnyElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};

    const GRAPH_WIDTH: f32 = 1200.0;
    const GRAPH_HEIGHT: f32 = 200.0;
    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;

    let chart_theme = theme_to_chart_theme(theme);

    // Use the before curve for the x-axis, or after if before is missing
    let reference = gd_before.or(gd_after).unwrap();
    let frequencies: Vec<f64> = reference.iter().map(|(f, _)| *f).collect();

    // Filter to 20Hz-20kHz and compute y range
    let in_range = |f: f64| (20.0..=20000.0).contains(&f);

    let before_values: Option<Vec<f64>> = gd_before.map(|b| b.iter().map(|(_, d)| *d).collect());

    let after_values: Option<Vec<f64>> = gd_after.map(|after| {
        // Interpolate after to match the reference frequency grid
        frequencies
            .iter()
            .map(|&f| {
                if let Some(pos) = after.windows(2).position(|w| w[0].0 <= f && f <= w[1].0) {
                    let (f0, d0) = after[pos];
                    let (f1, d1) = after[pos + 1];
                    let t = if (f1 - f0).abs() > 1e-12 {
                        (f - f0) / (f1 - f0)
                    } else {
                        0.0
                    };
                    d0 + t * (d1 - d0)
                } else {
                    after.last().map(|(_, d)| *d).unwrap_or(0.0)
                }
            })
            .collect()
    });

    // Compute y range from whichever datasets are present
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for (i, &f) in frequencies.iter().enumerate() {
        if in_range(f) {
            for vals in [&before_values, &after_values].into_iter().flatten() {
                if let Some(&v) = vals.get(i)
                    && v.is_finite()
                {
                    y_min = y_min.min(v);
                    y_max = y_max.max(v);
                }
            }
        }
    }
    // Fallback if no valid data found in range
    if !y_min.is_finite() || !y_max.is_finite() || y_min >= y_max {
        y_min = -5.0;
        y_max = 50.0;
    }
    // Round to nice bounds with some padding
    let margin = (y_max - y_min).max(1.0) * 0.1;
    y_min = (y_min - margin).floor();
    y_max = (y_max + margin).ceil();

    // Build chart: use whichever series is available as primary.
    // Only show "Before" when the measurement actually had phase data —
    // don't draw a misleading flat line at 0ms.
    let (primary_values, primary_label, primary_color) = if let Some(ref bv) = before_values {
        (bv.as_slice(), "Before", BLUE)
    } else if let Some(ref av) = after_values {
        (av.as_slice(), "After", ORANGE)
    } else {
        // Should not happen due to .when() guard, but handle gracefully
        return div().into_any_element();
    };

    let mut chart_builder = line(&frequencies, primary_values)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_range(y_min, y_max)
        .y_label("GD (ms)")
        .label(primary_label)
        .legend_position(LegendPosition::Bottom)
        .color(primary_color)
        .stroke_width(1.5)
        .opacity(0.7)
        .theme(chart_theme)
        .size(GRAPH_WIDTH, GRAPH_HEIGHT);

    // Add the secondary series only if it's different from the primary
    if before_values.is_some()
        && let Some(ref av) = after_values
    {
        chart_builder = chart_builder.add_series(av, Some("After"), ORANGE, 1.5, 0.9);
    }

    let chart = match chart_builder.build() {
        Ok(c) => c,
        Err(_) => {
            return div()
                .child(
                    Text::new("Group Delay: chart error")
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                )
                .into_any_element();
        }
    };

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Group Delay")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .child(chart)
        .into_any_element()
}
