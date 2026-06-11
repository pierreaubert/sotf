use crate::cea2034::Curve;
use ndarray::Array1;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Psychoacoustic variable smoothing configuration
///
/// Different frequency ranges benefit from different smoothing levels:
/// - Low frequencies (< 100 Hz): Fine resolution (1/48 octave) to preserve room modes
/// - High frequencies (> 1 kHz): Coarse resolution (1/6 octave) to ignore comb filtering
/// - Transition region (100 Hz - 1 kHz): Gradual interpolation between the two
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PsychoacousticSmoothingConfig {
    /// Smoothing resolution below low_freq (bands per octave, e.g., 48 for 1/48 octave)
    pub low_freq_n: usize,
    /// Smoothing resolution above high_freq (bands per octave, e.g., 6 for 1/6 octave)
    pub high_freq_n: usize,
    /// Lower transition frequency in Hz (default: 100 Hz)
    pub low_freq: f64,
    /// Upper transition frequency in Hz (default: 1000 Hz)
    pub high_freq: f64,
}

impl Default for PsychoacousticSmoothingConfig {
    fn default() -> Self {
        Self {
            low_freq_n: 48,    // 1/48 octave below 100 Hz (preserve room modes)
            high_freq_n: 6,    // 1/6 octave above 1 kHz (ignore comb filtering)
            low_freq: 100.0,   // Transition starts at 100 Hz
            high_freq: 1000.0, // Transition ends at 1 kHz
        }
    }
}

/// Apply psychoacoustic variable smoothing
///
/// This applies frequency-dependent smoothing that matches human perception:
/// - Fine resolution at low frequencies to preserve audible room modes
/// - Coarse resolution at high frequencies where comb filtering is inaudible
///
/// # Arguments
/// * `curve` - Input frequency response curve
/// * `config` - Smoothing configuration (use `Default::default()` for standard psychoacoustic settings)
///
/// # Returns
/// * Smoothed curve with frequency-appropriate resolution
///
/// # Example
/// ```
/// use autoeq::read::{smooth_psychoacoustic, PsychoacousticSmoothingConfig};
/// use autoeq::cea2034::Curve;
/// use ndarray::Array1;
///
/// // Create a dummy curve
/// let freqs = Array1::from(vec![20.0, 100.0, 1000.0, 10000.0]);
/// let spl = Array1::from(vec![80.0, 82.0, 78.0, 75.0]);
/// let curve = Curve { freq: freqs, spl, phase: None, ..Default::default() };
///
/// let config = PsychoacousticSmoothingConfig::default();
/// let smoothed = smooth_psychoacoustic(&curve, &config);
/// ```
pub fn smooth_psychoacoustic(curve: &Curve, config: &PsychoacousticSmoothingConfig) -> Curve {
    let freqs = &curve.freq;
    let values = &curve.spl;
    let mut out = Array1::zeros(values.len());

    for i in 0..freqs.len() {
        let f = freqs[i].max(1e-12);

        // Calculate frequency-dependent smoothing factor (N for 1/N octave)
        let n = calculate_variable_n(f, config);

        // Calculate window bounds
        let half_win = 2.0_f64.powf(1.0 / (2.0 * n));
        let lo = f / half_win;
        let hi = f * half_win;

        // Average values within window
        let mut sum = 0.0;
        let mut cnt = 0usize;
        for j in 0..freqs.len() {
            let fj = freqs[j];
            if fj >= lo && fj <= hi {
                sum += values[j];
                cnt += 1;
            }
        }
        out[i] = if cnt > 0 { sum / cnt as f64 } else { values[i] };
    }

    Curve {
        freq: curve.freq.clone(),
        spl: out,
        phase: None,
        ..Default::default()
    }
}

