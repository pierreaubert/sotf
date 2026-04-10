//! EPA (Evaluation, Potency, Activity) composite score for room EQ optimization.
//!
//! Maps psychoacoustic metrics (loudness, sharpness, roughness) onto the three
//! semantic differential dimensions commonly used in sound quality research.
//! The composite preference score provides a single optimization target.

use super::{loudness, roughness, sharpness};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for EPA scoring.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EpaConfig {
    /// Listening level in phon (affects loudness computation)
    pub listening_level_phon: f64,
    /// Target sharpness in acum (1.0 = natural broadband noise character)
    pub target_sharpness: f64,
    /// Maximum acceptable roughness (above this, penalty increases)
    pub max_roughness: f64,
    /// Weights for the three EPA dimensions in the composite score
    pub evaluation_weight: f64,
    pub potency_weight: f64,
    pub activity_weight: f64,
}

impl Default for EpaConfig {
    fn default() -> Self {
        Self {
            listening_level_phon: 75.0,
            target_sharpness: 1.2,
            max_roughness: 0.5,
            evaluation_weight: 0.6,
            potency_weight: 0.2,
            activity_weight: 0.2,
        }
    }
}

/// EPA dimensions computed from a frequency response.
#[derive(Debug, Clone)]
pub struct EpaScore {
    /// Evaluation: general quality (higher = better, 0-10 scale)
    pub evaluation: f64,
    /// Potency: perceived energy/strength (0-10 scale)
    pub potency: f64,
    /// Activity: temporal complexity (lower = calmer, 0-10 scale)
    pub activity: f64,
    /// Composite preference (weighted combination, higher = better)
    pub preference: f64,
    /// Individual metric values for diagnostics
    pub sharpness_acum: f64,
    pub roughness: f64,
    pub total_loudness_sone: f64,
    pub loudness_balance: f64,
}

/// Compute EPA score from a frequency response.
pub fn compute_epa(freqs: &[f64], spl_db: &[f64], config: &EpaConfig) -> EpaScore {
    // 1. Compute specific loudness across Bark bands
    let specific = loudness::specific_loudness(freqs, spl_db, config.listening_level_phon);
    let total_loud = loudness::total_loudness(&specific);

    // 2. Compute sharpness (high-frequency emphasis metric)
    let sharp = sharpness::sharpness(&specific);

    // 3. Compute roughness from spectral peak interactions
    let rough = roughness::roughness_from_spectrum(freqs, spl_db);

    // 4. Compute loudness balance (uniformity of specific loudness)
    let mean_specific = total_loud / 24.0;
    let variance = specific
        .iter()
        .map(|&n| (n - mean_specific).powi(2))
        .sum::<f64>()
        / 24.0;
    let balance = 1.0 / (1.0 + variance.sqrt()); // 1.0 = perfectly uniform

    // 5. Map to EPA dimensions (0-10 scale)

    // Evaluation: penalize sharpness deviation from target + reward flatness
    let sharpness_error = (sharp - config.target_sharpness).abs();
    let evaluation = (10.0 - 3.0 * sharpness_error - 2.0 * (1.0 - balance)).clamp(0.0, 10.0);

    // Potency: based on total loudness normalized to typical listening levels
    let potency = (total_loud / 10.0).clamp(0.0, 10.0); // ~100 sone -> 10

    // Activity: roughness drives this (lower roughness = calmer sound)
    let activity = (rough * 5.0).clamp(0.0, 10.0);

    // 6. Composite preference: high E, moderate P, low A
    let preference = config.evaluation_weight * evaluation + config.potency_weight * potency
        - config.activity_weight * activity;

    EpaScore {
        evaluation,
        potency,
        activity,
        preference,
        sharpness_acum: sharp,
        roughness: rough,
        total_loudness_sone: total_loud,
        loudness_balance: balance,
    }
}

