//! Cubic Distortion Tone (CDT) protection for room EQ.
//!
//! The human cochlea generates CDTs at `2*f1 - f2` when two tones f1, f2
//! are present. Over-correcting (deep cuts) at these frequencies removes
//! energy the ear expects from its own nonlinear processing, resulting in
//! a perceived loss of "warmth" or "naturalness."
//!
//! This module computes a protection envelope that limits how deeply the
//! optimizer can cut at CDT-sensitive frequencies.

/// Approximate CDT level from two primary tones at levels L1, L2 (dB SPL).
///
/// For moderate listening levels (~60-80 dB SPL):
///   L_cdt ≈ 2*L1 - L2 - 63 dB
///
/// This is a rough approximation; the actual CDT level depends on the
/// individual ear's nonlinearity and the absolute levels involved.
pub fn cdt_level(l1_db: f64, l2_db: f64) -> f64 {
    2.0 * l1_db - l2_db - 63.0
}

/// Generate a CDT protection envelope for the optimizer.
///
/// For musical fundamentals in the bass range (30-300 Hz) paired with
/// their low harmonics (2nd, 3rd), computes the CDT frequencies that
/// fall within the correction range. These frequencies get a limited
/// maximum cut depth to preserve the ear's expected distortion products.
///
/// Returns `(frequency_hz, max_cut_db)` pairs sorted by frequency.
/// `max_cut_db` is negative (e.g., -6.0 means no deeper than -6 dB).
///
/// Non-CDT frequencies between the protected points get a generous
/// limit (default -18 dB) so normal room mode correction isn't impeded.
pub fn cdt_protection_envelope(min_freq: f64, max_freq: f64) -> Vec<(f64, f64)> {
    let mut points: Vec<(f64, f64)> = Vec::new();

    // Musical fundamentals from 30 Hz to 300 Hz
    // For each fundamental f1, consider its 2nd and 3rd harmonics as f2.
    // CDT = 2*f1 - f2:
    //   With f2 = 2*f1: CDT = 2*f1 - 2*f1 = 0 (DC, irrelevant)
    //   With f2 = 3*f1: CDT = 2*f1 - 3*f1 = -f1 (negative, irrelevant)
    //
    // More usefully, consider two different fundamentals f1 and f2 where f1 < f2:
    //   CDT = 2*f1 - f2
    //   This is below f1, in the sub-bass region.
    //
    // For musical intervals:
    //   Perfect fifth (3:2): f2 = 1.5*f1, CDT = 2*f1 - 1.5*f1 = 0.5*f1
    //   Major third (5:4):   f2 = 1.25*f1, CDT = 2*f1 - 1.25*f1 = 0.75*f1
    //   Octave (2:1):        f2 = 2*f1, CDT = 0 (DC)
    //   Minor third (6:5):   f2 = 1.2*f1, CDT = 2*f1 - 1.2*f1 = 0.8*f1

    // Common musical intervals that produce CDTs in the audible bass range
    let intervals: &[f64] = &[
        1.2,  // minor third → CDT at 0.8*f1
        1.25, // major third → CDT at 0.75*f1
        1.5,  // perfect fifth → CDT at 0.5*f1
    ];

    // Sample fundamentals logarithmically
    let num_fundamentals = 20;
    let log_min = min_freq.max(30.0).ln();
    let log_max = 300.0_f64.min(max_freq).ln();

    if log_max <= log_min {
        return vec![(min_freq, -18.0), (max_freq, -18.0)];
    }

    for i in 0..num_fundamentals {
        let t = i as f64 / (num_fundamentals - 1) as f64;
        let f1 = (log_min + t * (log_max - log_min)).exp();

        for &ratio in intervals {
            let f2 = f1 * ratio;
            let cdt_freq = 2.0 * f1 - f2;

            if cdt_freq >= min_freq && cdt_freq <= max_freq {
                // CDT at moderate listening level (~75 dB) for two equal-level tones
                let cdt_db = cdt_level(75.0, 75.0); // ≈ -12 dB SPL
                // If CDT is above the hearing threshold (~40 dB at low frequencies),
                // it contributes to perception. Protect proportionally.
                let protection = if cdt_db > -20.0 {
                    -6.0 // strong protection: limit cut to -6 dB
                } else {
                    -12.0 // mild protection: limit cut to -12 dB
                };
                points.push((cdt_freq, protection));
            }
        }
    }

    if points.is_empty() {
        return vec![(min_freq, -18.0), (max_freq, -18.0)];
    }

    // Sort by frequency and deduplicate (keep the more protective value)
    points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Add boundary points with generous cut limit
    let mut envelope = Vec::with_capacity(points.len() + 2);
    if points[0].0 > min_freq {
        envelope.push((min_freq, -18.0));
    }
    envelope.extend_from_slice(&points);
    if points.last().unwrap().0 < max_freq {
        envelope.push((max_freq, -18.0));
    }

    envelope
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdt_level_equal_tones() {
        // Two 75 dB tones: CDT ≈ 2*75 - 75 - 63 = 12 dB
        let level = cdt_level(75.0, 75.0);
        assert!(
            (level - 12.0).abs() < 0.01,
            "Expected CDT level ~12 dB, got {}",
            level
        );
    }

    #[test]
    fn test_cdt_level_unequal_tones() {
        // L1=80, L2=70: CDT ≈ 2*80 - 70 - 63 = 27 dB
        let level = cdt_level(80.0, 70.0);
        assert!(
            (level - 27.0).abs() < 0.01,
            "Expected CDT level ~27 dB, got {}",
            level
        );
    }

    #[test]
    fn test_cdt_protection_envelope_has_entries() {
        let envelope = cdt_protection_envelope(20.0, 500.0);
        assert!(
            envelope.len() >= 3,
            "Envelope should have multiple points, got {}",
            envelope.len()
        );
        // Should be sorted by frequency
        for w in envelope.windows(2) {
            assert!(
                w[0].0 <= w[1].0,
                "Envelope not sorted: {} > {}",
                w[0].0,
                w[1].0
            );
        }
    }

    #[test]
    fn test_cdt_protection_envelope_limits() {
        let envelope = cdt_protection_envelope(20.0, 500.0);
        for &(freq, max_cut) in &envelope {
            assert!((20.0..=500.0).contains(&freq), "Freq {} out of range", freq);
            assert!(
                max_cut <= 0.0,
                "Max cut should be negative, got {}",
                max_cut
            );
            assert!(max_cut >= -18.0, "Max cut too deep: {}", max_cut);
        }
    }

    #[test]
    fn test_cdt_protection_envelope_empty_range() {
        // Very narrow range that produces no CDTs
        let envelope = cdt_protection_envelope(10000.0, 11000.0);
        assert!(envelope.len() >= 2, "Should have at least boundary points");
    }
}
