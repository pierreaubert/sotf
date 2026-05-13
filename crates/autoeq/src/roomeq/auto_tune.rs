//! Automatic room EQ optimizer parameter selection.

use super::types::OptimizerConfig;
use crate::Curve;

const AUTO_MIN_Q: f64 = 0.5;
const AUTO_HIGH_FREQ_MAX_Q_BROADBAND: f64 = 1.0;
const AUTO_HIGH_FREQ_MAX_Q: f64 = 1.5;
const AUTO_MAIN_MAX_Q: f64 = 6.0;
const AUTO_SUB_MAX_Q: f64 = 10.0;
const AUTO_MAIN_MAX_BOOST_DB: f64 = 4.0;
const AUTO_SUB_MAX_BOOST_DB: f64 = 6.0;
const AUTO_MAIN_MAX_CUT_DB: f64 = 12.0;
const AUTO_SUB_MAX_CUT_DB: f64 = 18.0;

#[derive(Debug, Clone)]
pub(super) struct AutoOptimizerContext {
    pub is_sub_channel: bool,
    pub effective_min_freq: f64,
    pub effective_max_freq: f64,
    pub detected_f3_hz: Option<f64>,
    pub schroeder_hz: Option<f64>,
    pub target_tilt_active: bool,
    pub broadband_enabled: bool,
}

#[derive(Debug, Clone, Copy)]
struct CurveAnalysis {
    octave_span: f64,
    rms_db: f64,
    max_peak_db: f64,
    max_dip_db: f64,
    problem_count: usize,
    modal_q_estimate: Option<f64>,
}

impl CurveAnalysis {
    fn analyze(curve: &Curve, min_freq: f64, max_freq: f64) -> Self {
        let mut points: Vec<(f64, f64)> = curve
            .freq
            .iter()
            .zip(curve.spl.iter())
            .filter_map(|(&f, &spl)| {
                if f.is_finite() && spl.is_finite() && f >= min_freq && f <= max_freq && f > 0.0 {
                    Some((f, spl))
                } else {
                    None
                }
            })
            .collect();

        points.sort_by(|a, b| a.0.total_cmp(&b.0));

        let octave_span = if min_freq > 0.0 && max_freq > min_freq {
            (max_freq / min_freq).log2()
        } else {
            1.0
        }
        .max(0.25);

        if points.len() < 3 {
            return Self {
                octave_span,
                rms_db: 0.0,
                max_peak_db: 0.0,
                max_dip_db: 0.0,
                problem_count: 0,
                modal_q_estimate: None,
            };
        }

        let mean = points.iter().map(|(_, spl)| spl).sum::<f64>() / points.len() as f64;
        let deviations: Vec<(f64, f64)> = points.iter().map(|(f, spl)| (*f, *spl - mean)).collect();
        let smoothed = smooth_deviations(&deviations, 5);
        let rms_db =
            (smoothed.iter().map(|(_, v)| v * v).sum::<f64>() / smoothed.len() as f64).sqrt();
        let max_peak_db = smoothed.iter().map(|(_, v)| *v).fold(0.0, f64::max);
        let max_dip_db = smoothed.iter().map(|(_, v)| *v).fold(0.0, f64::min);
        let (problem_count, modal_q_estimate) = count_problems_and_estimate_q(&smoothed);

        Self {
            octave_span,
            rms_db,
            max_peak_db,
            max_dip_db,
            problem_count,
            modal_q_estimate,
        }
    }
}

