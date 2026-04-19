//! Headphone loss functions.
//!
//! Implements the Olive/Welti/McMullin (2013) headphone preference
//! prediction model plus a convenience wrapper that handles target
//! curve normalization and optional 1/N octave smoothing.

use super::slope::{calculate_absolute_slope_in_range, calculate_standard_deviation_in_range};
use super::types::HeadphoneLossData;
use crate::Curve;
use crate::read;

/// Compute headphone preference score based on frequency response deviations
///
/// This implements the headphone preference prediction model from:
/// Olive, S. E., Welti, T., & McMullin, E. (2013). "A Statistical Model that
/// Predicts Listeners' Preference Ratings of Around-Ear and On-Ear Headphones"
///
/// The model predicts preference using the equation:
/// **Predicted Preference Rating = 114.49 - (12.62 × SD) - (15.52 × AS)**
///
/// Where:
/// - **SD** = Standard deviation of the deviation error over 50 Hz to 10 kHz
/// - **AS** = Absolute value of slope of the deviation over 50 Hz to 10 kHz
///
/// # Arguments
/// * `curve` - Frequency response curve representing deviation from Harman AE/OE target
///
/// # Returns
/// * Predicted preference rating (higher values indicate better preference)
///
/// # Important Note
/// The input curve should represent deviation from the Harman Around-Ear (AE) or
/// On-Ear (OE) target curve, **not** deviation from flat or neutral response.
///
/// The frequency range for calculations is 50 Hz to 10 kHz as specified in the paper.
///
/// # References
/// Olive, S. E., Welti, T., & McMullin, E. (2013). "A Statistical Model that
/// Predicts Listeners' Preference Ratings of Around-Ear and On-Ear Headphones".
/// Presented at the 135th Convention of the Audio Engineering Society.
pub fn headphone_loss(curve: &Curve) -> f64 {
    let freq = &curve.freq;
    let deviation = &curve.spl;

    // Define frequency range for analysis (50 Hz to 10 kHz per paper)
    const FMIN: f64 = 50.0;
    const FMAX: f64 = 10000.0;

    // Calculate SD (Standard Deviation) of the deviation error
    let sd = calculate_standard_deviation_in_range(freq, deviation, FMIN, FMAX);

    // Calculate AS (Absolute Slope) of the deviation
    let as_value = calculate_absolute_slope_in_range(freq, deviation, FMIN, FMAX);

    // Apply the Olive et al. equation (Equation 4 from the paper)
    // Predicted Preference Rating = 114.49 - (12.62 × SD) - (15.52 × AS)

    // Return the preference rating directly.
    // Optimizer wrappers convert this score into a minimization objective.
    114.49 - (12.62 * sd) - (15.52 * as_value)
}