/// Calculate variable smoothing N based on frequency
///
/// Uses logarithmic interpolation in the transition region for smooth blending.
fn calculate_variable_n(freq: f64, config: &PsychoacousticSmoothingConfig) -> f64 {
    if freq <= config.low_freq {
        // Below transition: fine resolution (e.g., 1/48 octave)
        config.low_freq_n as f64
    } else if freq >= config.high_freq {
        // Above transition: coarse resolution (e.g., 1/6 octave)
        config.high_freq_n as f64
    } else {
        // Transition region: logarithmic interpolation
        // t goes from 0 (at low_freq) to 1 (at high_freq) in log space
        let log_low = config.low_freq.ln();
        let log_high = config.high_freq.ln();
        let log_f = freq.ln();
        let t = (log_f - log_low) / (log_high - log_low);

        // Interpolate N in log space for smoother transition
        let log_n_low = (config.low_freq_n as f64).ln();
        let log_n_high = (config.high_freq_n as f64).ln();
        (log_n_low + t * (log_n_high - log_n_low)).exp()
    }
}

/// Simple 1/N-octave smoothing: for each frequency f_i, average values whose
/// frequency lies within [f_i * 2^(-1/(2N)), f_i * 2^(1/(2N))]
///
/// # Arguments
/// * `freqs` - Frequency array
/// * `values` - SPL values to smooth
/// * `n` - Number of bands per octave
///
/// # Returns
/// * Smoothed SPL values
pub fn smooth_one_over_n_octave(curve: &Curve, n: usize) -> Curve {
    let freqs = &curve.freq;
    let values = &curve.spl;
    let n = n.max(1);
    let half_win = (2.0_f64).powf(1.0 / (2.0 * n as f64));
    let mut out = Array1::zeros(values.len());
    for i in 0..freqs.len() {
        let f = freqs[i].max(1e-12);
        let lo = f / half_win;
        let hi = f * half_win;
        let mut sum = 0.0;
        let mut cnt = 0usize;
        for j in 0..freqs.len() {
            let fj = freqs[j];
            if fj >= lo && fj <= hi {
                sum += values[j];
                cnt += 1;
            }
        }
        out[i] = if cnt > 0 { sum / cnt as f64 } else { values[i] };
    }
    Curve {
        freq: curve.freq.clone(),
        spl: out,
        phase: None,
        ..Default::default()
    }
}