pub(super) fn resolve_auto_optimizer_config(
    curve: &Curve,
    base: &OptimizerConfig,
    context: &AutoOptimizerContext,
) -> OptimizerConfig {
    let Some(auto) = base.auto_optimizer.as_ref().filter(|cfg| cfg.enabled) else {
        return base.clone();
    };

    let mut resolved = base.clone();
    let analysis = CurveAnalysis::analyze(
        curve,
        context.effective_min_freq,
        context.effective_max_freq,
    );

    if auto.filter_count {
        resolved.num_filters = choose_filter_count(base, context, &analysis);
    }

    if auto.q_bounds {
        apply_q_bounds(&mut resolved, context, &analysis);
    }

    if auto.gain_bounds {
        apply_gain_bounds(&mut resolved, context, &analysis);
    }

    log::info!(
        "  Auto optimizer: filters={}, Q=[{:.2}, {:.1}], gain=[{:+.1}, {:+.1}] dB",
        resolved.num_filters,
        resolved.min_q,
        resolved.max_q,
        resolved.min_db,
        resolved.max_db,
    );

    resolved
}

pub(super) fn resolved_schroeder_hz(config: &OptimizerConfig) -> Option<f64> {
    config
        .schroeder_split
        .as_ref()
        .filter(|split| split.enabled)
        .map(|split| {
            split
                .room_dimensions
                .as_ref()
                .map(|dims| dims.schroeder_frequency())
                .unwrap_or(split.schroeder_freq)
        })
        .or_else(|| {
            config
                .decomposed_correction
                .as_ref()
                .filter(|dc| dc.enabled)
                .map(|dc| dc.schroeder_freq)
        })
}

fn choose_filter_count(
    base: &OptimizerConfig,
    context: &AutoOptimizerContext,
    analysis: &CurveAnalysis,
) -> usize {
    let auto = base
        .auto_optimizer
        .as_ref()
        .expect("auto optimizer enabled");
    let max_filters = auto.max_filters.max(1);
    let min_filters = auto.min_filters.max(1).min(max_filters);

    let filters_per_octave = if context.is_sub_channel { 2.0 } else { 1.15 };
    let mut estimate = (analysis.octave_span * filters_per_octave).ceil() as usize;

    estimate = estimate.max(analysis.problem_count.saturating_add(1));

    if analysis.rms_db >= 7.0 {
        estimate += 3;
    } else if analysis.rms_db >= 4.5 {
        estimate += 2;
    } else if analysis.rms_db >= 2.5 {
        estimate += 1;
    }

    if context
        .schroeder_hz
        .is_some_and(|sf| sf > context.effective_min_freq && sf < context.effective_max_freq)
    {
        estimate += 1;
    }

    if context.target_tilt_active {
        estimate += 1;
    }

    if context.broadband_enabled && estimate > min_filters {
        estimate -= 1;
    }

    if base
        .schroeder_split
        .as_ref()
        .is_some_and(|split| split.enabled)
    {
        estimate = estimate.max(2);
    }

    estimate.clamp(min_filters, max_filters)
}

fn apply_q_bounds(
    resolved: &mut OptimizerConfig,
    context: &AutoOptimizerContext,
    analysis: &CurveAnalysis,
) {
    let modal_q = analysis
        .modal_q_estimate
        .unwrap_or(if context.is_sub_channel { 8.0 } else { 5.0 })
        .clamp(3.0, AUTO_SUB_MAX_Q);

    resolved.min_q = AUTO_MIN_Q;

    if let Some(split) = resolved
        .schroeder_split
        .as_mut()
        .filter(|split| split.enabled)
    {
        let low_max_q = if context.is_sub_channel {
            modal_q.clamp(6.0, AUTO_SUB_MAX_Q)
        } else {
            modal_q.clamp(4.0, AUTO_SUB_MAX_Q)
        };
        let high_max_q = if context.broadband_enabled {
            AUTO_HIGH_FREQ_MAX_Q_BROADBAND
        } else {
            AUTO_HIGH_FREQ_MAX_Q
        };

        split.low_freq_config.min_q = AUTO_MIN_Q;
        split.low_freq_config.max_q = low_max_q;
        split.high_freq_config.max_q = high_max_q;
        resolved.max_q = low_max_q.max(high_max_q);
    } else if context.is_sub_channel || context.effective_max_freq <= 250.0 {
        resolved.max_q = modal_q.clamp(6.0, AUTO_SUB_MAX_Q);
    } else if context.schroeder_hz.is_some() || context.effective_max_freq <= 2000.0 {
        resolved.max_q = modal_q.clamp(4.0, AUTO_MAIN_MAX_Q);
    } else {
        resolved.max_q = 3.0;
    }
}

