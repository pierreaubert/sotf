//! Broadband slope estimation for measurement curves.
//!
//! Fits a least-squares line through SPL vs log₂(freq) and returns the
//! slope in dB/octave.  Used by `TargetShape::FromMeasurement` to
//! derive the target tilt from the measurement itself rather than a
//! hardcoded value.

use crate::Curve;

/// Estimate the broadband slope of a measurement curve in dB/octave.
///
/// Uses ordinary least-squares regression of SPL vs log₂(freq),
/// restricted to the `[min_freq, max_freq]` window to avoid
/// room-mode contamination at LF and measurement noise / rolloff at HF.
///
/// Returns `None` if fewer than 2 points fall inside the window.
pub fn estimate_slope_db_per_octave(curve: &Curve, min_freq: f64, max_freq: f64) -> Option<f64> {
    // Collect (log2_freq, spl) pairs inside the window.
    let mut sum_x = 0.0_f64;
    let mut sum_y = 0.0_f64;
    let mut sum_xx = 0.0_f64;
    let mut sum_xy = 0.0_f64;
    let mut n = 0_u64;

    for (freq, spl) in curve.freq.iter().zip(curve.spl.iter()) {
        let f = *freq;
        let y = *spl;
        if f < min_freq || f > max_freq || f <= 0.0 {
            continue;
        }
        let x = f.log2();
        sum_x += x;
        sum_y += y;
        sum_xx += x * x;
        sum_xy += x * y;
        n += 1;
    }

    if n < 2 {
        log::warn!(
            "Slope estimation failed: only {} point(s) in [{:.1}, {:.1}] Hz window",
            n,
            min_freq,
            max_freq
        );
        return None;
    }

    let nf = n as f64;
    let denom = nf * sum_xx - sum_x * sum_x;
    if denom.abs() < 1e-30 {
        log::warn!(
            "Slope estimation failed: degenerate regression in [{:.1}, {:.1}] Hz window (denominator {:.3e})",
            min_freq,
            max_freq,
            denom
        );
        return None;
    }

    // slope = Σ(x - x̄)(y - ȳ) / Σ(x - x̄)²  =  (n·Σxy − Σx·Σy) / (n·Σx² − (Σx)²)
    let slope = (nf * sum_xy - sum_x * sum_y) / denom;
    Some(slope)
}

/// Default lower bound for the regression window (Hz).
pub const DEFAULT_SLOPE_MIN_FREQ: f64 = 200.0;
/// Default upper bound for the regression window (Hz).
pub const DEFAULT_SLOPE_MAX_FREQ: f64 = 10_000.0;

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    fn make_curve(freqs: &[f64], spl: &[f64]) -> Curve {
        Curve {
            freq: Array1::from_vec(freqs.to_vec()),
            spl: Array1::from_vec(spl.to_vec()),
            phase: None,
            ..Default::default()
        }
    }

    #[test]
    fn flat_curve_has_zero_slope() {
        // 200 Hz – 10 kHz, flat at 80 dB
        let freqs: Vec<f64> = (0..100)
            .map(|i| 200.0 * (50.0_f64).powf(i as f64 / 99.0))
            .collect();
        let spl: Vec<f64> = vec![80.0; 100];
        let curve = make_curve(&freqs, &spl);

        let slope = estimate_slope_db_per_octave(&curve, 200.0, 10_000.0).unwrap();
        assert!(slope.abs() < 0.01, "expected ~0, got {slope}");
    }

    #[test]
    fn known_negative_slope() {
        // -1.0 dB/octave from 200 Hz, reference = 200 Hz at 80 dB
        let ref_freq = 200.0_f64;
        let ref_spl = 80.0;
        let slope_target = -1.0;

        let freqs: Vec<f64> = (0..100)
            .map(|i| 200.0 * (50.0_f64).powf(i as f64 / 99.0))
            .collect();
        let spl: Vec<f64> = freqs
            .iter()
            .map(|f| ref_spl + slope_target * (f / ref_freq).log2())
            .collect();
        let curve = make_curve(&freqs, &spl);

        let slope = estimate_slope_db_per_octave(&curve, 200.0, 10_000.0).unwrap();
        assert!(
            (slope - slope_target).abs() < 0.01,
            "expected {slope_target}, got {slope}"
        );
    }

    #[test]
    fn known_positive_slope() {
        let ref_freq = 200.0_f64;
        let ref_spl = 70.0;
        let slope_target = 0.5;

        let freqs: Vec<f64> = (0..100)
            .map(|i| 200.0 * (50.0_f64).powf(i as f64 / 99.0))
            .collect();
        let spl: Vec<f64> = freqs
            .iter()
            .map(|f| ref_spl + slope_target * (f / ref_freq).log2())
            .collect();
        let curve = make_curve(&freqs, &spl);

        let slope = estimate_slope_db_per_octave(&curve, 200.0, 10_000.0).unwrap();
        assert!(
            (slope - slope_target).abs() < 0.01,
            "expected {slope_target}, got {slope}"
        );
    }

    #[test]
    fn points_outside_window_are_ignored() {
        // Put a huge spike at 50 Hz (below window) — should not affect slope
        let ref_freq = 200.0_f64;
        let ref_spl = 80.0;
        let slope_target = -0.5;

        let mut freqs = vec![50.0]; // outside window
        let mut spl = vec![120.0]; // huge spike
        for i in 0..100 {
            let f = 200.0 * (50.0_f64).powf(i as f64 / 99.0);
            freqs.push(f);
            spl.push(ref_spl + slope_target * (f / ref_freq).log2());
        }
        let curve = make_curve(&freqs, &spl);

        let slope = estimate_slope_db_per_octave(&curve, 200.0, 10_000.0).unwrap();
        assert!(
            (slope - slope_target).abs() < 0.01,
            "expected {slope_target}, got {slope}"
        );
    }

    #[test]
    fn too_few_points_returns_none() {
        let curve = make_curve(&[500.0], &[80.0]);
        assert!(estimate_slope_db_per_octave(&curve, 200.0, 10_000.0).is_none());
    }

    #[test]
    fn empty_curve_returns_none() {
        let curve = make_curve(&[], &[]);
        assert!(estimate_slope_db_per_octave(&curve, 200.0, 10_000.0).is_none());
    }
}
