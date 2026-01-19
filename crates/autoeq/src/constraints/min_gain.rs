use super::super::cli::PeqModel;
use crate::param_utils;

/// Data needed by the nonlinear minimum gain constraint callback.
#[derive(Clone, Copy)]
pub struct MinGainConstraintData {
    /// Minimum required absolute gain in dB
    pub min_db: f64,
    /// PEQ model that defines the filter structure
    pub peq_model: PeqModel,
}

/// Inequality constraint: for Peak filters, require |gain| >= min_db OR |gain| = 0 (filter removal) (skip HP in HP+PK mode).
/// Returns fc(x) = max_i (min_db - |g_i|) over applicable filters, but allow |g_i| = 0. Feasible when <= 0.
pub fn constraint_min_gain(
    x: &[f64],
    _grad: Option<&mut [f64]>,
    data: &mut MinGainConstraintData,
) -> f64 {
    viol_min_gain_from_xs(x, data.peq_model, data.min_db)
}

/// Compute minimum gain constraint violation from parameter vector
///
/// Calculates the worst violation of minimum absolute gain requirement.
/// Only applies to peak filters (skips highpass filter in HP+PK mode).
/// Allows filter removal (gain = 0) as a valid option.
///
/// # Arguments
/// * `xs` - Parameter vector (layout depends on PeqModel)
/// * `peq_model` - PEQ model that defines the filter structure
/// * `min_db` - Minimum required absolute gain in dB
///
/// # Returns
/// Worst gain deficiency (0.0 if no violation or disabled)
pub fn viol_min_gain_from_xs(xs: &[f64], peq_model: PeqModel, min_db: f64) -> f64 {
    let n = param_utils::num_filters(xs, peq_model);
    if n == 0 {
        return 0.0;
    }
    let mut worst_short = 0.0_f64;
    for i in 0..n {
        // Skip non-peak filters based on the PEQ model and filter type
        let params = param_utils::get_filter_params(xs, i, peq_model);
        let filter_type = param_utils::determine_filter_type(i, n, peq_model, params.filter_type);

        // Skip non-peak filters
        use crate::iir::BiquadFilterType;
        let is_peak = matches!(filter_type, BiquadFilterType::Peak);
        if !is_peak {
            continue;
        }

        let g_abs = params.gain.abs();
        // Allow filter removal (gain = 0) or enforce minimum gain
        let short = if g_abs < 0.1 {
            // Effectively zero
            0.0 // No violation for removed filter
        } else {
            (min_db - g_abs).max(0.0) // Enforce minimum gain for active filters
        };
        if short > worst_short {
            worst_short = short;
        }
    }
    worst_short
}