fn apply_gain_bounds(
    resolved: &mut OptimizerConfig,
    context: &AutoOptimizerContext,
    analysis: &CurveAnalysis,
) {
    let boost_cap = if context.is_sub_channel {
        AUTO_SUB_MAX_BOOST_DB
    } else {
        AUTO_MAIN_MAX_BOOST_DB
    };
    let cut_cap = if context.is_sub_channel {
        AUTO_SUB_MAX_CUT_DB
    } else {
        AUTO_MAIN_MAX_CUT_DB
    };

    let needed_boost = (-analysis.max_dip_db + 1.0).clamp(2.0, boost_cap);
    let needed_cut = (analysis.max_peak_db + 2.0).clamp(6.0, cut_cap);

    resolved.max_db = needed_boost;
    resolved.min_db = -needed_cut;

    if let Some(split) = resolved
        .schroeder_split
        .as_mut()
        .filter(|split| split.enabled)
    {
        split.low_freq_config.allow_boost = false;
        split.low_freq_config.max_db = Some(needed_cut);
    }

    if resolved.max_boost_envelope.is_none()
        && let Some(zero_boost_until) = zero_boost_until_hz(context)
    {
        let release = (zero_boost_until * 1.25).min(context.effective_max_freq);
        resolved.max_boost_envelope = Some(vec![
            (context.effective_min_freq, 0.0),
            (zero_boost_until, 0.0),
            (release, needed_boost),
            (context.effective_max_freq, needed_boost),
        ]);
    }
}

fn zero_boost_until_hz(context: &AutoOptimizerContext) -> Option<f64> {
    let mut cutoff: Option<f64> = None;
    for candidate in [context.schroeder_hz, context.detected_f3_hz]
        .into_iter()
        .flatten()
    {
        if candidate > context.effective_min_freq && candidate < context.effective_max_freq {
            cutoff = Some(cutoff.map_or(candidate, |current| current.max(candidate)));
        }
    }
    cutoff
}

fn smooth_deviations(deviations: &[(f64, f64)], window_size: usize) -> Vec<(f64, f64)> {
    let half = window_size / 2;
    (0..deviations.len())
        .map(|i| {
            let start = i.saturating_sub(half);
            let end = (i + half + 1).min(deviations.len());
            let value = deviations[start..end]
                .iter()
                .map(|(_, value)| value)
                .sum::<f64>()
                / (end - start) as f64;
            (deviations[i].0, value)
        })
        .collect()
}

fn count_problems_and_estimate_q(smoothed: &[(f64, f64)]) -> (usize, Option<f64>) {
    let mut count = 0;
    let mut last_freq: Option<f64> = None;
    let mut q_estimate: Option<f64> = None;

    for i in 1..smoothed.len().saturating_sub(1) {
        let (freq, value) = smoothed[i];
        let prev = smoothed[i - 1].1;
        let next = smoothed[i + 1].1;
        let is_peak = value > prev && value > next && value >= 3.0;
        let is_dip = value < prev && value < next && value <= -4.0;

        if !is_peak && !is_dip {
            continue;
        }

        if last_freq.is_some_and(|last| (freq / last).log2() < 1.0 / 6.0) {
            continue;
        }

        count += 1;
        last_freq = Some(freq);

        if let Some(q) = estimate_feature_q(smoothed, i) {
            q_estimate = Some(q_estimate.map_or(q, |current| current.max(q)));
        }
    }

    (count, q_estimate)
}

