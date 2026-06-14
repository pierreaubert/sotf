use super::super::config::XtcPluginParams;
use rustfft::num_complex::Complex;

/// Apply frequency-dependent beta boosts (low/high) to the base beta value.
#[inline]
pub(super) fn apply_beta_freq_boosts(beta: f32, freq: f32, params: &XtcPluginParams) -> f32 {
    let low_factor =
        1.0 + params.beta_low_freq_boost * (1.0 / (1.0 + (-(100.0 - freq) / 30.0).exp()));
    let high_factor =
        1.0 + params.beta_high_freq_boost * (1.0 / (1.0 + (-(freq - 12000.0) / 1500.0).exp()));
    beta * low_factor * high_factor
}

/// Apply global effort constraint to the inverse filters.
///
/// Computes total filter effort E = Σ|W(f)|² across all bins and all 4 filters.
/// If E > E_max (derived from max_gain_db), scales all bins uniformly:
///   W(f) *= sqrt(E_max / E)
///
/// The per-bin tanh soft limiter is kept as a safety net with a higher threshold.
pub(crate) fn apply_effort_constraint(
    filter_ll: &mut [Complex<f32>],
    filter_lr: &mut [Complex<f32>],
    filter_rl: Option<&mut [Complex<f32>]>,
    filter_rr: Option<&mut [Complex<f32>]>,
    max_gain_linear: f32,
) {
    let num_bins = filter_ll.len();
    // Scale budget by number of active filter arrays (2 for symmetric, 4 for asymmetric/HRTF)
    let num_filters =
        2 + if filter_rl.is_some() { 1 } else { 0 } + if filter_rr.is_some() { 1 } else { 0 };
    let e_max = max_gain_linear * max_gain_linear * num_bins as f32 * num_filters as f32;

    // Compute total effort across all filter components
    let mut total_effort: f32 = 0.0;
    for bin in 0..num_bins {
        total_effort += filter_ll[bin].norm_sqr();
        total_effort += filter_lr[bin].norm_sqr();
    }
    if let Some(ref rl) = filter_rl {
        for c in rl.iter() {
            total_effort += c.norm_sqr();
        }
    }
    if let Some(ref rr) = filter_rr {
        for c in rr.iter() {
            total_effort += c.norm_sqr();
        }
    }

    if total_effort > e_max {
        let scale = (e_max / total_effort).sqrt();
        for c in filter_ll.iter_mut() {
            *c *= scale;
        }
        for c in filter_lr.iter_mut() {
            *c *= scale;
        }
        if let Some(rl) = filter_rl {
            for c in rl.iter_mut() {
                *c *= scale;
            }
        }
        if let Some(rr) = filter_rr {
            for c in rr.iter_mut() {
                *c *= scale;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustfft::num_complex::Complex;

    #[test]
    fn test_apply_effort_constraint_scales() {
        let mut ll = vec![Complex::new(10.0, 0.0); 10];
        let mut lr = vec![Complex::new(10.0, 0.0); 10];
        apply_effort_constraint(&mut ll, &mut lr, None, None, 1.0);
        assert!(ll[0].norm() < 10.0);
        assert!(lr[0].norm() < 10.0);
    }

    #[test]
    fn test_apply_effort_constraint_no_scale() {
        let mut ll = vec![Complex::new(0.1, 0.0); 10];
        let mut lr = vec![Complex::new(0.1, 0.0); 10];
        let original = ll[0].norm();
        apply_effort_constraint(&mut ll, &mut lr, None, None, 10.0);
        assert!((ll[0].norm() - original).abs() < 0.001);
    }

    #[test]
    fn test_apply_beta_freq_boosts() {
        let params = crate::config::XtcPluginParams::default();
        let beta = 0.01;
        let low = apply_beta_freq_boosts(beta, 50.0, &params);
        let mid = apply_beta_freq_boosts(beta, 1000.0, &params);
        let high = apply_beta_freq_boosts(beta, 20000.0, &params);
        assert!(low >= beta);
        assert!(high >= beta);
        assert!((mid - beta).abs() < 0.001);
    }
}
