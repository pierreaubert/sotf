use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::response;
use crate::roomeq::eq;
use crate::roomeq::types::{
    LowFreqFilterConfig, OptimizerConfig, SchroederSplitConfig, TargetCurveConfig, TargetShape,
};
use log::{debug, info};
use math_audio_iir_fir::Biquad;

/// Optimize EQ with optional Schroeder frequency split.
///
/// If the optimizer config has an enabled Schroeder split, performs two-pass
/// optimization with different Q constraints. Otherwise falls back to standard
/// single-pass optimization.
///
/// Historically used by the system-config workflows; after Phase 3 those
/// workflows route per-channel EQ through `process_single_speaker`, which
/// applies the Schroeder split itself inside `prepare_single_channel_eq`.
/// Kept as an internal convenience wrapper for tests and future callers.
#[allow(dead_code)]
pub(in crate::roomeq) fn optimize_eq_with_optional_schroeder(
    curve: &Curve,
    optimizer: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
) -> std::result::Result<(Vec<Biquad>, f64), Box<dyn std::error::Error>> {
    if let Some(schroeder_config) = &optimizer.schroeder_split
        && schroeder_config.enabled
    {
        let schroeder_freq = if let Some(ref dims) = schroeder_config.room_dimensions {
            dims.schroeder_frequency()
        } else {
            schroeder_config.schroeder_freq
        };
        info!(
            "  Schroeder split: optimizing below {:.1} Hz with max_q={:.1}, above with max_q={:.1}",
            schroeder_freq,
            schroeder_config.low_freq_config.max_q,
            schroeder_config.high_freq_config.max_q
        );

        let (low_filters, high_filters) =
            optimize_with_schroeder_split(curve, optimizer, schroeder_config, sample_rate)
                .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;

        let mut combined = low_filters;
        combined.extend(high_filters);
        // Loss is approximate (sum of both passes) — not used for scoring
        let loss = 0.0;
        Ok((combined, loss))
    } else {
        eq::optimize_channel_eq(curve, optimizer, target_config, sample_rate)
    }
}

/// Optimize EQ with Schroeder frequency split
///
/// Performs two-pass optimization with different Q constraints:
/// - Below Schroeder: high-Q narrow filters for room modes
/// - Above Schroeder: low-Q broad filters for tonal adjustment
pub(in crate::roomeq) fn optimize_with_schroeder_split(
    curve: &Curve,
    optimizer: &OptimizerConfig,
    schroeder_config: &SchroederSplitConfig,
    sample_rate: f64,
) -> Result<(Vec<Biquad>, Vec<Biquad>)> {
    let schroeder_freq = if let Some(ref dims) = schroeder_config.room_dimensions {
        dims.schroeder_frequency()
    } else {
        schroeder_config.schroeder_freq
    };

    let low_config = &schroeder_config.low_freq_config;
    let high_config = &schroeder_config.high_freq_config;

    // Determine filter allocation (roughly proportional to frequency range)
    let total_filters = optimizer.num_filters;
    let log_range_total = (optimizer.max_freq / optimizer.min_freq).log2();
    let log_range_low = (schroeder_freq / optimizer.min_freq).max(1.0).log2();
    let low_ratio = log_range_low / log_range_total;

    let low_filters = ((total_filters as f64 * low_ratio).round() as usize)
        .max(1)
        .min(total_filters - 1);
    let high_filters = total_filters - low_filters;

    debug!(
        "  Schroeder split: {} filters below {:.1}Hz, {} filters above",
        low_filters, schroeder_freq, high_filters
    );

    // Each sub-pass gets the full maxeval budget. With fewer filters (lower
    // dimensionality) the optimizer converges faster, so the same budget is
    // adequate for each pass independently.
    // When target_tilt is active, the optimizer works on a tilt-adjusted curve
    // where following the tilt may require both boosts and cuts. Allow limited
    // boost (half the configured max) to give the optimizer enough freedom.
    let has_non_flat_target = optimizer
        .target_response
        .as_ref()
        .is_some_and(|tr| tr.shape != TargetShape::Flat);

    let (low_min_db, low_max_db) = low_freq_gain_bounds(optimizer, low_config, has_non_flat_target);
    let low_optimizer = OptimizerConfig {
        num_filters: low_filters,
        min_freq: optimizer.min_freq,
        max_freq: schroeder_freq,
        min_q: low_config.min_q,
        max_q: low_config.max_q,
        min_db: low_min_db,
        max_db: low_max_db,
        ..optimizer.clone()
    };

    let (low_eq_filters, _) = eq::optimize_channel_eq(
        curve,
        &low_optimizer,
        None, // No additional target for split optimization
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("Low-frequency EQ optimization failed: {}", e),
    })?;

    // High frequency optimization (above Schroeder)
    let high_optimizer = OptimizerConfig {
        num_filters: high_filters,
        min_freq: schroeder_freq,
        max_freq: optimizer.max_freq,
        min_q: optimizer.min_q.max(0.3), // Ensure minimum Q for broad filters
        max_q: high_config.max_q,
        ..optimizer.clone()
    };

    // Apply low-freq correction first, then optimize high-freq on residual
    let low_resp =
        response::compute_peq_complex_response(&low_eq_filters, &curve.freq, sample_rate);
    let curve_with_low_correction = response::apply_complex_response(curve, &low_resp);

    let (high_eq_filters, _) = eq::optimize_channel_eq(
        &curve_with_low_correction,
        &high_optimizer,
        None,
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("High-frequency EQ optimization failed: {}", e),
    })?;

    // Post-optimization Q clamping: NLopt COBYLA can violate bounds slightly (or
    // significantly with low maxeval). Enforce the configured Q constraints on the
    // returned filters to guarantee the Schroeder split invariant.
    let low_eq_filters = clamp_filter_q(low_eq_filters, low_config.min_q, low_config.max_q);
    let high_eq_filters =
        clamp_filter_q(high_eq_filters, optimizer.min_q.max(0.3), high_config.max_q);

    Ok((low_eq_filters, high_eq_filters))
}

