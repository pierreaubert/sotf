//! Level normalization logic for room EQ

use autoeq::Curve;
use ndarray::Array1;

/// Compute average SPL over a frequency range
pub fn compute_average_spl(curve: &Curve, freq_min: f64, freq_max: f64) -> f64 {
    let mut sum = 0.0;
    let mut count = 0;

    for (i, &freq) in curve.freq.iter().enumerate() {
        if freq >= freq_min && freq <= freq_max {
            sum += curve.spl[i];
            count += 1;
        }
    }

    if count > 0 {
        sum / (count as f64)
    } else {
        0.0
    }
}

/// Sum two curves (in linear domain)
pub fn sum_curves(curve1: &Curve, curve2: &Curve) -> Curve {
    assert_eq!(curve1.freq.len(), curve2.freq.len(), "Curves must have same frequency points");

    let freq = curve1.freq.clone();
    let mut spl = Array1::zeros(freq.len());

    for i in 0..freq.len() {
        // Convert dB to linear, sum, convert back to dB
        let linear1 = 10.0_f64.powf(curve1.spl[i] / 20.0);
        let linear2 = 10.0_f64.powf(curve2.spl[i] / 20.0);
        let sum_linear = linear1 + linear2;
        spl[i] = 20.0 * sum_linear.log10();
    }

    Curve { freq, spl }
}

/// Normalize a curve to a target average SPL
pub fn normalize_curve_to_average(curve: &Curve, target_avg: f64, freq_min: f64, freq_max: f64) -> (Curve, f64) {
    let current_avg = compute_average_spl(curve, freq_min, freq_max);
    let gain_db = target_avg - current_avg;

    let mut normalized = curve.clone();
    normalized.spl = &normalized.spl + gain_db;

    (normalized, gain_db)
}

/// Compute gain needed to normalize a curve to a target average
pub fn compute_normalization_gain(curve: &Curve, target_avg: f64, freq_min: f64, freq_max: f64) -> f64 {
    let current_avg = compute_average_spl(curve, freq_min, freq_max);
    target_avg - current_avg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_average_spl() {
        let freq = Array1::from_vec(vec![100.0, 200.0, 300.0, 400.0, 500.0]);
        let spl = Array1::from_vec(vec![80.0, 82.0, 84.0, 86.0, 88.0]);
        let curve = Curve { freq, spl };

        let avg = compute_average_spl(&curve, 100.0, 500.0);
        assert!((avg - 84.0).abs() < 0.01);

        let avg_partial = compute_average_spl(&curve, 200.0, 400.0);
        assert!((avg_partial - 84.0).abs() < 0.01);
    }

    #[test]
    fn test_normalize_curve() {
        let freq = Array1::from_vec(vec![100.0, 200.0, 300.0]);
        let spl = Array1::from_vec(vec![80.0, 80.0, 80.0]);
        let curve = Curve { freq, spl };

        let (normalized, gain) = normalize_curve_to_average(&curve, 85.0, 100.0, 300.0);

        assert!((gain - 5.0).abs() < 0.01);
        assert!((normalized.spl[0] - 85.0).abs() < 0.01);
    }
}
