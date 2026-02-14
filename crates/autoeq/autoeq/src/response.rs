//! Frequency response calculation for filters

use crate::Curve;
use crate::iir::{Biquad, BiquadFilterType};
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

/// Compute complex frequency response of a list of Biquad filters
pub fn compute_peq_complex_response(
    filters: &[Biquad],
    freqs: &Array1<f64>,
    sample_rate: f64,
) -> Vec<Complex64> {
    freqs
        .iter()
        .map(|&f| {
            let w = 2.0 * PI * f / sample_rate;
            let z_inv = Complex64::from_polar(1.0, -w);
            let z_inv_2 = z_inv * z_inv;

            let mut total_h = Complex64::new(1.0, 0.0);

            for b in filters {
                let f0 = b.freq;
                let fs = b.srate;
                let q = b.q;
                let db = b.db_gain;

                let w0 = 2.0 * PI * f0 / fs;
                let cos_w0 = w0.cos();
                let sin_w0 = w0.sin();
                let alpha = sin_w0 / (2.0 * q);
                let big_a = 10.0_f64.powf(db / 40.0);

                let (b0, b1, b2, a0, a1, a2) = match b.filter_type {
                    BiquadFilterType::Peak => {
                        let b0 = 1.0 + alpha * big_a;
                        let b1 = -2.0 * cos_w0;
                        let b2 = 1.0 - alpha * big_a;
                        let a0 = 1.0 + alpha / big_a;
                        let a1 = -2.0 * cos_w0;
                        let a2 = 1.0 - alpha / big_a;
                        (b0, b1, b2, a0, a1, a2)
                    }
                    BiquadFilterType::Lowshelf => {
                        let sqrt_a = big_a.sqrt();
                        let b0 =
                            big_a * ((big_a + 1.0) - (big_a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
                        let b1 = 2.0 * big_a * ((big_a - 1.0) - (big_a + 1.0) * cos_w0);
                        let b2 =
                            big_a * ((big_a + 1.0) - (big_a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
                        let a0 = (big_a + 1.0) + (big_a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
                        let a1 = -2.0 * ((big_a - 1.0) + (big_a + 1.0) * cos_w0);
                        let a2 = (big_a + 1.0) - (big_a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;
                        (b0, b1, b2, a0, a1, a2)
                    }
                    BiquadFilterType::Highshelf => {
                        let sqrt_a = big_a.sqrt();
                        let b0 =
                            big_a * ((big_a + 1.0) + (big_a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
                        let b1 = -2.0 * big_a * ((big_a - 1.0) + (big_a + 1.0) * cos_w0);
                        let b2 =
                            big_a * ((big_a + 1.0) + (big_a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
                        let a0 = (big_a + 1.0) - (big_a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
                        let a1 = 2.0 * ((big_a - 1.0) - (big_a + 1.0) * cos_w0);
                        let a2 = (big_a + 1.0) - (big_a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;
                        (b0, b1, b2, a0, a1, a2)
                    }
                    BiquadFilterType::Lowpass => {
                        let b0 = (1.0 - cos_w0) / 2.0;
                        let b1 = 1.0 - cos_w0;
                        let b2 = (1.0 - cos_w0) / 2.0;
                        let a0 = 1.0 + alpha;
                        let a1 = -2.0 * cos_w0;
                        let a2 = 1.0 - alpha;
                        (b0, b1, b2, a0, a1, a2)
                    }
                    BiquadFilterType::Highpass => {
                        let b0 = (1.0 + cos_w0) / 2.0;
                        let b1 = -(1.0 + cos_w0);
                        let b2 = (1.0 + cos_w0) / 2.0;
                        let a0 = 1.0 + alpha;
                        let a1 = -2.0 * cos_w0;
                        let a2 = 1.0 - alpha;
                        (b0, b1, b2, a0, a1, a2)
                    }
                    // TODO: Add other filter types as needed (HighPass, LowPass, BandPass etc.)
                    // For now, identity
                    _ => (1.0, 0.0, 0.0, 1.0, 0.0, 0.0),
                };

                let num = Complex64::new(b0, 0.0)
                    + Complex64::new(b1, 0.0) * z_inv
                    + Complex64::new(b2, 0.0) * z_inv_2;
                let den = Complex64::new(a0, 0.0)
                    + Complex64::new(a1, 0.0) * z_inv
                    + Complex64::new(a2, 0.0) * z_inv_2;

                if den.norm_sqr() > 1e-12 {
                    total_h *= num / den;
                }
            }
            total_h
        })
        .collect()
}

/// Compute complex frequency response of FIR coefficients
pub fn compute_fir_complex_response(
    coeffs: &[f64],
    freqs: &Array1<f64>,
    sample_rate: f64,
) -> Vec<Complex64> {
    // Direct DFT calculation (O(N*M))
    // Appropriate for evaluation at specific log-spaced frequencies
    freqs
        .iter()
        .map(|&f| {
            let w = 2.0 * PI * f / sample_rate;
            let mut h = Complex64::new(0.0, 0.0);

            for (n, &val) in coeffs.iter().enumerate() {
                let angle = -w * n as f64;
                h += Complex64::from_polar(val, angle);
            }
            h
        })
        .collect()
}

/// Apply complex filter response (magnitude and phase) to a curve
pub fn apply_complex_response(curve: &Curve, response: &[Complex64]) -> Curve {
    let mut new_spl = Array1::zeros(curve.freq.len());
    let mut new_phase = Array1::zeros(curve.freq.len());
    let old_phase = curve.phase.as_ref();

    for i in 0..curve.freq.len() {
        let h = response[i];
        let h_mag_db = 20.0 * h.norm().log10();
        let h_phase_deg = h.arg().to_degrees();

        new_spl[i] = curve.spl[i] + h_mag_db;
        let p_in = old_phase.map(|p| p[i]).unwrap_or(0.0);
        new_phase[i] = p_in + h_phase_deg;
    }

    Curve {
        freq: curve.freq.clone(),
        spl: new_spl,
        phase: Some(new_phase),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;
    use std::f64::consts::PI;

    #[test]
    fn test_fir_response_impulse() {
        // Impulse response [1.0] should have flat magnitude (0 dB) and 0 phase
        let coeffs = vec![1.0];
        let freqs = Array1::from(vec![100.0, 1000.0, 10000.0]);
        let sample_rate = 48000.0;

        let response = compute_fir_complex_response(&coeffs, &freqs, sample_rate);

        for h in response {
            assert!((h.norm() - 1.0).abs() < 1e-10);
            assert!(h.arg().abs() < 1e-10);
        }
    }

    #[test]
    fn test_fir_response_delay() {
        // Delayed impulse [0.0, 1.0] should have flat magnitude and linear phase
        let coeffs = vec![0.0, 1.0];
        let freqs = Array1::from(vec![100.0, 1000.0, 10000.0]);
        let sample_rate = 48000.0;

        let response = compute_fir_complex_response(&coeffs, &freqs, sample_rate);

        for (i, h) in response.iter().enumerate() {
            assert!((h.norm() - 1.0).abs() < 1e-10);

            // Phase should be -w * delay
            // delay = 1 sample
            let w = 2.0 * PI * freqs[i] / sample_rate;
            let expected_phase = -w;

            // Normalize phase to [-pi, pi]
            let phase = h.arg();
            let mut diff = (phase - expected_phase).abs();
            while diff > PI {
                diff -= 2.0 * PI;
            }
            assert!(diff.abs() < 1e-10);
        }
    }
}
