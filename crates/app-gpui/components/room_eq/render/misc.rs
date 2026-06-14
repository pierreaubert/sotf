use gpui::*;

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

pub fn is_room_eq_sub_or_lfe_channel(channel_name: &str) -> bool {
    autoeq::roomeq::home_cinema::role_for_channel(channel_name).is_sub_or_lfe()
}

pub(super) fn count_points_in_domain(points: &[(f64, f64)], domain: (f64, f64)) -> usize {
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

pub(super) fn finite_positive_frequency_range(frequencies: &[f64]) -> Option<(f64, f64)> {
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

pub(super) fn route_display_name(kind: &str) -> &'static str {
    match kind {
        "main_highpass_to_self" => "main high-pass",
        "main_highpass" => "main high-pass",
        "redirected_bass_lowpass_to_sub" => "redirected bass",
        "lfe_lowpass_to_sub" => "LFE to sub",
        "full_range" => "full range",
        _ => "route",
    }
}

pub(super) fn rgba_from_u32(color: u32) -> Rgba {
    let r = ((color >> 16) & 0xff) as f32 / 255.0;
    let g = ((color >> 8) & 0xff) as f32 / 255.0;
    let b = (color & 0xff) as f32 / 255.0;
    Rgba { r, g, b, a: 1.0 }
}
