//! Speaker-specific loss functions.

use super::slope::regression_slope_per_octave_in_range;
use super::types::SpeakerLossData;
use crate::cea2034 as score;
use ndarray::Array1;

/// Compute the speaker preference score for a candidate PEQ response.
/// `peq_response` must be computed for the candidate parameters.
pub fn speaker_score_loss(
    score_data: &SpeakerLossData,
    freq: &Array1<f64>,
    peq_response: &Array1<f64>,
) -> f64 {
    // Compute 1/2-octave intervals on the fly using the provided frequency grid
    let intervals = score::octave_intervals(2, freq);
    let metrics = if peq_response.iter().all(|v| v.abs() < 1e-12) {
        // Exact score when no PEQ is applied
        score::score(
            freq,
            &intervals,
            &score_data.on,
            &score_data.lw,
            &score_data.sp,
            &score_data.pir,
        )
    } else {
        score::score_peq_approx(
            freq,
            &intervals,
            &score_data.lw,
            &score_data.sp,
            &score_data.pir,
            &score_data.on,
            peq_response,
        )
    };

    metrics.pref_score
}

/// Compute a mixed loss based on flatness on lw and pir
pub fn mixed_loss(
    score_data: &SpeakerLossData,
    freq: &Array1<f64>,
    peq_response: &Array1<f64>,
) -> f64 {
    let lw2 = &score_data.lw + peq_response;
    let pir2 = &score_data.pir + peq_response;
    // Compute slopes in dB per octave over 100 Hz .. 10 kHz
    let lw2_slope = regression_slope_per_octave_in_range(freq, &lw2, 100.0, 10000.0);
    let pir_og_slope = regression_slope_per_octave_in_range(freq, &score_data.pir, 100.0, 10000.0);
    let pir2_slope = regression_slope_per_octave_in_range(freq, &pir2, 100.0, 10000.0);
    if let (Some(lw2eq), Some(pir2og), Some(pir2eq)) = (lw2_slope, pir_og_slope, pir2_slope) {
        // some nlopt algorithms stop for negative values; keep result positive-ish
        (0.5 + lw2eq).powi(2) + (pir2og - pir2eq).powi(2)
    } else {
        f64::INFINITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Curve;
    use std::collections::HashMap;

    #[test]
    fn score_loss_matches_score_when_peq_zero() {
        // Simple synthetic data
        let freq = Array1::from(vec![100.0, 1000.0]);
        let on = Array1::from(vec![80.0_f64, 85.0_f64]);
        let lw = Array1::from(vec![81.0_f64, 84.0_f64]);
        let sp = Array1::from(vec![78.0_f64, 82.0_f64]);
        let pir = Array1::from(vec![80.5_f64, 84.0_f64]);

        // Build spin map expected by constructor
        let mut spin: HashMap<String, Curve> = HashMap::new();
        spin.insert(
            "On Axis".to_string(),
            Curve {
                freq: freq.clone(),
                spl: on.clone(),
                phase: None,
                ..Default::default()
            },
        );
        spin.insert(
            "Listening Window".to_string(),
            Curve {
                freq: freq.clone(),
                spl: lw.clone(),
                phase: None,
                ..Default::default()
            },
        );
        spin.insert(
            "Sound Power".to_string(),
            Curve {
                freq: freq.clone(),
                spl: sp.clone(),
                phase: None,
                ..Default::default()
            },
        );
        spin.insert(
            "Estimated In-Room Response".to_string(),
            Curve {
                freq: freq.clone(),
                spl: pir.clone(),
                phase: None,
                ..Default::default()
            },
        );

        let sd = SpeakerLossData::try_new(&spin).expect("test spin data should be valid");
        let zero = Array1::zeros(freq.len());

        // Expected preference using score() with zero PEQ (i.e., base curves)
        let intervals = score::octave_intervals(2, &freq);
        let expected = score::score(&freq, &intervals, &on, &lw, &sp, &pir);
        let got = speaker_score_loss(&sd, &freq, &zero);
        if got.is_nan() && expected.pref_score.is_nan() {
            // ok
        } else {
            assert!((got - expected.pref_score).abs() < 1e-12);
        }
    }

    #[test]
    fn mixed_loss_finite_with_zero_peq() {
        // Frequency grid
        let freq = Array1::from(vec![
            100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0, 10000.0,
        ]);
        // Zero curves
        let on = Array1::zeros(freq.len());
        let lw = Array1::zeros(freq.len());
        let sp = Array1::zeros(freq.len());
        let pir = Array1::zeros(freq.len());

        // Build spin map
        let mut spin: HashMap<String, Curve> = HashMap::new();
        spin.insert(
            "On Axis".to_string(),
            Curve {
                freq: freq.clone(),
                spl: on,
                phase: None,
                ..Default::default()
            },
        );
        spin.insert(
            "Listening Window".to_string(),
            Curve {
                freq: freq.clone(),
                spl: lw,
                phase: None,
                ..Default::default()
            },
        );
        spin.insert(
            "Sound Power".to_string(),
            Curve {
                freq: freq.clone(),
                spl: sp,
                phase: None,
                ..Default::default()
            },
        );
        spin.insert(
            "Estimated In-Room Response".to_string(),
            Curve {
                freq: freq.clone(),
                spl: pir,
                phase: None,
                ..Default::default()
            },
        );

        let sd = SpeakerLossData::try_new(&spin).expect("test spin data should be valid");
        let peq = Array1::zeros(freq.len());
        let v = mixed_loss(&sd, &freq, &peq);
        assert!(v.is_finite(), "mixed_loss should be finite, got {}", v);
    }
}
