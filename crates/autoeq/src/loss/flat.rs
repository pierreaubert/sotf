//! Flat loss functions and the underlying weighted MSE primitives.

use ndarray::{Array1, Zip};

/// Default bass/treble split frequency in Hz.
///
/// This is a heuristic split point - bass frequencies are weighted more heavily
/// because room acoustics are typically more problematic in the low frequencies.
/// The value of 3000 Hz is based on practical experience but is somewhat arbitrary.
pub(super) const DEFAULT_BASS_TREBLE_SPLIT_HZ: f64 = 3000.0;

/// Compute the flat loss as the weighted MSE of `error` across `freqs`.
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `error` - Error values at each frequency point
/// * `min_freq` - Minimum frequency in Hz (inclusive)
/// * `max_freq` - Maximum frequency in Hz (inclusive)
///
/// # Returns
/// * Weighted error value computed only for frequencies within [min_freq, max_freq]
pub fn flat_loss(freqs: &Array1<f64>, error: &Array1<f64>, min_freq: f64, max_freq: f64) -> f64 {
    weighted_mse(freqs, error, min_freq, max_freq)
}

/// Compute weighted mean squared error with frequency-dependent weighting within a frequency range
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `error` - Error values at each frequency point
/// * `min_freq` - Minimum frequency in Hz (inclusive)
/// * `max_freq` - Maximum frequency in Hz (inclusive)
///
/// # Returns
/// * Weighted error value computed only for frequencies within [min_freq, max_freq]
///
/// # Details
/// Filters frequencies to the specified range, then computes RMS error separately
/// for frequencies below and above the bass/treble split (default 3000 Hz),
/// with higher weight given to the lower frequency band.
/// If the frequency range excludes all data points, returns 0.0.
pub(super) fn weighted_mse(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
) -> f64 {
    weighted_mse_with_split(
        freqs,
        error,
        min_freq,
        max_freq,
        DEFAULT_BASS_TREBLE_SPLIT_HZ,
    )
}