fn estimate_feature_q(smoothed: &[(f64, f64)], center_idx: usize) -> Option<f64> {
    let center_freq = smoothed[center_idx].0;
    let center_abs = smoothed[center_idx].1.abs();
    if center_freq <= 0.0 || center_abs < 3.0 {
        return None;
    }

    let half_abs = center_abs * 0.5;
    let mut low = center_freq;
    for i in (0..center_idx).rev() {
        low = smoothed[i].0;
        if smoothed[i].1.abs() <= half_abs {
            break;
        }
    }

    let mut high = center_freq;
    for &(freq, value) in smoothed.iter().skip(center_idx + 1) {
        high = freq;
        if value.abs() <= half_abs {
            break;
        }
    }

    let bandwidth = high - low;
    if bandwidth > 0.0 {
        Some(center_freq / bandwidth)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roomeq::types::{AutoOptimizerConfig, SchroederSplitConfig};
    use ndarray::Array1;

    fn test_curve() -> Curve {
        let freqs: Vec<f64> = (0..220)
            .map(|i| 20.0 * 10.0_f64.powf(i as f64 / 90.0))
            .collect();
        let spl: Vec<f64> = freqs
            .iter()
            .map(|f| {
                let mode = 9.0 * (-((*f / 55.0).log2()).powi(2) / 0.01).exp();
                let dip = -5.0 * (-((*f / 120.0).log2()).powi(2) / 0.03).exp();
                mode + dip
            })
            .collect();

        Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: None,
            ..Default::default()
        }
    }

    fn context() -> AutoOptimizerContext {
        AutoOptimizerContext {
            is_sub_channel: false,
            effective_min_freq: 20.0,
            effective_max_freq: 1600.0,
            detected_f3_hz: Some(65.0),
            schroeder_hz: Some(250.0),
            target_tilt_active: false,
            broadband_enabled: false,
        }
    }

    #[test]
    fn disabled_auto_optimizer_keeps_config_unchanged() {
        let curve = test_curve();
        let config = OptimizerConfig::default();
        let resolved = resolve_auto_optimizer_config(&curve, &config, &context());

        assert_eq!(resolved.num_filters, config.num_filters);
        assert_eq!(resolved.min_q, config.min_q);
        assert_eq!(resolved.max_q, config.max_q);
        assert_eq!(resolved.max_boost_envelope, config.max_boost_envelope);
    }

    #[test]
    fn auto_optimizer_sets_count_q_and_boost_envelope() {
        let curve = test_curve();
        let config = OptimizerConfig {
            num_filters: 3,
            auto_optimizer: Some(AutoOptimizerConfig {
                enabled: true,
                max_filters: 10,
                ..Default::default()
            }),
            schroeder_split: Some(SchroederSplitConfig {
                enabled: true,
                ..Default::default()
            }),
            ..Default::default()
        };

        let resolved = resolve_auto_optimizer_config(&curve, &config, &context());
        let split = resolved.schroeder_split.as_ref().unwrap();

        assert!((2..=10).contains(&resolved.num_filters));
        assert!(split.low_freq_config.max_q > split.high_freq_config.max_q);
        assert!(!split.low_freq_config.allow_boost);
        assert!(split.low_freq_config.max_db.unwrap() >= 6.0);

        let envelope = resolved.max_boost_envelope.as_ref().unwrap();
        assert_eq!(envelope[0].1, 0.0);
        assert_eq!(envelope[1].1, 0.0);
        assert!(envelope[2].1 > 0.0);
    }

    #[test]
    fn sub_channel_auto_allows_more_filters_and_q() {
        let curve = test_curve();
        let config = OptimizerConfig {
            auto_optimizer: Some(AutoOptimizerConfig {
                enabled: true,
                max_filters: 12,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut ctx = context();
        ctx.is_sub_channel = true;
        ctx.effective_max_freq = 200.0;

        let resolved = resolve_auto_optimizer_config(&curve, &config, &ctx);

        assert!(resolved.num_filters >= 4);
        assert!(resolved.max_q >= 6.0);
        assert!(resolved.max_db <= AUTO_SUB_MAX_BOOST_DB);
    }
}
