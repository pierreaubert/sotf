use crate::components::design::Ds;
use crate::components::graphs::common::render_empty_state;
use crate::components::icons::IconName;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, StackSpacing, Text, TextSize, TextWeight, VStack,
};
use sotf_audio::signal_analysis as dsp;

/// Whether the Review step should render the per-filter plot for a
/// channel result.
///
/// Pure predicate extracted from [`Self::render_room_eq_review`] so that
/// the rule is testable in isolation. The plot should appear whenever we
/// have frequency-response data **and** at least one filter stage to
/// overlay — either the main IIR set (`has_main`) or the broadband
/// pre-correction (`has_broadband`).
///
/// Regression guard for Issue 6: a previous version gated on `has_main`
/// alone, which meant broadband-only optimizations rendered no plot at
/// all even though filters existed.
pub fn should_render_filter_plot(
    has_response_data: bool,
    has_main: bool,
    has_broadband: bool,
) -> bool {
    has_response_data && (has_main || has_broadband)
}

pub fn room_eq_trend_fit_domain(_channel_name: &str, frequencies: &[f64]) -> Option<(f64, f64)> {
    let (data_min, data_max) = finite_positive_frequency_range(frequencies)?;
    let (fit_min, fit_max) = (100.0_f64, 10_000.0_f64);

    let min_freq = fit_min.max(data_min);
    let max_freq = fit_max.min(data_max);
    (max_freq > min_freq).then_some((min_freq, max_freq))
}

pub fn is_room_eq_sub_or_lfe_channel(channel_name: &str) -> bool {
    autoeq::roomeq::home_cinema::role_for_channel(channel_name).is_sub_or_lfe()
}

pub fn room_eq_passband_trend_fit_domain(freqs: &[f64], values: &[f64]) -> Option<(f64, f64)> {
    const PASSBAND_DROP_DB: f64 = 3.0;
    const LOG_INSET_FRACTION: f64 = 0.20;

    let points: Vec<(f64, f64)> = freqs
        .iter()
        .zip(values.iter())
        .filter_map(|(&f, &v)| (f.is_finite() && f > 0.0 && v.is_finite()).then_some((f, v)))
        .collect();
    if points.len() < 2 {
        return None;
    }

    let max_value = points
        .iter()
        .map(|(_, v)| *v)
        .fold(f64::NEG_INFINITY, f64::max);
    if !max_value.is_finite() {
        return None;
    }
    let threshold = max_value - PASSBAND_DROP_DB;
    let first_in_band = points.iter().position(|(_, v)| *v >= threshold)?;
    let last_in_band = points.iter().rposition(|(_, v)| *v >= threshold)?;

    let lower_3db = if first_in_band > 0 {
        interpolate_log_frequency_at_db(points[first_in_band - 1], points[first_in_band], threshold)
    } else {
        points[first_in_band].0
    };
    let upper_3db = if last_in_band + 1 < points.len() {
        interpolate_log_frequency_at_db(points[last_in_band], points[last_in_band + 1], threshold)
    } else {
        points[last_in_band].0
    };

    if !lower_3db.is_finite() || !upper_3db.is_finite() || upper_3db <= lower_3db {
        return None;
    }

    let log_lower = lower_3db.ln();
    let log_upper = upper_3db.ln();
    let log_width = log_upper - log_lower;
    if log_width <= 1e-9 {
        return None;
    }

    let reduced = (
        (log_lower + LOG_INSET_FRACTION * log_width).exp(),
        (log_upper - LOG_INSET_FRACTION * log_width).exp(),
    );
    if count_points_in_domain(&points, reduced) >= 2 {
        Some(reduced)
    } else if count_points_in_domain(&points, (lower_3db, upper_3db)) >= 2 {
        Some((lower_3db, upper_3db))
    } else {
        None
    }
}

fn interpolate_log_frequency_at_db(lower: (f64, f64), upper: (f64, f64), target_db: f64) -> f64 {
    let denom = upper.1 - lower.1;
    if denom.abs() < 1e-12 {
        return (lower.0 * upper.0).sqrt();
    }
    let t = ((target_db - lower.1) / denom).clamp(0.0, 1.0);
    (lower.0.ln() + t * (upper.0.ln() - lower.0.ln())).exp()
}

