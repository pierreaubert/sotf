//! Frequency-response helpers for linear DSP models.

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use num_complex::Complex64;
use std::f64::consts::PI;

/// Complex response of an IIR biquad at `freq_hz`.
pub fn biquad_complex_response(biquad: &Biquad<f64>, freq_hz: f64) -> Complex64 {
    let (a1, a2, b0, b1, b2) = biquad.constants();
    let omega = 2.0 * PI * freq_hz / biquad.srate;
    let z_inv = Complex64::from_polar(1.0, -omega);
    let z_inv2 = z_inv * z_inv;
    let num = b0 + b1 * z_inv + b2 * z_inv2;
    let den = 1.0 + a1 * z_inv + a2 * z_inv2;
    num / den
}

/// Complex response of an FIR impulse at `freq_hz`.
pub fn fir_complex_response(taps: &[f64], freq_hz: f64, sample_rate: f64) -> Complex64 {
    if taps.is_empty() {
        return Complex64::new(1.0, 0.0);
    }
    let omega = -2.0 * PI * freq_hz / sample_rate;
    taps.iter()
        .enumerate()
        .fold(Complex64::new(0.0, 0.0), |acc, (idx, tap)| {
            acc + Complex64::from_polar(*tap, omega * idx as f64)
        })
}

/// Approximate the runtime LR4 crossover response for one output band.
///
/// The crossover plugin currently uses LR4 internally regardless of the
/// configured type string, so this helper models two cascaded second-order
/// Butterworth sections.
pub fn lr4_crossover_response(
    output: &str,
    cutoff_hz: f64,
    freq_hz: f64,
    sample_rate: f64,
) -> Result<Complex64, String> {
    if cutoff_hz <= 0.0 || !cutoff_hz.is_finite() {
        return Err(format!(
            "crossover cutoff must be positive, got {cutoff_hz}"
        ));
    }
    let filter_type = match output.to_ascii_lowercase().as_str() {
        "low" | "lowpass" | "lp" => BiquadFilterType::Lowpass,
        "high" | "highpass" | "hp" => BiquadFilterType::Highpass,
        "both" => return Ok(Complex64::new(1.0, 0.0)),
        other => return Err(format!("unsupported crossover output mode '{other}'")),
    };
    let section = Biquad::new(
        filter_type,
        cutoff_hz,
        sample_rate,
        std::f64::consts::FRAC_1_SQRT_2,
        0.0,
    );
    let response = biquad_complex_response(&section, freq_hz);
    Ok(response * response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fir_response_tracks_delay_phase() {
        let taps = [0.0, 1.0];
        let response = fir_complex_response(&taps, 12_000.0, 48_000.0);
        assert!(response.re.abs() < 1e-12);
        assert!((response.im + 1.0).abs() < 1e-12);
    }

    #[test]
    fn lr4_low_high_sum_near_unity_at_crossover() {
        let low = lr4_crossover_response("low", 1_000.0, 1_000.0, 48_000.0).unwrap();
        let high = lr4_crossover_response("high", 1_000.0, 1_000.0, 48_000.0).unwrap();
        assert!((low.norm() - 0.5).abs() < 0.05);
        assert!((high.norm() - 0.5).abs() < 0.05);
    }
}