/// EPA-based loss function for the optimizer.
/// Lower = better (the optimizer minimizes this).
pub fn epa_loss(freqs: &[f64], spl_db: &[f64], config: &EpaConfig, flatness_loss: f64) -> f64 {
    let epa = compute_epa(freqs, spl_db, config);

    let sharpness_penalty = (epa.sharpness_acum - config.target_sharpness).powi(2);
    let roughness_penalty = (epa.roughness - config.max_roughness).max(0.0).powi(2);
    let balance_penalty = (1.0 - epa.loudness_balance).powi(2);

    // Weighted combination: flatness dominates, EPA refines
    0.4 * flatness_loss + 0.3 * sharpness_penalty + 0.2 * roughness_penalty + 0.1 * balance_penalty
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_flat_response(level_db: f64) -> (Vec<f64>, Vec<f64>) {
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl = vec![level_db; n];
        (freqs, spl)
    }

    fn make_harsh_response() -> (Vec<f64>, Vec<f64>) {
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl: Vec<f64> = freqs
            .iter()
            .map(|&f| if f > 5000.0 { 85.0 } else { 75.0 })
            .collect();
        (freqs, spl)
    }

    fn make_peaked_response() -> (Vec<f64>, Vec<f64>) {
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let mut spl: Vec<f64> = vec![70.0; n];
        // Add multiple prominent peaks that create roughness and imbalance.
        // Wide enough windows (~20 Hz) to hit multiple bins at ~32 Hz spacing.
        // Peaks at various frequencies including high-freq to raise sharpness.
        for (i, &f) in freqs.iter().enumerate() {
            if (f - 300.0).abs() < 20.0
                || (f - 370.0).abs() < 20.0
                || (f - 5000.0).abs() < 100.0
                || (f - 8000.0).abs() < 100.0
            {
                spl[i] = 90.0; // 20 dB peaks
            }
        }
        (freqs, spl)
    }

    #[test]
    fn test_epa_score_flat_response() {
        let (freqs, spl) = make_flat_response(75.0);
        let config = EpaConfig::default();
        let score = compute_epa(&freqs, &spl, &config);

        assert!(
            score.evaluation > 6.0,
            "Flat response should have high evaluation, got {}",
            score.evaluation
        );
        assert!(
            score.activity < 2.0,
            "Flat response should have low activity, got {}",
            score.activity
        );
    }

    #[test]
    fn test_epa_score_harsh_response() {
        let (freqs_flat, spl_flat) = make_flat_response(75.0);
        let (freqs_harsh, spl_harsh) = make_harsh_response();
        let config = EpaConfig::default();

        let flat_score = compute_epa(&freqs_flat, &spl_flat, &config);
        let harsh_score = compute_epa(&freqs_harsh, &spl_harsh, &config);

        assert!(
            harsh_score.evaluation < flat_score.evaluation,
            "Harsh response (eval={}) should have lower evaluation than flat (eval={})",
            harsh_score.evaluation,
            flat_score.evaluation
        );
        assert!(
            harsh_score.sharpness_acum > flat_score.sharpness_acum,
            "Harsh response (sharp={}) should have higher sharpness than flat (sharp={})",
            harsh_score.sharpness_acum,
            flat_score.sharpness_acum
        );
    }

    #[test]
    fn test_epa_loss_flat_is_low() {
        let (freqs, spl) = make_flat_response(75.0);
        let config = EpaConfig::default();
        let loss = epa_loss(&freqs, &spl, &config, 0.0);
        assert!(
            loss < 2.0,
            "Flat response with zero flatness loss should have low EPA loss, got {loss}"
        );
    }

    #[test]
    fn test_epa_loss_peaked_is_higher() {
        let (freqs_flat, spl_flat) = make_flat_response(75.0);
        let (freqs_peaked, spl_peaked) = make_peaked_response();
        let config = EpaConfig::default();

        let flat_loss = epa_loss(&freqs_flat, &spl_flat, &config, 0.0);
        let peaked_loss = epa_loss(&freqs_peaked, &spl_peaked, &config, 0.0);

        assert!(
            peaked_loss > flat_loss,
            "Peaked response (loss={peaked_loss}) should have higher loss than flat (loss={flat_loss})"
        );
    }

    #[test]
    fn test_epa_config_default() {
        let config = EpaConfig::default();
        assert_eq!(config.listening_level_phon, 75.0);
        assert_eq!(config.target_sharpness, 1.2);
        assert_eq!(config.max_roughness, 0.5);
        assert_eq!(config.evaluation_weight, 0.6);
        assert_eq!(config.potency_weight, 0.2);
        assert_eq!(config.activity_weight, 0.2);
        // Weights should sum to 1.0
        let total = config.evaluation_weight + config.potency_weight + config.activity_weight;
        assert!(
            (total - 1.0).abs() < 1e-10,
            "EPA weights should sum to 1.0, got {total}"
        );
    }
}