fn count_points_in_domain(points: &[(f64, f64)], domain: (f64, f64)) -> usize {
    points
        .iter()
        .filter(|(f, _)| *f >= domain.0 && *f <= domain.1)
        .count()
}

pub fn calculate_room_eq_log_trend(
    freqs: &[f64],
    values: &[f64],
    domain: (f64, f64),
) -> Option<(f64, f64)> {
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_xx = 0.0;
    let mut count = 0.0;

    for (i, &f) in freqs.iter().enumerate() {
        if f >= domain.0
            && f <= domain.1
            && f.is_finite()
            && f > 0.0
            && let Some(&y) = values.get(i)
            && y.is_finite()
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
}

fn finite_positive_frequency_range(frequencies: &[f64]) -> Option<(f64, f64)> {
    let mut min_freq = f64::INFINITY;
    let mut max_freq = f64::NEG_INFINITY;

    for &f in frequencies {
        if f.is_finite() && f > 0.0 {
            min_freq = min_freq.min(f);
            max_freq = max_freq.max(f);
        }
    }

    (min_freq.is_finite() && max_freq.is_finite()).then_some((min_freq, max_freq))
}

#[derive(Debug, Clone)]
pub(crate) struct CrossoverResponseOverlay {
    pub main_highpass_label: String,
    pub main_highpass: Vec<(f64, f64)>,
    pub sub_lowpass_label: String,
    pub sub_lowpass: Vec<(f64, f64)>,
    pub crossover_label: String,
    pub crossover_hz: f64,
}

fn trend_anchor_frequency(domain: (f64, f64)) -> f64 {
    if domain.0 <= 1000.0 && domain.1 >= 1000.0 {
        1000.0
    } else {
        (domain.0 * domain.1).sqrt()
    }
}

fn trend_line_points(domain: (f64, f64), slope: f64, intercept: f64) -> (Vec<f64>, Vec<f64>) {
    let x = vec![domain.0, domain.1];
    let y = x.iter().map(|f| slope * f.log10() + intercept).collect();
    (x, y)
}

pub fn sum_room_eq_responses_db(
    main: &[(f64, f64)],
    sub: &[(f64, f64)],
    main_phase_deg: Option<&[(f64, f64)]>,
    sub_phase_deg: Option<&[(f64, f64)]>,
) -> Vec<(f64, f64)> {
    let sub_freqs: Vec<f64> = sub.iter().map(|(f, _)| *f).collect();
    let sub_values: Vec<f64> = sub.iter().map(|(_, db)| *db).collect();
    let Some((sub_min, sub_max)) = finite_positive_frequency_range(&sub_freqs) else {
        return main.to_vec();
    };

    let main_phase_freqs: Vec<f64> = main_phase_deg
        .unwrap_or_default()
        .iter()
        .map(|(f, _)| *f)
        .collect();
    let main_phase_values: Vec<f64> = main_phase_deg
        .unwrap_or_default()
        .iter()
        .map(|(_, p)| *p)
        .collect();
    let sub_phase_freqs: Vec<f64> = sub_phase_deg
        .unwrap_or_default()
        .iter()
        .map(|(f, _)| *f)
        .collect();
    let sub_phase_values: Vec<f64> = sub_phase_deg
        .unwrap_or_default()
        .iter()
        .map(|(_, p)| *p)
        .collect();
    let phase_available = !main_phase_freqs.is_empty() && !sub_phase_freqs.is_empty();

    main.iter()
        .map(|&(f, main_db)| {
            if f < sub_min || f > sub_max || !f.is_finite() || !main_db.is_finite() {
                return (f, main_db);
            }

            let sub_db = interpolate_value_at(&sub_freqs, &sub_values, f);
            if !sub_db.is_finite() {
                return (f, main_db);
            }

            let sum_db = if phase_available {
                let main_phase =
                    interpolate_value_at(&main_phase_freqs, &main_phase_values, f).to_radians();
                let sub_phase =
                    interpolate_value_at(&sub_phase_freqs, &sub_phase_values, f).to_radians();
                let main_amp = 10.0_f64.powf(main_db / 20.0);
                let sub_amp = 10.0_f64.powf(sub_db / 20.0);
                let re = main_amp * main_phase.cos() + sub_amp * sub_phase.cos();
                let im = main_amp * main_phase.sin() + sub_amp * sub_phase.sin();
                20.0 * re.hypot(im).max(1.0e-12).log10()
            } else {
                let power = 10.0_f64.powf(main_db / 10.0) + 10.0_f64.powf(sub_db / 10.0);
                10.0 * power.max(1.0e-24).log10()
            };
            (f, sum_db)
        })
        .collect()
}

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

/// Interpolate a sampled curve at a single frequency using log-frequency linear interpolation.
fn interpolate_value_at(frequencies: &[f64], values: &[f64], target_freq: f64) -> f64 {
    if frequencies.is_empty() || values.is_empty() {
        return 0.0;
    }
    if target_freq <= frequencies[0] {
        return values[0];
    }
    if target_freq >= *frequencies.last().unwrap() {
        return *values.last().unwrap();
    }
    for i in 0..frequencies.len() - 1 {
        if target_freq >= frequencies[i] && target_freq <= frequencies[i + 1] {
            let denom = frequencies[i + 1].ln() - frequencies[i].ln();
            if denom.abs() < 1e-12 {
                return values[i];
            }
            let t = (target_freq.ln() - frequencies[i].ln()) / denom;
            return values[i] + t * (values[i + 1] - values[i]);
        }
    }
    *values.last().unwrap()
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
    has_fir: bool,
    crossover_overlay: Option<&CrossoverResponseOverlay>,
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
        // Filter plot: each filter and the sum (if available).
        // Render if there are ANY filters to show (main IIR or broadband
        // pre-correction). Previously this gated on `eq_filters` only, so
        // broadband-only optimizations silently dropped the plot.
        .when(
            should_render_filter_plot(
                has_response_data,
                !result.eq_filters.is_empty(),
                !result.broadband_filters.is_empty(),
            ),
            |div| {
                let (Some(original), Some(normalized)) = (
                    result.original_response.as_ref(),
                    result.normalized_response.as_ref(),
                ) else {
                    return div;
                };
                div.child(render_filter_plot(
                    original,
                    normalized,
                    &result.eq_filters,
                    &result.broadband_filters,
                    result.preamp_gain_db,
                    has_fir,
                    theme,
                    smoothing_octaves,
                    y_axis_auto,
                    interactive_state,
                ))
            },
        )
        // Original vs Corrected with trendlines (if available)
        .when(has_response_data, |div| {
            let (Some(original), Some(normalized)) = (
                result.original_response.as_ref(),
                result.normalized_response.as_ref(),
            ) else {
                return div;
            };
            div.child(render_response_comparison_graph(
                &result.channel_name,
                original,
                normalized,
                result.preamp_gain_db,
                theme,
                smoothing_octaves,
                y_axis_auto,
                normalize_to_target,
                interactive_state,
                target_curve,
                crossover_overlay,
            ))
        })
        // Histogram (if trend data available)
        .when(
            (result.group_delay_before.is_some() || result.group_delay_after.is_some())
                && has_response_data,
            |div| {
                let (Some(original), Some(normalized)) = (
                    result.original_response.as_ref(),
                    result.normalized_response.as_ref(),
                ) else {
                    return div;
                };
                div.child(render_tonal_histogram(
                    &result.channel_name,
                    original,
                    normalized,
                    result.preamp_gain_db,
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
        .when_some(result.impulse_response.as_ref(), |div, ir| {
            div.child(render_impulse_response_graph(ir, theme))
        })
        // EQ Filter details — main (IIR room correction) and broadband
        // pre-correction are shown as separate tables so users can tell them
        // apart. The `Applied to Rack` action creates one named plugin per
        // section, so the grouping here mirrors what lands in the rack.
        .when(!result.eq_filters.is_empty(), |el| {
            el.child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Text::new("Room EQ Filters")
                            .weight(TextWeight::Semibold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    )
                    .child(render_filter_table(d, &result.eq_filters, theme)),
            )
        })
        .when(!result.broadband_filters.is_empty(), |el| {
            el.child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Text::new("Broadband Pre-correction Filters")
                            .weight(TextWeight::Semibold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    )
                    .child(render_filter_table(d, &result.broadband_filters, theme)),
            )
        })
        // Crossover info (if multi-driver)
        .when_some(result.crossover_freqs.as_ref(), |el, xover_freqs| {
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
    channel_name: &str,
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    preamp_gain_db: f64,
    theme: &crate::theme::Theme,
    smoothing_octaves: f64,
    y_axis_auto: bool,
    normalize_to_target: bool,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
    target_curve: Option<&[(f64, f64)]>,
    crossover_overlay: Option<&CrossoverResponseOverlay>,
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
    // Strip the inter-channel level-match preamp from the Corrected curve.
    // `preamp_gain_db` is a constant gain plugin pushed onto the chain by
    // post-optimization stages (spectral-alignment, VoG). It shifts the
    // channel's absolute level to match its neighbours but is not part of
    // the per-channel EQ correction. Including it here makes one channel
    // look uniformly lifted (positive preamp) or dropped (negative preamp)
    // versus its Original, which obscures whether the EQ filter shape
    // actually flattened the response.
    let corrected_values_raw: Vec<f64> = corrected
        .iter()
        .map(|(_, db)| *db - preamp_gain_db)
        .collect();

    // When normalizing to target, subtract the interpolated target curve from all
    // series so the target becomes a flat 0 dB line, then anchor inside the
    // channel's trend-fit band.
    let target_interpolated =
        target_curve.map(|target| interpolate_target_at_frequencies(&frequencies, target));

    let standard_offset = crate::app::types::RoomEqState::calculate_normalization_offset(
        &frequencies,
        &original_values_raw,
    );

    // normalize_to_target shifts are deferred until after trend computation.
    // First, subtract the target curve (if normalizing) so the target becomes 0 dB.
    let (original_values, corrected_values): (Vec<f64>, Vec<f64>) =
        if normalize_to_target && let Some(ref target_vals) = target_interpolated {
            // Subtract target from raw curves (target becomes 0 dB everywhere)
            let orig: Vec<f64> = original_values_raw
                .iter()
                .zip(target_vals.iter())
                .map(|(&o, &t)| o - t)
                .collect();
            let corr: Vec<f64> = corrected_values_raw
                .iter()
                .zip(target_vals.iter())
                .map(|(&c, &t)| c - t)
                .collect();
            (orig, corr)
        } else {
            // Standard normalization: 1-2 kHz average
            (
                original_values_raw
                    .iter()
                    .map(|&db| db - standard_offset)
                    .collect(),
                corrected_values_raw
                    .iter()
                    .map(|&db| db - standard_offset)
                    .collect(),
            )
        };

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

    // Overlay prep — `subtract_preamp` strips the main channel's level-match
    // preamp from curves derived from `corrected_response` (main_highpass).
    // The sub_lowpass is left alone because its absolute level (relative to
    // the un-de-preamped mains) is what places it correctly in-room, and we
    // want the combined "Main + Sub" sum to reflect that physical balance.
    let prepare_overlay_series =
        |points: &[(f64, f64)], subtract_preamp: bool| -> Option<(Vec<f64>, Vec<f64>)> {
            if points.is_empty() {
                return None;
            }
            let preamp = if subtract_preamp { preamp_gain_db } else { 0.0 };
            let overlay_freqs: Vec<f64> = points.iter().map(|(f, _)| *f).collect();
            let overlay_raw: Vec<f64> = points.iter().map(|(_, db)| *db - preamp).collect();
            let overlay_values: Vec<f64> = if normalize_to_target && let Some(target) = target_curve
            {
                let target_vals = interpolate_target_at_frequencies(&overlay_freqs, target);
                overlay_raw
                    .iter()
                    .zip(target_vals.iter())
                    .map(|(&v, &target)| v - target)
                    .collect()
            } else {
                overlay_raw.iter().map(|&db| db - standard_offset).collect()
            };
            let overlay_smooth =
                dsp::smooth_response_f64(&overlay_freqs, &overlay_values, smoothing_octaves);
            Some((overlay_freqs, sanitize(&overlay_smooth)))
        };

    // Power-sum two dB curves sharing a frequency grid.
    // Returns the predicted combined SPL assuming uncorrelated summation —
    // the standard worst-case approximation for in-room main + sub overlap,
    // since the phase relationship at the listening position is unknown.
    let power_sum_db = |freqs_a: &[f64],
                        values_a: &[f64],
                        freqs_b: &[f64],
                        values_b: &[f64]|
     -> Option<(Vec<f64>, Vec<f64>)> {
        if freqs_a.len() != values_a.len()
            || freqs_b.len() != values_b.len()
            || freqs_a.is_empty()
            || freqs_a.len() != freqs_b.len()
        {
            return None;
        }
        // Cheap grid check: if frequencies match index-for-index we sum
        // directly. Different grids would need interpolation, which we
        // skip here — main_highpass and sub_lowpass both come from
        // `apply_crossover_route` and inherit the same measurement grid.
        if freqs_a
            .iter()
            .zip(freqs_b.iter())
            .any(|(a, b)| (a - b).abs() > a.abs() * 1e-6 + 1e-9)
        {
            return None;
        }
        let combined: Vec<f64> = values_a
            .iter()
            .zip(values_b.iter())
            .map(|(&a, &b)| {
                let pa = 10f64.powf(a / 10.0);
                let pb = 10f64.powf(b / 10.0);
                10.0 * (pa + pb).max(1e-30).log10()
            })
            .collect();
        Some((freqs_a.to_vec(), combined))
    };

    let crossover_overlay_series = crossover_overlay.and_then(|overlay| {
        let main_highpass = prepare_overlay_series(&overlay.main_highpass, true)?;
        let sub_lowpass = prepare_overlay_series(&overlay.sub_lowpass, false)?;
        let combined = power_sum_db(
            &main_highpass.0,
            &main_highpass.1,
            &sub_lowpass.0,
            &sub_lowpass.1,
        );
        Some((overlay, main_highpass, sub_lowpass, combined))
    });

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
        if let Some((_, (_, highpass_values), (_, lowpass_values), combined)) =
            crossover_overlay_series.as_ref()
        {
            for &v in highpass_values.iter().chain(lowpass_values.iter()) {
                min_val = min_val.min(v);
                max_val = max_val.max(v);
            }
            if let Some((_, combined_values)) = combined {
                for &v in combined_values.iter() {
                    min_val = min_val.min(v);
                    max_val = max_val.max(v);
                }
            }
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
        return render_empty_state(IconName::AudioWaveform, "No data available", theme);
    }

    let chart_theme = theme_to_chart_theme(theme);

    let is_sub_or_lfe = is_room_eq_sub_or_lfe_channel(channel_name);
    let orig_trend_domain = if is_sub_or_lfe {
        room_eq_passband_trend_fit_domain(&frequencies, &original_smooth)
    } else {
        room_eq_trend_fit_domain(channel_name, &frequencies)
    };
    let corr_trend_domain = if is_sub_or_lfe {
        room_eq_passband_trend_fit_domain(&frequencies, &corrected_smooth)
    } else {
        room_eq_trend_fit_domain(channel_name, &frequencies)
    };
    let orig_trend = orig_trend_domain
        .and_then(|domain| calculate_room_eq_log_trend(&frequencies, &original_smooth, domain));
    let corr_trend = corr_trend_domain
        .and_then(|domain| calculate_room_eq_log_trend(&frequencies, &corrected_smooth, domain));

    // When normalizing to target, shift each curve (and its trend) so its
    // trend line crosses 0 dB at 1 kHz.  This makes deviations from the
    // target directly readable without the broadband level difference
    // obscuring them.
    let (original_smooth, corrected_smooth, orig_trend, corr_trend) =
        if normalize_to_target && target_interpolated.is_some() {
            let orig_anchor_freq = orig_trend_domain
                .map(trend_anchor_frequency)
                .unwrap_or(1000.0);
            let corr_anchor_freq = corr_trend_domain
                .map(trend_anchor_frequency)
                .unwrap_or(1000.0);
            let orig_log_anchor = orig_anchor_freq.log10();
            let corr_log_anchor = corr_anchor_freq.log10();
            let orig_offset = orig_trend
                .map(|(s, i)| s * orig_log_anchor + i)
                .unwrap_or_else(|| {
                    interpolate_value_at(&frequencies, &original_smooth, orig_anchor_freq)
                });
            let corr_offset = corr_trend
                .map(|(s, i)| s * corr_log_anchor + i)
                .unwrap_or_else(|| {
                    interpolate_value_at(&frequencies, &corrected_smooth, corr_anchor_freq)
                });
            (
                original_smooth
                    .iter()
                    .map(|&v| v - orig_offset)
                    .collect::<Vec<_>>(),
                corrected_smooth
                    .iter()
                    .map(|&v| v - corr_offset)
                    .collect::<Vec<_>>(),
                orig_trend.map(|(s, i)| (s, i - orig_offset)),
                corr_trend.map(|(s, i)| (s, i - corr_offset)),
            )
        } else {
            (original_smooth, corrected_smooth, orig_trend, corr_trend)
        };

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

    let original_label = format!("{} Original", channel_name);
    let corrected_label = if preamp_gain_db.abs() >= 0.05 {
        format!(
            "{} Corrected (preamp {:+.1} dB removed)",
            channel_name, preamp_gain_db
        )
    } else {
        format!("{} Corrected", channel_name)
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
        .label(original_label)
        .legend_position(LegendPosition::Right)
        .color(BLUE)
        .stroke_width(2.0)
        .opacity(1.0)
        .theme(chart_theme.clone())
        .size(GRAPH_WIDTH, GRAPH_HEIGHT)
        .add_series(&corrected_smooth, Some(&corrected_label), ORANGE, 2.0, 1.0);

    if let Some((overlay, (highpass_x, highpass_y), (lowpass_x, lowpass_y), combined)) =
        crossover_overlay_series.as_ref()
    {
        chart_builder = chart_builder
            .add_series_with_x(
                highpass_x,
                highpass_y,
                Some(&overlay.main_highpass_label),
                0x2ca02c,
                1.8,
                0.9,
            )
            .add_series_with_x(
                lowpass_x,
                lowpass_y,
                Some(&overlay.sub_lowpass_label),
                0x17becf,
                1.8,
                0.9,
            );

        // Predicted combined response (Main HP + Sub LP, power-summed).
        // The point of this overlay is to answer "does the sub fill the
        // main's bass dip?" — if the combined line lifts the corrected
        // curve back onto the trend below the crossover, bass management
        // is doing its job; if it doesn't, the dip survives and the user
        // needs to revisit sub level, crossover, or the sub's own EQ.
        if let Some((combined_x, combined_y)) = combined.as_ref() {
            chart_builder = chart_builder.add_series_with_x(
                combined_x,
                combined_y,
                Some("Combined (Main HP + Sub LP)"),
                0x9467bd,
                2.0,
                0.95,
            );
        }

        if overlay.crossover_hz.is_finite() && overlay.crossover_hz > 0.0 {
            chart_builder = chart_builder.add_series_with_x(
                &[overlay.crossover_hz, overlay.crossover_hz],
                &[y_min_domain, y_max_domain],
                Some(&overlay.crossover_label),
                0x666666,
                1.0,
                0.55,
            );
        }
    }

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
        let (trend_x, trend_y) = trend_line_points(
            orig_trend_domain.unwrap_or((20.0, 20_000.0)),
            slope,
            intercept,
        );
        chart_builder = chart_builder.add_series_with_x(
            &trend_x,
            &trend_y,
            Some(&format!("{:.2} dB/dec", slope)),
            BLUE,
            1.5,
            0.6,
        );
    }

    if let Some((slope, intercept)) = corr_trend {
        let (trend_x, trend_y) = trend_line_points(
            corr_trend_domain.unwrap_or((20.0, 20_000.0)),
            slope,
            intercept,
        );
        chart_builder = chart_builder.add_series_with_x(
            &trend_x,
            &trend_y,
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

/// Render the filter plot showing each individual filter and their combined response.
///
/// `has_fir` tells the plot whether the channel's DSP chain contains a
/// convolution/FIR block. We can't decompose FIR magnitude into parametric
/// bands, but at minimum the user deserves to know the chain includes an
/// FIR correction they won't see as individual lines.
fn render_filter_plot(
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    eq_filters: &[crate::app::types::EqFilterConfig],
    broadband_filters: &[crate::app::types::EqFilterConfig],
    preamp_gain_db: f64,
    has_fir: bool,
    theme: &crate::theme::Theme,
    _smoothing_octaves: f64,
    y_axis_auto: bool,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, StrokeDashArray, line};
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

    if frequencies.is_empty() || (eq_filters.is_empty() && broadband_filters.is_empty()) {
        return div()
            .child(render_empty_state(
                IconName::AudioWaveform,
                "No filter data available",
                theme,
            ))
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

    let parse_type = |s: &str| -> BiquadFilterType {
        match s {
            "peak" | "pk" | "Peak" => BiquadFilterType::Peak,
            "lowshelf" | "ls" | "Lowshelf" => BiquadFilterType::Lowshelf,
            "highshelf" | "hs" | "Highshelf" => BiquadFilterType::Highshelf,
            "lowpass" | "lp" | "Lowpass" => BiquadFilterType::Lowpass,
            "highpass" | "hp" | "Highpass" => BiquadFilterType::Highpass,
            _ => BiquadFilterType::Peak,
        }
    };

    let filter_response_at = |f: &crate::app::types::EqFilterConfig, freq: f64| -> f64 {
        let biquad = Biquad::new(
            parse_type(&f.filter_type),
            f.frequency,
            SAMPLE_RATE,
            f.q,
            f.gain_db,
        );
        biquad.log_result(freq)
    };

    let sanitize = |v: &[f64]| -> Vec<f64> {
        v.iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .collect()
    };

    // Compute combined sum of main EQ + broadband, plus any flat preamp gain
    // applied by post-optimization stages (spectral-alignment, VoG). Without
    // including `preamp_gain_db` here, the Sum line drifts from the actual
    // correction visible in the "Original vs Corrected" plot whenever a
    // flat-gain plugin is present in the channel chain.
    let all_filters: Vec<&crate::app::types::EqFilterConfig> =
        eq_filters.iter().chain(broadband_filters.iter()).collect();
    let eq_response: Vec<f64> = sanitize(
        &frequencies
            .iter()
            .map(|&freq| {
                all_filters
                    .iter()
                    .map(|f| filter_response_at(f, freq))
                    .sum::<f64>()
                    + preamp_gain_db
            })
            .collect::<Vec<_>>(),
    );

    let mut chart_builder = line(&frequencies, &vec![0.0; frequencies.len()])
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_range(-12.0, 6.0)
        .y_label("EQ (dB)")
        .label("Sum")
        .legend_position(LegendPosition::Right)
        .color(GREEN)
        .stroke_width(2.0)
        .opacity(1.0)
        .theme(chart_theme.clone())
        .size(GRAPH_WIDTH, GRAPH_HEIGHT);

    chart_builder = chart_builder.add_series(&eq_response, Some("Sum"), GREEN, 2.0, 1.0);

    // Main EQ filters (parametric IIR biquads from the room optimizer).
    for (i, filter) in eq_filters.iter().enumerate() {
        let resp = sanitize(
            &frequencies
                .iter()
                .map(|&f| filter_response_at(filter, f))
                .collect::<Vec<_>>(),
        );
        let color = filter_colors[i % filter_colors.len()];
        let label = format!(
            "IIR {} {} {:.0}Hz",
            i + 1,
            filter.filter_type,
            filter.frequency
        );
        chart_builder = chart_builder.add_series(&resp, Some(&label), color, 1.5, 0.7);
    }

    // Broadband pre-correction filters — same palette but drawn dashed so
    // the user can visually separate "room IIR correction" from "driver
    // tonal pre-tilt" without having to hunt the color legend.
    const BB_COLOR: u32 = 0x8B4513; // saddle brown — distinct from PK palette
    for (i, filter) in broadband_filters.iter().enumerate() {
        let resp = sanitize(
            &frequencies
                .iter()
                .map(|&f| filter_response_at(filter, f))
                .collect::<Vec<_>>(),
        );
        let label = format!(
            "Broadband {} {} {:.0}Hz",
            i + 1,
            filter.filter_type,
            filter.frequency
        );
        chart_builder = chart_builder
            .add_series(&resp, Some(&label), BB_COLOR, 1.5, 0.7)
            .series_dash_array(StrokeDashArray::Dashed);
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

    let iir_count = eq_filters.len();
    let bb_count = broadband_filters.len();
    // Tell the user exactly what each line represents. This header line
    // doubles as a legend key so they can parse the chart without hunting
    // through the color-coded entries on the side.
    let mut subtitle_parts: Vec<String> = Vec::new();
    if iir_count > 0 {
        subtitle_parts.push(format!("{} IIR peak filters", iir_count));
    }
    if bb_count > 0 {
        subtitle_parts.push(format!("{} broadband pre-corrections (dashed)", bb_count));
    }
    if preamp_gain_db.abs() >= 0.05 {
        subtitle_parts.push(format!("preamp {:+.1} dB", preamp_gain_db));
    }
    if has_fir {
        subtitle_parts
            .push("FIR correction applied (magnitude included in Corrected curve)".to_string());
    }
    let subtitle = if subtitle_parts.is_empty() {
        None
    } else {
        Some(subtitle_parts.join(" + "))
    };

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("EQ Filters")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(subtitle, |el, s| {
            el.child(Text::new(s).size(TextSize::Xs).color(theme.text_secondary))
        })
        .when_some(chart_element, |el, c| el.child(c))
        .into_any_element()
}

/// Render the tonal balance histogram
fn render_tonal_histogram(
    channel_name: &str,
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    preamp_gain_db: f64,
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
    // Strip the inter-channel level-match preamp from Corrected so the
    // histogram reflects EQ shape, not the per-channel gain shift.
    let corrected_values_raw: Vec<f64> = corrected
        .iter()
        .map(|(_, db)| *db - preamp_gain_db)
        .collect();

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

    let is_sub_or_lfe = is_room_eq_sub_or_lfe_channel(channel_name);
    let orig_trend_domain = if is_sub_or_lfe {
        room_eq_passband_trend_fit_domain(&frequencies, &original_smooth)
    } else {
        room_eq_trend_fit_domain(channel_name, &frequencies)
    };
    let corr_trend_domain = if is_sub_or_lfe {
        room_eq_passband_trend_fit_domain(&frequencies, &corrected_smooth)
    } else {
        room_eq_trend_fit_domain(channel_name, &frequencies)
    };
    let orig_trend = orig_trend_domain
        .and_then(|domain| calculate_room_eq_log_trend(&frequencies, &original_smooth, domain));
    let corr_trend = corr_trend_domain
        .and_then(|domain| calculate_room_eq_log_trend(&frequencies, &corrected_smooth, domain));

    let hist_chart = if let (Some((slope_orig, int_orig)), Some((slope_corr, int_corr))) =
        (orig_trend, corr_trend)
    {
        let calculate_histogram = |freqs: &[f64],
                                   values: &[f64],
                                   domain: Option<(f64, f64)>,
                                   slope: f64,
                                   intercept: f64|
         -> Vec<f64> {
            let mut bins = vec![0.0; 9];

            for (i, &f) in freqs.iter().enumerate() {
                if let Some(domain) = domain
                    && f >= domain.0
                    && f <= domain.1
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

        let hist_orig = calculate_histogram(
            &frequencies,
            &original_smooth,
            orig_trend_domain,
            slope_orig,
            int_orig,
        );
        let hist_corr = calculate_histogram(
            &frequencies,
            &corrected_smooth,
            corr_trend_domain,
            slope_corr,
            int_corr,
        );

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

    let Some(reference) = phase_before.or(phase_after) else {
        return div().into_any_element();
    };
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

    let chart_element: Option<gpui::AnyElement> =
        chart_builder.build().ok().map(|c| c.into_any_element());

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Phase Response")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(chart_element, |div, el| div.child(el))
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

    let chart_element: Option<gpui::AnyElement> = line(&samples, &sanitize)
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
        .ok()
        .map(|c| c.into_any_element());

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Impulse Response")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(chart_element, |div, el| div.child(el))
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
            .child(render_empty_state(
                IconName::AudioWaveform,
                "No filters",
                theme,
            ))
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
                        .child(Text::caption(format!("Q:{:.1}", f.q))),
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
    let Some(reference) = gd_before.or(gd_after) else {
        return div().into_any_element();
    };
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

    let chart_element: Option<gpui::AnyElement> =
        chart_builder.build().ok().map(|c| c.into_any_element());

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Group Delay")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(chart_element, |div, el| div.child(el))
        .into_any_element()
}