/// Apply Gaussian smoothing to a signal
///
/// # Arguments
/// * `signal` - Input signal to smooth
/// * `sigma` - Standard deviation of Gaussian kernel
///
/// # Returns
/// Smoothed signal
pub fn smooth_gaussian(signal: &Array1<f64>, sigma: f64) -> Array1<f64> {
    if sigma <= 0.0 {
        return signal.clone();
    }

    let n = signal.len();
    let mut result = Array1::zeros(n);

    // Calculate kernel size (3 sigma on each side is usually sufficient)
    let kernel_half_size = (3.0 * sigma).ceil() as usize;
    let kernel_size = 2 * kernel_half_size + 1;

    // Pre-calculate Gaussian kernel
    let mut kernel = Vec::with_capacity(kernel_size);
    let mut kernel_sum = 0.0;

    for i in 0..kernel_size {
        let x = i as f64 - kernel_half_size as f64;
        let weight = (-0.5 * (x / sigma).powi(2)).exp();
        kernel.push(weight);
        kernel_sum += weight;
    }

    // Normalize kernel
    for weight in kernel.iter_mut() {
        *weight /= kernel_sum;
    }

    // Apply convolution with boundary handling
    for i in 0..n {
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        for (j, &kernel_weight) in kernel.iter().enumerate() {
            let sample_idx = i as isize + j as isize - kernel_half_size as isize;

            if sample_idx >= 0 && sample_idx < n as isize {
                weighted_sum += signal[sample_idx as usize] * kernel_weight;
                weight_sum += kernel_weight;
            }
        }

        result[i] = if weight_sum > 0.0 {
            weighted_sum / weight_sum
        } else {
            signal[i]
        };
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::{clamp_positive_only, smooth_one_over_n_octave};
    use ndarray::Array1;

    #[test]
    fn clamp_positive_only_clamps_only_positive_side() {
        let arr = Array1::from(vec![-15.0, -1.0, 0.0, 1.0, 10.0, 25.0]);
        let out = clamp_positive_only(&arr, 12.0);
        assert_eq!(out.to_vec(), vec![-15.0, -1.0, 0.0, 1.0, 10.0, 12.0]);
    }

    #[test]
    fn smooth_one_over_n_octave_basic_monotonic() {
        use crate::cea2034::Curve;
        // Simple check: with N large, window small -> output close to input
        let freqs = Array1::from(vec![100.0, 200.0, 400.0, 800.0]);
        let vals = Array1::from(vec![0.0, 1.0, 0.0, -1.0]);
        let curve = Curve {
            freq: freqs,
            spl: vals.clone(),
            phase: None,
            ..Default::default()
        };
        let out = smooth_one_over_n_octave(&curve, 24);
        // Expect no drastic change
        for (o, v) in out.spl.iter().zip(vals.iter()) {
            assert!((o - v).abs() <= 0.5);
        }
    }

    #[test]
    fn test_calculate_variable_n_below_transition() {
        let config = PsychoacousticSmoothingConfig::default();
        // Below 100 Hz should use low_freq_n (48)
        let n = calculate_variable_n(50.0, &config);
        assert!((n - 48.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_variable_n_above_transition() {
        let config = PsychoacousticSmoothingConfig::default();
        // Above 1000 Hz should use high_freq_n (6)
        let n = calculate_variable_n(2000.0, &config);
        assert!((n - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_variable_n_in_transition() {
        let config = PsychoacousticSmoothingConfig::default();
        // At geometric mean of 100 and 1000 (≈316 Hz), N should be between 6 and 48
        let n = calculate_variable_n(316.0, &config);
        assert!(
            n > 6.0 && n < 48.0,
            "N at 316 Hz should be between 6 and 48, got {}",
            n
        );
    }

    #[test]
    fn test_psychoacoustic_smoothing_preserves_length() {
        let freqs = Array1::linspace(20.0, 20000.0, 100);
        let vals = Array1::zeros(100);
        let curve = Curve {
            freq: freqs,
            spl: vals,
            phase: None,
            ..Default::default()
        };
        let config = PsychoacousticSmoothingConfig::default();
        let out = smooth_psychoacoustic(&curve, &config);
        assert_eq!(out.freq.len(), curve.freq.len());
        assert_eq!(out.spl.len(), curve.spl.len());
    }

    #[test]
    fn test_psychoacoustic_smoothing_flat_input_stays_flat() {
        // Log-spaced frequencies from 20 Hz to 20 kHz
        let freqs: Vec<f64> = (0..100)
            .map(|i| 20.0 * (1000.0_f64).powf(i as f64 / 99.0))
            .collect();
        let freqs = Array1::from(freqs);
        let vals = Array1::from_elem(100, 80.0); // Flat 80 dB
        let curve = Curve {
            freq: freqs,
            spl: vals,
            phase: None,
            ..Default::default()
        };
        let config = PsychoacousticSmoothingConfig::default();
        let out = smooth_psychoacoustic(&curve, &config);

        // Flat input should remain flat (within floating point precision)
        for &v in out.spl.iter() {
            assert!((v - 80.0).abs() < 0.01, "Expected 80.0, got {}", v);
        }
    }

    #[test]
    fn smooth_gaussian_zero_sigma_returns_clone() {
        let signal = Array1::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let result = smooth_gaussian(&signal, 0.0);
        assert_eq!(result.to_vec(), signal.to_vec());
    }

    #[test]
    fn smooth_gaussian_flat_signal_stays_flat() {
        let signal = Array1::from_elem(20, 5.0);
        let result = smooth_gaussian(&signal, 2.0);
        for &v in result.iter() {
            assert!(
                (v - 5.0).abs() < 1e-9,
                "flat signal should stay flat, got {}",
                v
            );
        }
    }

    #[test]
    fn smooth_gaussian_reduces_peak() {
        let signal = Array1::from_vec(vec![0.0, 0.0, 10.0, 0.0, 0.0]);
        let result = smooth_gaussian(&signal, 1.0);
        // Peak should be lower after smoothing
        let max_val = result.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(max_val < 10.0, "peak should be reduced by smoothing");
        assert!(max_val > 0.0, "peak should still be positive");
    }

    #[test]
    fn smooth_gaussian_preserves_length() {
        let signal = Array1::from_vec(vec![1.0, 5.0, 3.0, 8.0, 2.0]);
        let result = smooth_gaussian(&signal, 1.5);
        assert_eq!(result.len(), signal.len());
    }
}
