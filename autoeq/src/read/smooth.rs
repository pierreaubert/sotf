use autoeq_cea2034::Curve;
use ndarray::Array1;

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
        use autoeq_cea2034::Curve;
        // Simple check: with N large, window small -> output close to input
        let freqs = Array1::from(vec![100.0, 200.0, 400.0, 800.0]);
        let vals = Array1::from(vec![0.0, 1.0, 0.0, -1.0]);
        let curve = Curve {
            freq: freqs,
            spl: vals.clone(),
        };
        let out = smooth_one_over_n_octave(&curve, 24);
        // Expect no drastic change
        for (o, v) in out.spl.iter().zip(vals.iter()) {
            assert!((o - v).abs() <= 0.5);
        }
    }
}
