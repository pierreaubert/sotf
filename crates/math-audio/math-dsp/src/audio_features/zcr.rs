//! Zero-crossing rate descriptor.

use super::utils::{normalize, number_crossings};

const MAX_VALUE: f32 = 1.0;
const MIN_VALUE: f32 = 0.0;

/// Compute the zero-crossing rate of a signal.
///
/// Returns a normalized value in [-1, 1].
pub fn compute_zcr(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -1.0;
    }
    let crossings = number_crossings(samples);
    let rate = crossings as f32 / samples.len() as f32;
    normalize(rate, MIN_VALUE, MAX_VALUE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zcr_silence() {
        let chunk = vec![0.0; 1024];
        assert_eq!(-1.0, compute_zcr(&chunk));
    }

    #[test]
    fn test_zcr_max_crossings() {
        let one_chunk = [-1.0, 1.0];
        let chunks: Vec<f32> = std::iter::repeat_n(one_chunk.iter(), 512)
            .flatten()
            .cloned()
            .collect();
        let val = compute_zcr(&chunks);
        assert!((0.9980469 - val).abs() < 0.001);
    }
}

#[test]
fn test_zcr_empty() {
    assert_eq!(compute_zcr(&[]), -1.0);
}
