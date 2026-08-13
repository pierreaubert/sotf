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

/// Limit loudspeaker effort independently at each frequency.
///
/// A coefficient-wise limit still permits a two-input output row to carry
/// twice the configured power. This enforces `Σ_input |W[out,input](f)|² <=
/// max_gain_linear²` for each loudspeaker row, which has a physical source-power
/// interpretation and remains effective after coefficient limiting.
pub(crate) fn apply_effort_constraint(
    filter_ll: &mut [Complex<f32>],
    filter_lr: &mut [Complex<f32>],
    filter_rl: Option<&mut [Complex<f32>]>,
    filter_rr: Option<&mut [Complex<f32>]>,
    max_gain_linear: f32,
) {
    let max_power = max_gain_linear * max_gain_linear;
    for (ll, lr) in filter_ll.iter_mut().zip(filter_lr.iter_mut()) {
        let power = ll.norm_sqr() + lr.norm_sqr();
        if power > max_power && power.is_finite() {
            let scale = (max_power / power).sqrt();
            *ll *= scale;
            *lr *= scale;
        }
    }
    if let (Some(rl), Some(rr)) = (filter_rl, filter_rr) {
        for (rl, rr) in rl.iter_mut().zip(rr.iter_mut()) {
            let power = rl.norm_sqr() + rr.norm_sqr();
            if power > max_power && power.is_finite() {
                let scale = (max_power / power).sqrt();
                *rl *= scale;
                *rr *= scale;
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
        assert!((ll[0].norm_sqr() + lr[0].norm_sqr() - 1.0).abs() < 1e-5);
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
