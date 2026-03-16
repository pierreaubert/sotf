use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, StackSpacing, Text, TextSize, TextWeight, VStack,
};
use sotf_audio::signal_analysis as dsp;

// === Free functions for channel configuration UI ===

/// Render a single channel configuration row
pub(crate) fn render_channel_config_row(
    idx: usize,
    config: &crate::app::types::RoomEqSpeakerConfig,
    theme: &crate::theme::Theme,
    view: &Entity<PlayerView>,
) -> impl IntoElement {
    use crate::app::types::SpeakerConfigType;

    let channel_name = config.channel_name.clone();
    let is_multi = config.config_type == SpeakerConfigType::MultiDriver;
    let crossover_type = config.crossover_type;

    div()
        .flex()
        .gap_4()
        .items_center()
        .w_full()
        .p_3()
        .bg(theme.surface)
        .rounded_lg()
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
                .gap_2()
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
                    .gap_2()
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
    result: &crate::app::types::ChannelOptResult,
    theme: &crate::theme::Theme,
    smoothing_octaves: f64,
    y_axis_auto: bool,
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
        .gap_3()
        .p_4()
        .w_full()
        .bg(theme.surface)
        .rounded_lg()
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
                        .gap_4()
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
        // Frequency response plot (if available)
        .when(has_response_data, |div| {
            // Use original response and normalized response (the level-normalized corrected output)
            let original = result.original_response.as_ref().unwrap();
            let normalized = result.normalized_response.as_ref().unwrap();
            div.child(render_response_comparison_graph(
                original,
                normalized,
                &result.eq_filters,
                theme,
                smoothing_octaves,
                y_axis_auto,
                interactive_state,
                target_curve,
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
                .child(render_filter_table(&result.eq_filters, theme)),
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
                            .gap_2()
                            .children(xover_freqs.iter().map(|f| {
                                gpui::div()
                                    .px_2()
                                    .py_1()
                                    .bg(theme.background_secondary)
                                    .rounded_md()
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

/// Render the frequency response comparison graph with tonal balance histogram
/// If interactive_state is provided, the chart will support pan/zoom interactions
fn render_response_comparison_graph(
    original: &[(f64, f64)],
    _normalized: &[(f64, f64)],
    eq_filters: &[crate::app::types::EqFilterConfig],
    theme: &crate::theme::Theme,
    smoothing_octaves: f64,
    y_axis_auto: bool,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
    target_curve: Option<&[(f64, f64)]>,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{BarTheme, LegendPosition, ScaleType, bar, line};
    use math_audio_iir_fir::{Biquad, BiquadFilterType};

    // Use a large width that will be constrained by the parent container
    const GRAPH_WIDTH: f32 = 1200.0;
    const GRAPH_HEIGHT: f32 = 400.0;
    const SAMPLE_RATE: f64 = 48000.0;

    // CEA2034 standard colors for consistency
    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;
    const GREEN: u32 = 0x2ca02c;
    const RED: u32 = 0xd62728; // Color for target curve

    // Convert (freq, db) pairs to separate vectors
    let frequencies: Vec<f64> = original.iter().map(|(f, _)| *f).collect();
    let original_values_raw: Vec<f64> = original.iter().map(|(_, db)| *db).collect();

    // Calculate normalization offset to center around 0dB (usually 1k-2k range)
    let offset = crate::app::types::RoomEqState::calculate_normalization_offset(
        &frequencies,
        &original_values_raw,
    );
    let original_normalized: Vec<f64> = original_values_raw.iter().map(|&db| db - offset).collect();

    // Compute EQ response curve from filters
    let eq_response: Vec<f64> = if eq_filters.is_empty() {
        vec![0.0; frequencies.len()]
    } else {
        frequencies
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
                        let biquad =
                            Biquad::new(filter_type, f.frequency, SAMPLE_RATE, f.q, f.gain_db);
                        biquad.log_result(freq)
                    })
                    .sum::<f64>()
            })
            .collect()
    };

    // Apply smoothing to normalized data
    let original_values =
        dsp::smooth_response_f64(&frequencies, &original_normalized, smoothing_octaves);

    // Compute Corrected = Original (smoothed) + EQ
    // We use smoothed original for the display curve so it looks clean
    let corrected_values: Vec<f64> = original_values
        .iter()
        .zip(eq_response.iter())
        .map(|(orig, eq)| orig + eq)
        .collect();

    // Sanitize data early: replace non-finite values with 0.0 to prevent lyon_path panics
    let sanitize = |v: &[f64]| -> Vec<f64> {
        v.iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .collect()
    };
    let original_values = sanitize(&original_values);
    let corrected_values = sanitize(&corrected_values);
    let eq_response = sanitize(&eq_response);

    // Compute Y-axis range
    // If auto: include all curves
    // If fixed: use -40 to +10 dB (relative to target/average?)
    // Usually fixed range is absolute dB SPL, but here normalized might be around 0?
    // Wait, original measurement is usually absolute SPL (e.g. 70-80dB).
    // If we want fixed range [-40, 10], that implies normalized data (around 0dB).
    // The "normalized_response" passed in was likely centered around 0.
    // Our "corrected_values" are (Original + EQ). If Original is 75dB, Corrected is ~75dB.
    // A fixed range of [-40, 10] only makes sense for Relative/Normalized curves.
    // For absolute SPL, we probably want [Mean - 40, Mean + 10] or similar.
    // Or maybe the user means "Window of 50dB range".

    // Let's check the data range of original.
    let mean_spl = if !original_values.is_empty() {
        original_values.iter().sum::<f64>() / original_values.len() as f64
    } else {
        0.0
    };

    let (y_min_auto, y_max_auto) = {
        let mut min_val = f64::INFINITY;
        let mut max_val = f64::NEG_INFINITY;

        for &v in original_values.iter().chain(corrected_values.iter()) {
            min_val = min_val.min(v);
            max_val = max_val.max(v);
        }

        if let Some(target) = target_curve {
            for &(_, db) in target {
                min_val = min_val.min(db);
                max_val = max_val.max(db);
            }
        }

        // Round to nearest multiple of 5
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
        // Absolute SPL: Center around mean
        (mean_spl - 40.0, mean_spl + 10.0)
    } else {
        // Relative dB: -40 to +10
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

    // Manual linear regression helper
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

    let orig_trend = calculate_trend(&frequencies, &original_values);
    // Use corrected values for trend
    let corr_trend = calculate_trend(&frequencies, &corrected_values);

    // Compute Y2 range for EQ response
    let (eq_min, eq_max) = eq_response
        .iter()
        .fold((0.0_f64, 0.0_f64), |(min, max), &v| {
            (min.min(v), max.max(v))
        });
    let eq_y_min = (eq_min.floor() - 2.0).min(-12.0);
    let eq_y_max = (eq_max.ceil() + 2.0).max(6.0);

    // Get domain bounds - use interactive state only when zoomed, otherwise use computed/default
    let (x_min, x_max) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.x_domain())
        .unwrap_or((20.0, 20000.0));
    let (y_min_domain, y_max_domain) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.y_domain())
        .unwrap_or((y_min, y_max));

    // Build line chart
    let mut chart_builder = line(&frequencies, &original_values)
        .x_scale(ScaleType::Log)
        .x_range(x_min, x_max)
        .y_range(y_min_domain, y_max_domain)
        .y_label("SPL (dB)")
        .y2_label("EQ (dB)")
        .y2_range(eq_y_min, eq_y_max)
        .label("Original")
        .legend_position(LegendPosition::Bottom)
        .color(BLUE)
        .stroke_width(2.0)
        .opacity(1.0)
        .theme(chart_theme.clone())
        .size(GRAPH_WIDTH, GRAPH_HEIGHT)
        // Use corrected_values (Original + EQ) instead of normalized_values
        .add_series(&corrected_values, Some("Corrected"), ORANGE, 2.0, 1.0)
        .add_series_y2(&eq_response, Some("EQ"), GREEN, 2.0, 0.8);

    // Add target curve if available
    if let Some(target) = target_curve {
        // Interpolate target curve to match frequencies if needed, but line chart handles different x points?
        // gpui_px line chart expects x and y arrays. `add_series` takes y array and assumes same x array as primary.
        // `line` builder uses the primary series x array.
        // So we need to interpolate target curve to `frequencies`.

        let target_values: Vec<f64> = frequencies
            .iter()
            .map(|&f| {
                // Linear interpolation of target curve
                // Find surrounding points in target
                let mut lower = (20.0, 0.0);
                let mut upper = (20000.0, 0.0);

                // Assume target is sorted by freq
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

                // Log-linear interpolation
                let t = (f.ln() - lower.0.ln()) / (upper.0.ln() - lower.0.ln());
                lower.1 + t * (upper.1 - lower.1)
            })
            .collect();

        chart_builder = chart_builder.add_series(&target_values, Some("Target"), RED, 2.0, 0.8);
    }

    // Add trend lines if calculated
    if let Some((slope, intercept)) = orig_trend {
        let trend: Vec<f64> = frequencies
            .iter()
            .map(|f| slope * f.log10() + intercept)
            .collect();
        chart_builder = chart_builder.add_series(
            &trend,
            Some(&format!("{:.2} dB/oct", slope)),
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
            Some(&format!("{:.2} dB/oct", slope)),
            ORANGE,
            1.5,
            0.6,
        );
    }

    let line_chart = chart_builder.build();

    // Build histogram if we have trend data
    let hist_chart = if let (Some((slope_orig, int_orig)), Some((slope_corr, int_corr))) =
        (orig_trend, corr_trend)
    {
        let calculate_histogram =
            |freqs: &[f64], values: &[f64], slope: f64, intercept: f64| -> Vec<f64> {
                let min_freq = 100.0;
                let max_freq = 10000.0;
                // Bins: [0, 0.5), [0.5, 1.0), ... [3.5, 4.0), [4.0, inf)
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
                            bins[8] += 1.0; // Overflow bin
                        }
                    }
                }
                bins
            };

        let hist_orig = calculate_histogram(&frequencies, &original_values, slope_orig, int_orig);
        let hist_corr = calculate_histogram(&frequencies, &corrected_values, slope_corr, int_corr);

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
            .size(GRAPH_WIDTH, GRAPH_HEIGHT / 2.0)
            .bar_gap(4.0)
            .opacity(0.8)
            .legend_position(LegendPosition::Bottom)
            .add_series(&hist_corr, Some("Corrected"), ORANGE, 0.8)
            .build()
            .ok()
    } else {
        None
    };

    // Build the main chart element, wrapping with interactive if state is provided
    let line_chart_element: Option<gpui::AnyElement> = line_chart.ok().map(|chart| {
        if let Some(state) = interactive_state {
            gpui_px::interaction::interactive("room-eq-response-chart", chart, state.clone())
                .build()
                .into_any_element()
        } else {
            chart.into_any_element()
        }
    });

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .when_some(line_chart_element, |el, c| el.child(c))
        .when_some(hist_chart, |el, c| el.child(c))
        .into_any_element()
}

/// Render the EQ filter table
fn render_filter_table(
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
        .gap_2()
        .children(filters.iter().enumerate().map(|(i, f)| {
            let gain_color = if f.gain_db > 0.5 {
                theme.success
            } else if f.gain_db < -0.5 {
                theme.error
            } else {
                theme.text_muted
            };

            div()
                .px_3()
                .py_2()
                .bg(theme.background_secondary)
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap_1()
                .min_w(rems(5.0))
                // Filter number and type
                .child(
                    div()
                        .flex()
                        .gap_1()
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
                        .gap_2()
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