/// Compute weighted mean squared error with configurable bass/treble split frequency
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `error` - Error values at each frequency point
/// * `min_freq` - Minimum frequency in Hz (inclusive)
/// * `max_freq` - Maximum frequency in Hz (inclusive)
/// * `bass_treble_split_hz` - Frequency in Hz that divides bass and treble bands
///
/// # Returns
/// * Weighted error value computed only for frequencies within [min_freq, max_freq]
fn weighted_mse_with_split(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
    bass_treble_split_hz: f64,
) -> f64 {
    // Create masks for frequency bands using ndarray's vectorized operations
    let _in_range = freqs.mapv(|f| f >= min_freq && f <= max_freq);
    let bass_band = freqs.mapv(|f| f < bass_treble_split_hz && f >= min_freq && f <= max_freq);
    let treble_band = freqs.mapv(|f| f >= bass_treble_split_hz && f >= min_freq && f <= max_freq);

    // Count points in each band
    let n1: usize = bass_band.iter().filter(|&&b| b).count();
    let n2: usize = treble_band.iter().filter(|&&b| b).count();

    if n1 == 0 && n2 == 0 {
        return f64::INFINITY;
    }

    // Compute squared errors only for valid points
    let squared_errors = error.mapv(|e| e * e);

    let ss1: f64 = Zip::from(&bass_band)
        .and(&squared_errors)
        .fold(0.0, |acc, &mask, &err| if mask { acc + err } else { acc });

    let ss2: f64 = Zip::from(&treble_band)
        .and(&squared_errors)
        .fold(0.0, |acc, &mask, &err| if mask { acc + err } else { acc });

    let err1 = if n1 > 0 {
        (ss1 / n1 as f64).sqrt()
    } else {
        0.0
    };
    let err2 = if n2 > 0 {
        (ss2 / n2 as f64).sqrt()
    } else {
        0.0
    };
    match (n1 > 0, n2 > 0) {
        (true, true) => err1 + err2 / 3.0,
        (true, false) => err1,
        (false, true) => err2,
        (false, false) => f64::INFINITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_weighted_mse_basic() {
        // Two points below 3k, two points above 3k with unit error
        let freqs = array![1000.0, 2000.0, 4000.0, 8000.0];
        let err = array![1.0, 1.0, 1.0, 1.0];
        let v = weighted_mse(&freqs, &err, 100.0, 10000.0); // Full range
        // RMS below = 1, RMS above = 1 -> total = 1 + 1/3 = 1.333...
        assert!((v - (1.0 + 1.0 / 3.0)).abs() < 1e-12, "got {}", v);
    }

    #[test]
    fn test_weighted_mse_empty_upper_segment() {
        // All freqs below 3k -> upper RMS = 0
        let freqs = array![100.0, 200.0, 500.0];
        let err = array![2.0, 2.0, 2.0]; // squares: 4,4,4 -> mean=4 -> rms=2
        let v = weighted_mse(&freqs, &err, 50.0, 10000.0); // Full range
        assert!((v - 2.0).abs() < 1e-12, "got {}", v);
    }

    #[test]
    fn test_weighted_mse_scaling() {
        // Different errors below and above to verify weighting
        let freqs = array![1000.0, 1500.0, 4000.0, 6000.0];
        let err = array![2.0, 2.0, 3.0, 3.0];
        // below RMS = sqrt((4+4)/2)=2, above RMS = sqrt((9+9)/2)=3
        let v = weighted_mse(&freqs, &err, 500.0, 10000.0); // Full range
        let expected = 2.0 + 3.0 / 3.0; // 3.0
        assert!((v - expected).abs() < 1e-12, "got {}", v);
    }

    #[test]
    fn test_weighted_mse_frequency_filtering() {
        // Test that frequency filtering works correctly
        let freqs = array![100.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
        let err = array![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 1.0];

        // Filter to only include 1kHz-4kHz range
        let v = weighted_mse(&freqs, &err, 1000.0, 4000.0);
        // Only frequencies 1000, 2000, 4000 should be included with errors 2, 2, 3
        // All below 3kHz: RMS = sqrt((4+4)/2) = 2
        // 4kHz above 3kHz: RMS = sqrt(9/1) = 3
        let expected = 2.0 + 3.0 / 3.0; // 3.0
        assert!(
            (v - expected).abs() < 1e-12,
            "got {} expected {}",
            v,
            expected
        );
    }

    #[test]
    fn test_weighted_mse_no_frequencies_in_range() {
        // Test edge case where no frequencies are in range
        let freqs = array![100.0, 200.0, 500.0];
        let err = array![2.0, 3.0, 1.0];

        // Filter to range that excludes all frequencies
        let v = weighted_mse(&freqs, &err, 1000.0, 5000.0);
        assert!(
            v.is_infinite(),
            "Should return INFINITY when no frequencies in range"
        );
    }

    #[test]
    fn test_weighted_mse_partial_range() {
        // Test filtering that includes only some frequencies
        let freqs = array![100.0, 1000.0, 2000.0, 4000.0, 8000.0];
        let err = array![1.0, 2.0, 2.0, 3.0, 4.0];

        // Filter to 500-3000 range (should include 1000, 2000)
        let v = weighted_mse(&freqs, &err, 500.0, 3000.0);
        // Only 1000, 2000 with errors 2, 2 - both below 3kHz
        // RMS = sqrt((4+4)/2) = 2.0, no high freq component
        let expected = 2.0;
        assert!(
            (v - expected).abs() < 1e-12,
            "got {} expected {}",
            v,
            expected
        );
    }

    #[test]
    fn test_flat_loss_frequency_filtering() {
        // Test that flat_loss correctly delegates to weighted_mse with frequency bounds
        let freqs = array![100.0, 1000.0, 2000.0, 4000.0, 8000.0];
        let err = array![1.0, 2.0, 2.0, 3.0, 4.0];

        let v1 = flat_loss(&freqs, &err, 1000.0, 4000.0);
        let v2 = weighted_mse(&freqs, &err, 1000.0, 4000.0);

        assert_eq!(v1, v2, "flat_loss should equal weighted_mse");
    }

    #[test]
    fn test_frequency_filtering_boundary_conditions() {
        // Test inclusive boundaries
        let freqs = array![1000.0, 2000.0, 3000.0];
        let err = array![1.0, 1.0, 1.0];

        // Include only exact boundary frequencies
        let v = weighted_mse(&freqs, &err, 1000.0, 3000.0);
        // All three frequencies should be included
        // 1000, 2000 are below 3kHz threshold (n1=2, ss1=2)
        // 3000 is >= 3kHz threshold (n2=1, ss2=1)
        // err1 = sqrt(2/2) = 1.0, err2 = sqrt(1/1) = 1.0
        // result = 1.0 + 1.0/3.0 = 1.333...
        let expected = 1.0 + 1.0 / 3.0;
        assert!(
            (v - expected).abs() < 1e-12,
            "got {} expected {}",
            v,
            expected
        );

        // Exclude boundary frequencies
        let v2 = weighted_mse(&freqs, &err, 1001.0, 2999.0);
        // Only 2000 Hz should be included (below 3kHz threshold)
        // err1 = sqrt(1/1) = 1.0, err2 = 0
        // result = 1.0 + 0/3.0 = 1.0
        let expected2 = 1.0;
        assert!(
            (v2 - expected2).abs() < 1e-12,
            "got {} expected {}",
            v2,
            expected2
        );
    }

    #[test]
    fn test_weighted_mse_treble_only() {
        // All frequencies above 3kHz — bass band is empty
        let freqs = Array1::from_vec(vec![4000.0, 8000.0, 16000.0]);
        let error = Array1::from_vec(vec![2.0, 2.0, 2.0]);
        let result = weighted_mse(&freqs, &error, 4000.0, 16000.0);
        // With all treble, should return err2 (not err2/3)
        assert!(
            (result - 2.0).abs() < 1e-10,
            "treble-only loss should be full RMS, got {}",
            result
        );
    }

    #[test]
    fn test_weighted_mse_bass_only() {
        // All frequencies below 3kHz — treble band is empty
        let freqs = Array1::from_vec(vec![100.0, 500.0, 2000.0]);
        let error = Array1::from_vec(vec![3.0, 3.0, 3.0]);
        let result = weighted_mse(&freqs, &error, 100.0, 2000.0);
        // With all bass, should return err1
        assert!(
            (result - 3.0).abs() < 1e-10,
            "bass-only loss should be full RMS, got {}",
            result
        );
    }

    #[test]
    fn test_weighted_mse_both_bands() {
        // Mix of bass and treble
        let freqs = Array1::from_vec(vec![100.0, 1000.0, 5000.0, 10000.0]);
        let error = Array1::from_vec(vec![3.0, 3.0, 6.0, 6.0]);
        let result = weighted_mse(&freqs, &error, 100.0, 10000.0);
        // err1 (bass RMS) = 3.0, err2 (treble RMS) = 6.0
        // result = 3.0 + 6.0/3.0 = 5.0
        assert!(
            (result - 5.0).abs() < 1e-10,
            "two-band loss incorrect, got {}",
            result
        );
    }
}
