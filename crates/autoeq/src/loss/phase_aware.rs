//! Phase-aware optimization for room equalization.
//!
//! Research:
//! "Phase-Coherent Equalization for Loudspeakers" (Klein et al.)
//! "The Effect of Phase on Loudspeaker Sound Quality" (Zacharov)

use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

/// Compute the phase of a complex frequency response
pub fn compute_phase(response: &Array1<Complex64>) -> Array1<f64> {
    response.mapv(|c| c.arg().to_degrees())
}

/// Unwrap phase to avoid discontinuities.
/// Handles arbitrary multiples of 360° (not just single wraps),
/// equivalent to NumPy's `np.unwrap` applied to degree-valued phase.
pub fn unwrap_phase_degrees(phase: &Array1<f64>) -> Array1<f64> {
    let mut unwrapped = Array1::zeros(phase.len());
    if phase.is_empty() {
        return unwrapped;
    }

    unwrapped[0] = phase[0];
    let mut offset = 0.0;

    for i in 1..phase.len() {
        let diff = phase[i] - phase[i - 1];
        // Round to nearest multiple of 360° and subtract it
        let wraps = (diff / 360.0).round();
        offset -= wraps * 360.0;
        unwrapped[i] = phase[i] + offset;
    }

    unwrapped
}

/// Compute group delay from phase response
pub fn compute_group_delay(freqs: &Array1<f64>, phase: &Array1<f64>) -> Array1<f64> {
    let mut gd = Array1::zeros(phase.len());

    for i in 1..phase.len() {
        let d_freq = freqs[i] - freqs[i - 1];
        if d_freq > 0.0 {
            let d_phase = (phase[i] - phase[i - 1]).to_radians();
            gd[i] = -d_phase / (2.0 * PI * d_freq) * 1000.0; // ms
        }
    }
    gd[0] = gd[1]; // Copy first value

    gd
}

/// Compute phase deviation from a target phase
pub fn phase_deviation(
    measured_phase: &Array1<f64>,
    target_phase: &Array1<f64>,
    freqs: &Array1<f64>,
) -> f64 {
    assert_eq!(measured_phase.len(), target_phase.len());
    assert_eq!(measured_phase.len(), freqs.len());

    let unwrapped_measured = unwrap_phase_degrees(measured_phase);
    let unwrapped_target = unwrap_phase_degrees(target_phase);

    // Compute difference
    let diff = &unwrapped_measured - &unwrapped_target;

    // RMS of phase deviation
    let n = diff.len() as f64;
    let sum_sq: f64 = diff.iter().map(|d| d * d).sum();
    (sum_sq / n).sqrt()
}

/// Combined magnitude and phase loss
pub fn magnitude_phase_loss(magnitude_error: f64, phase_error: f64, phase_weight: f64) -> f64 {
    magnitude_error + phase_weight * phase_error
}

/// Reconstruct minimum phase from magnitude response
///
/// When phase data is not available, we can reconstruct minimum phase
/// which represents the "most compact" impulse response.
pub fn reconstruct_minimum_phase(magnitude_db: &Array1<f64>) -> Array1<f64> {
    // Convert dB to linear magnitude
    // let magnitude_linear: Array1<f64> = magnitude_db.mapv(|db| 10.0_f64.powf(db / 20.0));

    // For simplicity, return minimum phase approximation
    // (this is a placeholder - full implementation requires Hilbert transform)
    magnitude_db.mapv(|_| 0.0) // Zero phase = minimum phase
}

/// Compute the impulse response duration (related to phase linearity)
pub fn impulse_response_duration(freqs: &Array1<f64>, phase: &Array1<f64>) -> f64 {
    let gd = compute_group_delay(freqs, phase);

    // Measure deviation of group delay (preringing indicator)
    let mean_gd: f64 = gd.iter().sum::<f64>() / gd.len() as f64;
    let gd_variance: f64 = gd.iter().map(|g| (g - mean_gd).powi(2)).sum::<f64>() / gd.len() as f64;

    // Return standard deviation of group delay in ms
    gd_variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    #[test]
    fn compute_phase_basic() {
        let response = Array1::from(vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 1.0),
            Complex64::new(-1.0, 0.0),
        ]);
        let phase = compute_phase(&response);
        assert!((phase[0] - 0.0).abs() < 1e-12);
        assert!((phase[1] - 90.0).abs() < 1e-12);
        assert!((phase[2] - 180.0).abs() < 1e-12, "got {}", phase[2]);
    }

    #[test]
    fn compute_group_delay_linear_phase() {
        let freqs = Array1::from(vec![100.0, 200.0]);
        let phase = Array1::from(vec![0.0, -90.0]);
        let gd = compute_group_delay(&freqs, &phase);
        // d_phase = -90 deg = -PI/2 rad
        // gd = -(-PI/2) / (2*PI*100) * 1000 = 2.5 ms
        assert!((gd[1] - 2.5).abs() < 1e-9, "gd[1] = {}", gd[1]);
        assert_eq!(gd[0], gd[1]);
    }

    #[test]
    fn phase_deviation_rms() {
        let measured = Array1::from(vec![0.0, 90.0]);
        let target = Array1::from(vec![0.0, 0.0]);
        let freqs = Array1::from(vec![100.0, 200.0]);
        let dev = phase_deviation(&measured, &target, &freqs);
        let expected = ((0.0f64.powi(2) + 90.0f64.powi(2)) / 2.0).sqrt();
        assert!((dev - expected).abs() < 1e-9);
    }

    #[test]
    fn magnitude_phase_loss_combination() {
        let loss = magnitude_phase_loss(1.5, 2.0, 0.5);
        assert!((loss - 2.5).abs() < 1e-12);
    }

    #[test]
    fn impulse_response_duration_constant_gd() {
        // Uniform frequency spacing + constant phase step -> constant group delay
        let freqs = Array1::from(vec![100.0, 200.0, 300.0]);
        let phase = Array1::from(vec![0.0, -90.0, -180.0]);
        let duration = impulse_response_duration(&freqs, &phase);
        assert!(
            duration.abs() < 1e-9,
            "duration should be ~0, got {}",
            duration
        );
    }
}