fn low_freq_gain_bounds(
    optimizer: &OptimizerConfig,
    low_config: &LowFreqFilterConfig,
    has_non_flat_target: bool,
) -> (f64, f64) {
    if let Some(configured_max) = low_config.max_db {
        let configured_abs = configured_max.abs();
        let max_db = if low_config.allow_boost {
            configured_abs
        } else {
            0.0
        };
        return (-configured_abs, max_db);
    }

    if low_config.allow_boost {
        (optimizer.min_db, optimizer.max_db)
    } else if has_non_flat_target {
        (optimizer.min_db, (optimizer.max_db / 2.0).min(3.0))
    } else {
        (optimizer.min_db, 0.0)
    }
}

/// Clamp Q values of filters to [min_q, max_q], recomputing biquad coefficients.
pub(in crate::roomeq) fn clamp_filter_q(
    filters: Vec<Biquad>,
    min_q: f64,
    max_q: f64,
) -> Vec<Biquad> {
    filters
        .into_iter()
        .map(|f| {
            let clamped_q = f.q.clamp(min_q, max_q);
            if (clamped_q - f.q).abs() > 1e-6 {
                debug!(
                    "  Clamping filter Q at {:.0} Hz: {:.2} -> {:.2}",
                    f.freq, f.q, clamped_q
                );
                Biquad::new(f.filter_type, f.freq, f.srate, clamped_q, f.db_gain)
            } else {
                f
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_low_freq_max_db_respects_cuts_only_setting() {
        let optimizer = OptimizerConfig {
            min_db: -12.0,
            max_db: 4.0,
            ..Default::default()
        };
        let low_config = LowFreqFilterConfig {
            allow_boost: false,
            max_db: Some(14.0),
            ..Default::default()
        };

        assert_eq!(
            low_freq_gain_bounds(&optimizer, &low_config, false),
            (-14.0, 0.0)
        );
    }

    #[test]
    fn explicit_low_freq_max_db_allows_symmetric_range_when_boost_enabled() {
        let optimizer = OptimizerConfig::default();
        let low_config = LowFreqFilterConfig {
            allow_boost: true,
            max_db: Some(14.0),
            ..Default::default()
        };

        assert_eq!(
            low_freq_gain_bounds(&optimizer, &low_config, false),
            (-14.0, 14.0)
        );
    }
}
