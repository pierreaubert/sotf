//! Target curve tilt generation for room correction
//!
//! Implements Harman-style tilted target curves instead of flat response.
//! Research shows that a gentle downward tilt (-0.8 dB/octave) is preferred
//! for in-room listening.

use crate::Curve;
use ndarray::Array1;

use super::types::{TargetTiltConfig, TiltType};

/// Build a target curve with configurable tilt
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `config` - Target tilt configuration
///
/// # Returns
/// * Target curve with applied tilt
///
/// # Details
/// The tilt is applied as: `tilt_db = slope * log2(f / ref_freq)`
/// For frequencies below bass_shelf_freq, an optional bass shelf boost is applied
/// with a smooth transition using a 2nd-order shelving function.
pub fn build_target_curve_with_tilt(freqs: &Array1<f64>, config: &TargetTiltConfig) -> Curve {
    let slope = match config.tilt_type {
        TiltType::Flat => 0.0,
        TiltType::Harman => -0.8, // Standard Harman room curve tilt
        TiltType::Custom => config.slope_db_per_octave,
    };

    let ref_freq = config.reference_freq;
    let bass_shelf_db = config.bass_shelf_db;
    let bass_shelf_freq = config.bass_shelf_freq;

    let spl = Array1::from_shape_fn(freqs.len(), |i| {
        let f = freqs[i].max(1.0); // Avoid log(0)

        // Base tilt: slope * log2(f / ref_freq)
        let tilt_db = slope * (f / ref_freq).log2();

        // Bass shelf with smooth transition
        let bass_boost = if bass_shelf_db.abs() > 0.001 && f < bass_shelf_freq * 2.0 {
            // Use a smooth shelving transition
            // At f = bass_shelf_freq, boost is bass_shelf_db * 0.5 (midpoint)
            // At f << bass_shelf_freq, boost approaches bass_shelf_db
            // At f >> bass_shelf_freq, boost approaches 0
            let ratio = f / bass_shelf_freq;
            // 2nd-order shelving function for smooth transition
            let transition = 1.0 / (1.0 + ratio.powi(2));
            bass_shelf_db * transition
        } else {
            0.0
        };

        tilt_db + bass_boost
    });

    Curve {
        freq: freqs.clone(),
        spl,
        phase: None,
    }
}

/// Create a Harman-style target curve with default settings
///
/// Uses -0.8 dB/octave slope with 1kHz reference frequency.
pub fn build_harman_target_curve(freqs: &Array1<f64>) -> Curve {
    let config = TargetTiltConfig {
        tilt_type: TiltType::Harman,
        slope_db_per_octave: -0.8,
        reference_freq: 1000.0,
        bass_shelf_db: 0.0,
        bass_shelf_freq: 200.0,
    };
    build_target_curve_with_tilt(freqs, &config)
}

/// Create a Harman-style target curve with bass boost
///
/// Uses -0.8 dB/octave slope with 1kHz reference frequency
/// and +3dB bass shelf below 200 Hz.
pub fn build_harman_target_curve_with_bass_boost(freqs: &Array1<f64>, bass_boost_db: f64) -> Curve {
    let config = TargetTiltConfig {
        tilt_type: TiltType::Harman,
        slope_db_per_octave: -0.8,
        reference_freq: 1000.0,
        bass_shelf_db: bass_boost_db,
        bass_shelf_freq: 200.0,
    };
    build_target_curve_with_tilt(freqs, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_frequencies() -> Array1<f64> {
        Array1::from(vec![20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0])
    }

    #[test]
    fn test_flat_target() {
        let freqs = test_frequencies();
        let config = TargetTiltConfig {
            tilt_type: TiltType::Flat,
            ..Default::default()
        };
        let curve = build_target_curve_with_tilt(&freqs, &config);

        // Flat target should have all zeros
        for &spl in curve.spl.iter() {
            assert!((spl - 0.0).abs() < 1e-10, "Expected 0.0, got {}", spl);
        }
    }

    #[test]
    fn test_harman_target() {
        let freqs = test_frequencies();
        let curve = build_harman_target_curve(&freqs);

        // At 1000 Hz (reference), should be 0 dB
        let idx_1k = freqs.iter().position(|&f| (f - 1000.0).abs() < 1.0).unwrap();
        assert!((curve.spl[idx_1k] - 0.0).abs() < 1e-10, "At 1kHz should be 0 dB");

        // At 2000 Hz (1 octave above), should be -0.8 dB
        let idx_2k = freqs.iter().position(|&f| (f - 2000.0).abs() < 1.0).unwrap();
        assert!((curve.spl[idx_2k] - (-0.8)).abs() < 1e-10, "At 2kHz should be -0.8 dB");

        // At 500 Hz (1 octave below), should be +0.8 dB
        let idx_500 = freqs.iter().position(|&f| (f - 500.0).abs() < 1.0).unwrap();
        assert!((curve.spl[idx_500] - 0.8).abs() < 1e-10, "At 500Hz should be +0.8 dB");
    }

    #[test]
    fn test_custom_slope() {
        let freqs = test_frequencies();
        let config = TargetTiltConfig {
            tilt_type: TiltType::Custom,
            slope_db_per_octave: -1.5, // Steeper slope
            reference_freq: 1000.0,
            bass_shelf_db: 0.0,
            bass_shelf_freq: 200.0,
        };
        let curve = build_target_curve_with_tilt(&freqs, &config);

        // At 2000 Hz (1 octave above), should be -1.5 dB
        let idx_2k = freqs.iter().position(|&f| (f - 2000.0).abs() < 1.0).unwrap();
        assert!((curve.spl[idx_2k] - (-1.5)).abs() < 1e-10, "At 2kHz should be -1.5 dB");
    }

    #[test]
    fn test_bass_shelf() {
        let freqs = test_frequencies();
        let config = TargetTiltConfig {
            tilt_type: TiltType::Flat, // Flat to isolate bass shelf effect
            slope_db_per_octave: 0.0,
            reference_freq: 1000.0,
            bass_shelf_db: 3.0,
            bass_shelf_freq: 200.0,
        };
        let curve = build_target_curve_with_tilt(&freqs, &config);

        // Well below shelf frequency should have full boost
        let idx_20 = 0; // 20 Hz
        assert!(curve.spl[idx_20] > 2.5, "At 20Hz should have significant bass boost");

        // Well above shelf frequency should have no boost
        let idx_1k = freqs.iter().position(|&f| (f - 1000.0).abs() < 1.0).unwrap();
        assert!(curve.spl[idx_1k].abs() < 0.1, "At 1kHz should have negligible boost");
    }

    #[test]
    fn test_combined_tilt_and_bass() {
        let freqs = test_frequencies();
        let curve = build_harman_target_curve_with_bass_boost(&freqs, 3.0);

        // At 1000 Hz, should still be ~0 dB (reference)
        let idx_1k = freqs.iter().position(|&f| (f - 1000.0).abs() < 1.0).unwrap();
        assert!(curve.spl[idx_1k].abs() < 0.1, "At 1kHz should be ~0 dB");

        // At 20 Hz, should have positive value (tilt + bass boost)
        let idx_20 = 0;
        // Tilt at 20Hz: -0.8 * log2(20/1000) = -0.8 * (-5.64) = +4.5 dB (approx)
        // Plus bass boost (~3 dB)
        assert!(curve.spl[idx_20] > 5.0, "At 20Hz should have significant boost from tilt + bass shelf");
    }
}
