//! Signal processing utilities
//!
//! This module contains functions for analyzing and processing signals,
//! including peak detection and related signal analysis operations.

use ndarray::Array1;

/// Find peaks in a signal using simple local maxima detection
///
/// # Arguments
/// * `signal` - Input signal to analyze
/// * `min_height` - Minimum height for peaks
/// * `min_distance` - Minimum distance between peaks (in samples)
///
/// # Returns
/// Indices of detected peaks
pub fn find_peaks(signal: &Array1<f64>, min_height: f64, min_distance: usize) -> Vec<usize> {
    let mut peaks = Vec::new();
    let n = signal.len();

    if n < 3 {
        return peaks;
    }

    // Find local maxima
    for i in 1..n - 1 {
        if signal[i] > signal[i - 1] && signal[i] > signal[i + 1] && signal[i] >= min_height {
            peaks.push(i);
        }
    }

    // Filter by minimum distance
    if min_distance > 0 {
        peaks = filter_peaks_by_distance(peaks, min_distance);
    }

    peaks
}

/// Filter peaks by minimum distance constraint
fn filter_peaks_by_distance(mut peaks: Vec<usize>, min_distance: usize) -> Vec<usize> {
    if peaks.is_empty() {
        return peaks;
    }

    peaks.sort_unstable();
    let mut filtered = vec![peaks[0]];

    for &peak in peaks.iter().skip(1) {
        if peak >= filtered.last().unwrap() + min_distance {
            filtered.push(peak);
        }
    }

    filtered
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_find_peaks() {
        let signal = Array1::from(vec![1.0, 3.0, 2.0, 5.0, 1.0, 4.0, 2.0]);
        let peaks = find_peaks(&signal, 2.5, 1);
        // Should find peaks at indices 1 (value 3.0), 3 (value 5.0), 5 (value 4.0)
        assert_eq!(peaks, vec![1, 3, 5]);
    }
}