/// Compute headphone preference score with additional target curve
///
/// # Arguments
/// * `data` - Headphone loss data containing smoothing parameters
/// * `response` - Measured frequency response in dB
/// * `target` - Target frequency response in dB
///
/// # Returns
/// * Predicted headphone preference score where higher is better
pub fn headphone_loss_with_target(
    data: &HeadphoneLossData,
    response: &Curve,
    target: &Curve,
) -> f64 {
    // freqs on which we normalize every curve: 12 points per octave between 20 and 20kHz
    let freqs = read::create_log_frequency_grid(10 * 12, 20.0, 20000.0);

    let input_curve = read::normalize_and_interpolate_response(&freqs, response);
    let target_curve = read::normalize_and_interpolate_response(&freqs, target);

    // normalized and potentially smooth
    let deviation = Curve {
        freq: freqs.clone(),
        spl: &target_curve.spl - &input_curve.spl,
        phase: None,
        ..Default::default()
    };
    let smooth_deviation = if data.smooth {
        read::smooth_one_over_n_octave(&deviation, data.smooth_n)
    } else {
        deviation.clone()
    };

    headphone_loss(&smooth_deviation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_headphone_loss_perfect_harman_deviation() {
        // Test with zero deviation from Harman target (perfect response)
        let freq = Array1::from(vec![50.0, 100.0, 1000.0, 5000.0, 10000.0]);
        let deviation = Array1::zeros(5); // Perfect match to Harman target

        let curve = Curve {
            freq: freq.clone(),
            spl: deviation,
            phase: None,
            ..Default::default()
        };
        let score = headphone_loss(&curve);

        // With zero deviation (SD=0, AS=0), predicted preference = 114.49
        // The helper returns the raw preference score.
        let expected_score = 114.49;
        assert!(
            (score - expected_score).abs() < 1e-12,
            "Perfect Harman score incorrect: got {}, expected {}",
            score,
            expected_score
        );
    }

    #[test]
    fn test_headphone_loss_with_deviation() {
        // Test with some deviation from Harman target
        let freq = Array1::from(vec![50.0, 100.0, 1000.0, 5000.0, 10000.0]);
        let deviation = Array1::from(vec![1.0, 1.0, 1.0, 1.0, 1.0]); // 1dB constant deviation

        let curve = Curve {
            freq: freq.clone(),
            spl: deviation,
            phase: None,
            ..Default::default()
        };
        let score = headphone_loss(&curve);

        // SD = 0 (constant deviation), AS = 0 (flat slope)
        // predicted preference = 114.49 - (12.62 * 0) - (15.52 * 0) = 114.49
        // But wait - SD should be 0 for constant values, but AS should also be 0
        let expected_preference = 114.49;
        let expected_score = expected_preference;
        assert!(
            (score - expected_score).abs() < 1e-10,
            "Constant deviation score incorrect: got {}, expected {}",
            score,
            expected_score
        );
    }

    #[test]
    fn test_headphone_loss_with_slope() {
        // Test with a sloped deviation
        let freq = Array1::from(vec![
            50.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0, 10000.0,
        ]);
        // Create a 1 dB/octave slope in the deviation
        let deviation = freq.mapv(|f: f64| 1.0 * f.log2());

        let curve = Curve {
            freq: freq.clone(),
            spl: deviation,
            phase: None,
            ..Default::default()
        };
        let score = headphone_loss(&curve);

        // AS = 1.0 (absolute slope)
        // SD will be non-zero due to the slope
        // predicted preference = 114.49 - (12.62 * SD) - (15.52 * 1.0)
        // Score should be worse (more negative) than perfect case (-114.49)
        assert!(
            score > 50.0,
            "Sloped deviation should have lower preference: got {}",
            score
        );
    }

    #[test]
    fn test_headphone_loss_with_target() {
        // Test the target curve variant with zero deviation
        let freq = Array1::logspace(10.0, 1.301, 4.301, 100);
        let response = Array1::from_elem(100, 5.0); // Constant 5dB response
        let target = Array1::from_elem(100, 5.0); // Same as response

        let response_curve = Curve {
            freq: freq.clone(),
            spl: response,
            phase: None,
            ..Default::default()
        };
        let target_curve = Curve {
            freq: freq.clone(),
            spl: target,
            phase: None,
            ..Default::default()
        };
        let data = HeadphoneLossData::new(false, 2);
        let score = headphone_loss_with_target(&data, &response_curve, &target_curve);

        // When response matches target, deviation is zero, so should get perfect score
        let expected_perfect_score = 114.49;
        assert!(
            (score - expected_perfect_score).abs() < 1e-10,
            "Perfect target match score incorrect: got {}, expected {}",
            score,
            expected_perfect_score
        );
    }

    #[test]
    fn test_headphone_loss_perfect_correction() {
        // Test that zero deviation gives perfect score
        let freq = Array1::logspace(10.0, 1.699, 4.0, 100); // 50Hz to 10kHz
        let zero_deviation = Array1::zeros(100);

        let curve = Curve {
            freq: freq.clone(),
            spl: zero_deviation,
            phase: None,
            ..Default::default()
        };
        let score = headphone_loss(&curve);

        // Perfect correction should give score of 114.49
        let expected_perfect = 114.49;
        assert!(
            (score - expected_perfect).abs() < 1e-10,
            "Perfect correction score incorrect: got {}, expected {}",
            score,
            expected_perfect
        );
    }

    #[test]
    fn test_headphone_loss_sign_independence() {
        // Test that headphone_loss gives same result for +deviation and -deviation
        // (since SD and AS are sign-independent)
        let freq = Array1::logspace(10.0, 1.699, 4.0, 100);

        // Create a deviation with varying values
        let deviation_positive = freq.mapv(|f: f64| 0.5 * f.log2() + 2.0);
        let deviation_negative = -&deviation_positive;

        let curve_pos = Curve {
            freq: freq.clone(),
            spl: deviation_positive,
            phase: None,
            ..Default::default()
        };
        let curve_neg = Curve {
            freq: freq.clone(),
            spl: deviation_negative,
            phase: None,
            ..Default::default()
        };

        let score_pos = headphone_loss(&curve_pos);
        let score_neg = headphone_loss(&curve_neg);

        // Scores should be equal since SD is symmetric and AS uses absolute value
        assert!(
            (score_pos - score_neg).abs() < 1e-10,
            "Sign independence violated: pos={}, neg={}",
            score_pos,
            score_neg
        );
    }

    #[test]
    fn test_headphone_loss_worse_than_perfect() {
        // Test that non-zero deviation gives worse score than zero
        let freq = Array1::logspace(10.0, 1.699, 4.0, 100);
        let zero_deviation = Array1::zeros(100);
        let nonzero_deviation = Array1::from_elem(100, 3.0); // 3dB constant deviation

        let perfect_curve = Curve {
            freq: freq.clone(),
            spl: zero_deviation,
            phase: None,
            ..Default::default()
        };
        let imperfect_curve = Curve {
            freq: freq.clone(),
            spl: nonzero_deviation,
            phase: None,
            ..Default::default()
        };

        let perfect_score = headphone_loss(&perfect_curve);
        let imperfect_score = headphone_loss(&imperfect_curve);

        // Imperfect should score lower (worse) than perfect
        assert!(
            imperfect_score < perfect_score,
            "Imperfect correction should score lower: perfect={}, imperfect={}",
            perfect_score,
            imperfect_score
        );

        // Perfect should be 114.49
        assert!(
            (perfect_score - 114.49).abs() < 1e-10,
            "Perfect score should be 114.49, got {}",
            perfect_score
        );
    }
}
