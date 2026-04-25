//! Shared frequency-grid validation helpers for RoomEQ.

use ndarray::Array1;

/// Return true when two frequency axes are equivalent by value.
///
/// A tiny relative tolerance allows harmless floating-point serialization
/// differences, but rejects genuinely shifted measurement grids.
pub fn same_frequency_grid(reference: &Array1<f64>, candidate: &Array1<f64>) -> bool {
    if reference.len() != candidate.len() {
        return false;
    }

    reference.iter().zip(candidate.iter()).all(|(&a, &b)| {
        let scale = a.abs().max(b.abs()).max(1.0);
        (a - b).abs() <= scale * 1e-6
    })
}

/// Return true when a frequency grid is finite and strictly increasing.
pub fn is_valid_frequency_grid(freq: &Array1<f64>) -> bool {
    freq.len() >= 2
        && freq.iter().all(|f| f.is_finite() && *f > 0.0)
        && freq.windows(2).into_iter().all(|w| w[1] > w[0])
}

/// Compute the common frequency span shared by a set of curves.
pub fn common_frequency_range<'a>(
    curves: impl IntoIterator<Item = &'a crate::Curve>,
) -> Option<(f64, f64)> {
    let mut min_freq = f64::NEG_INFINITY;
    let mut max_freq = f64::INFINITY;
    let mut saw_curve = false;

    for curve in curves {
        if !is_valid_frequency_grid(&curve.freq) {
            return None;
        }
        min_freq = min_freq.max(curve.freq[0]);
        max_freq = max_freq.min(curve.freq[curve.freq.len() - 1]);
        saw_curve = true;
    }

    if saw_curve && min_freq < max_freq {
        Some((min_freq, max_freq))
    } else {
        None
    }
}
