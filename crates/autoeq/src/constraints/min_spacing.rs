use crate::cli::PeqModel;
use crate::param_utils;

/// Data needed by the nonlinear spacing constraint callback.
#[derive(Clone, Copy)]
pub struct SpacingConstraintData {
    /// Minimum required spacing between filter centers in octaves
    pub min_spacing_oct: f64,
    /// PEQ model to determine parameter layout
    pub peq_model: PeqModel,
}

/// Compute the minimum octave spacing between any pair of filters.
///
/// Returns `f64::INFINITY` if there are 0 or 1 filters.
fn compute_min_octave_spacing(xs: &[f64], peq_model: PeqModel, min_freq_hz: f64) -> f64 {
    let n = param_utils::num_filters(xs, peq_model);
    if n <= 1 {
        return f64::INFINITY;
    }

    let mut min_dist = f64::INFINITY;
    for i in 0..n {
        let fi = param_utils::freq_from_log10_clamped(
            param_utils::get_filter_params(xs, i, peq_model).freq,
            min_freq_hz,
        );
        for j in (i + 1)..n {
            let fj = param_utils::freq_from_log10_clamped(
                param_utils::get_filter_params(xs, j, peq_model).freq,
                min_freq_hz,
            );
            let d_oct = (fj / fi).log2().abs();
            min_dist = min_dist.min(d_oct);
        }
    }
    min_dist
}

/// Inequality constraint: spacing between any pair of center freqs must be at least min_spacing_oct.
/// Returns fc(x) = min_spacing_oct - min_pair_distance. Feasible when <= 0.
pub fn constraint_spacing(
    x: &[f64],
    _grad: Option<&mut [f64]>,
    data: &mut SpacingConstraintData,
) -> f64 {
    if data.min_spacing_oct <= 0.0 {
        return 0.0;
    }
    let min_dist = compute_min_octave_spacing(x, data.peq_model, 1e-6);
    if min_dist.is_finite() {
        data.min_spacing_oct - min_dist
    } else {
        0.0
    }
}

/// Compute spacing constraint violation from parameter vector
///
/// Calculates how much the closest pair of filters violates the minimum
/// spacing requirement in octaves.
///
/// # Arguments
/// * `xs` - Parameter vector (layout depends on PeqModel)
/// * `peq_model` - PEQ model that determines parameter layout
/// * `min_spacing_oct` - Minimum required spacing in octaves
///
/// # Returns
/// Spacing violation amount (0.0 if no violation or disabled)
pub fn viol_spacing_from_xs(xs: &[f64], peq_model: PeqModel, min_spacing_oct: f64) -> f64 {
    if min_spacing_oct <= 0.0 {
        return 0.0;
    }
    let min_dist = compute_min_octave_spacing(xs, peq_model, 1e-9);
    if min_dist.is_finite() {
        (min_spacing_oct - min_dist).max(0.0)
    } else {
        0.0
    }
}
