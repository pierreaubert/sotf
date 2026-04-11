//! Flat loss via ERB + band-weighted perceptual error.
//!
//! Previously this module implemented a 2-band RMS split at 3 kHz.
//! It now delegates to [`combined_weighted_loss`] for better perceptual
//! relevance. See `loss/enhanced_weights.rs` for the underlying math.

use super::enhanced_weights::{FrequencyBandWeights, combined_weighted_loss};
use ndarray::Array1;

/// Default blend for `flat_loss`: 70% ERB-weighted, 30% band-weighted.
///
/// ERB (Equivalent Rectangular Bandwidth) models cochlear filter bandwidth
/// and is the standard perceptual frequency scale. The 30% band component
/// adds the bass/mid/treble bias defined by `FrequencyBandWeights::default`.
const DEFAULT_FLAT_ERB_WEIGHT: f64 = 0.7;
const DEFAULT_FLAT_BAND_WEIGHT: f64 = 0.3;

/// Compute the flat loss as an ERB + band weighted combination of `error`
/// values inside `[min_freq, max_freq]`.
///
/// Values at frequencies outside the range are excluded before the loss
/// is computed. Returns `f64::INFINITY` if no points remain in range.
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `error` - Error values at each frequency point
/// * `min_freq` - Minimum frequency in Hz (inclusive)
/// * `max_freq` - Maximum frequency in Hz (inclusive)
pub fn flat_loss(freqs: &Array1<f64>, error: &Array1<f64>, min_freq: f64, max_freq: f64) -> f64 {
    let (f_in, e_in) = filter_in_range(freqs, error, min_freq, max_freq);
    if f_in.is_empty() {
        return f64::INFINITY;
    }
    let bands = FrequencyBandWeights::default();
    combined_weighted_loss(
        &f_in,
        &e_in,
        &bands,
        DEFAULT_FLAT_ERB_WEIGHT,
        DEFAULT_FLAT_BAND_WEIGHT,
    )
}

fn filter_in_range(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
) -> (Array1<f64>, Array1<f64>) {
    assert_eq!(freqs.len(), error.len());
    let mut f_out = Vec::new();
    let mut e_out = Vec::new();
    for (&f, &e) in freqs.iter().zip(error.iter()) {
        if f >= min_freq && f <= max_freq {
            f_out.push(f);
            e_out.push(e);
        }
    }
    (Array1::from(f_out), Array1::from(e_out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn zero_error_gives_zero_loss() {
        let freqs = array![100.0, 1000.0, 10000.0];
        let err = array![0.0, 0.0, 0.0];
        assert!(flat_loss(&freqs, &err, 20.0, 20000.0).abs() < 1e-12);
    }

    #[test]
    fn loss_scales_monotonically_with_error_magnitude() {
        let freqs = array![100.0, 500.0, 2000.0, 8000.0];
        let err_small: Array1<f64> = array![0.5, 0.5, 0.5, 0.5];
        let err_large: Array1<f64> = array![2.0, 2.0, 2.0, 2.0];
        let small = flat_loss(&freqs, &err_small, 20.0, 20000.0);
        let large = flat_loss(&freqs, &err_large, 20.0, 20000.0);
        assert!(
            large > small,
            "larger error should give larger loss (got {small} vs {large})"
        );
    }

    #[test]
    fn loss_scales_linearly_when_errors_are_uniformly_scaled() {
        let freqs = array![100.0, 1000.0, 10000.0];
        let err = array![1.0, 1.0, 1.0];
        let err_scaled = array![3.0, 3.0, 3.0];
        let a = flat_loss(&freqs, &err, 20.0, 20000.0);
        let b = flat_loss(&freqs, &err_scaled, 20.0, 20000.0);
        assert!(
            (b - 3.0 * a).abs() < 1e-9,
            "loss should scale linearly (a={a}, b={b})"
        );
    }

    #[test]
    fn range_filter_excludes_out_of_range_points() {
        // With range 200..=10000 the 50 Hz and 15 kHz entries are filtered
        // out, so changing their error values must not affect the loss.
        let freqs = array![50.0, 500.0, 5000.0, 15000.0];
        let err_a = array![10.0, 0.5, 0.5, 10.0];
        let err_b = array![0.0, 0.5, 0.5, 0.0];
        let a = flat_loss(&freqs, &err_a, 200.0, 10000.0);
        let b = flat_loss(&freqs, &err_b, 200.0, 10000.0);
        assert!(
            (a - b).abs() < 1e-9,
            "out-of-range points must not affect loss (a={a}, b={b})"
        );
    }

    #[test]
    fn empty_range_returns_infinity() {
        let freqs = array![100.0, 200.0, 500.0];
        let err = array![1.0, 1.0, 1.0];
        assert!(flat_loss(&freqs, &err, 5000.0, 10000.0).is_infinite());
    }
}
