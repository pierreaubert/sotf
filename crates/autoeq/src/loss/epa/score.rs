//! EPA (Evaluation, Potency, Activity) composite score for room EQ optimization.
//!
//! Maps psychoacoustic metrics (loudness, sharpness, roughness) onto the three
//! semantic differential dimensions commonly used in sound quality research.
//! The composite preference score provides a single optimization target.

use super::{loudness, roughness, sharpness};
use crate::loss::enhanced_weights::FrequencyBandWeights;
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
    /// Band weights used for the flatness component of the EPA loss.
    /// Only consulted when `flatness_band_weight > 0`.
    #[serde(default)]
    pub flatness_band_weights: FrequencyBandWeights,
    /// ERB weight for the flatness component of the EPA loss.
    /// Default 1.0 (pure ERB) because EPA already has band-sensitive
    /// sharpness / roughness / loudness_balance terms — adding band
    /// weighting on top of flatness would double-count frequency bias.
    #[serde(default = "default_flatness_erb_weight")]
    pub flatness_erb_weight: f64,
    /// Band weight for the flatness component of the EPA loss.
    /// Default 0.0 (see `flatness_erb_weight`).
    #[serde(default)]
    pub flatness_band_weight: f64,
}

fn default_flatness_erb_weight() -> f64 {
    1.0
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
            flatness_band_weights: FrequencyBandWeights::default(),
            flatness_erb_weight: 1.0,
            flatness_band_weight: 0.0,
        }
    }
}

/// EPA dimensions computed from a frequency response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
///
/// `spl_db` is expected to be **absolute** dB SPL. If you are working with
/// level-relative (mean-subtracted around 1 kHz) measurements such as those
/// in `CurveData`, use [`epa_loss_normalized`] instead.
pub fn epa_loss(freqs: &[f64], spl_db: &[f64], config: &EpaConfig, flatness_loss: f64) -> f64 {
    let epa = compute_epa(freqs, spl_db, config);

    let sharpness_penalty = (epa.sharpness_acum - config.target_sharpness).powi(2);
    let roughness_penalty = (epa.roughness - config.max_roughness).max(0.0).powi(2);
    let balance_penalty = (1.0 - epa.loudness_balance).powi(2);

    // Weighted combination: flatness dominates, EPA refines
    0.4 * flatness_loss + 0.3 * sharpness_penalty + 0.2 * roughness_penalty + 0.1 * balance_penalty
}

/// Denormalize a level-relative SPL curve to approximate absolute dB SPL.
///
/// Measurement curves in the autoeq/roomeq pipeline are typically
/// mean-subtracted around 1–2 kHz so they hover near 0 dB. The psychoacoustic
/// loudness model in [`crate::loss::epa::loudness`] compares against the
/// absolute threshold of hearing and therefore needs absolute dB SPL to
/// produce meaningful sone / loudness-balance values.
///
/// Since the phon scale is defined as equal-loudness contours referenced to a
/// 1 kHz sinusoid in dB SPL, adding `listening_level_phon` to a curve
/// normalized at 1 kHz yields an absolute SPL curve where 1 kHz sits at the
/// listener's chosen level. This is an approximation — it does not account
/// for frequency-dependent phon → SPL conversion via ISO 226 contours — but
/// it is the correct first-order calibration for comparative metrics and is
/// good enough for the loudness/balance penalty in [`epa_loss`].
fn denormalize_spl(spl_rel: &[f64], listening_level_phon: f64) -> Vec<f64> {
    spl_rel.iter().map(|v| v + listening_level_phon).collect()
}

/// Like [`compute_epa`] but for level-relative (mean-subtracted) input curves.
///
/// The input is denormalized by adding `config.listening_level_phon` before
/// evaluation. See [`denormalize_spl`] for the calibration rationale.
pub fn compute_epa_normalized(freqs: &[f64], spl_rel: &[f64], config: &EpaConfig) -> EpaScore {
    let spl_abs = denormalize_spl(spl_rel, config.listening_level_phon);
    compute_epa(freqs, &spl_abs, config)
}

/// Like [`epa_loss`] but for level-relative (mean-subtracted) input curves.
///
/// The input is denormalized by adding `config.listening_level_phon` before
/// evaluation so the loudness/balance components of the objective are
/// correctly calibrated against the absolute threshold of hearing.
pub fn epa_loss_normalized(
    freqs: &[f64],
    spl_rel: &[f64],
    config: &EpaConfig,
    flatness_loss: f64,
) -> f64 {
    let spl_abs = denormalize_spl(spl_rel, config.listening_level_phon);
    epa_loss(freqs, &spl_abs, config, flatness_loss)
}

/// Compute the flatness component of the EPA loss using the blend and
/// band weights specified in `config`.
///
/// This is the EPA-tunable counterpart of [`crate::loss::flat::flat_loss`].
/// Unlike the plain `flat_loss`, which uses a fixed 70/30 ERB-band blend,
/// `epa_flatness` honors the `flatness_erb_weight`, `flatness_band_weight`,
/// and `flatness_band_weights` fields on [`EpaConfig`] so that users
/// tuning their EPA runs can fully control the perceptual weighting of
/// the flatness term alongside the sharpness / roughness / balance
/// penalties in [`epa_loss`].
///
/// Frequencies outside `[min_freq, max_freq]` are excluded before the loss
/// is evaluated. Returns `f64::INFINITY` if no points remain in range.
pub fn epa_flatness(
    freqs: &ndarray::Array1<f64>,
    error: &ndarray::Array1<f64>,
    min_freq: f64,
    max_freq: f64,
    config: &EpaConfig,
) -> f64 {
    use crate::loss::enhanced_weights::combined_weighted_loss;
    let mut f_in = Vec::new();
    let mut e_in = Vec::new();
    for (&f, &e) in freqs.iter().zip(error.iter()) {
        if f >= min_freq && f <= max_freq {
            f_in.push(f);
            e_in.push(e);
        }
    }
    if f_in.is_empty() {
        return f64::INFINITY;
    }
    combined_weighted_loss(
        &ndarray::Array1::from(f_in),
        &ndarray::Array1::from(e_in),
        &config.flatness_band_weights,
        config.flatness_erb_weight,
        config.flatness_band_weight,
    )
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

    #[test]
    fn test_compute_epa_normalized_matches_absolute_equivalent() {
        // A curve normalized around 0 dB plus a 75 phon listening level should
        // produce the same EpaScore as the equivalent absolute 75 dB SPL curve.
        let (freqs, spl_abs) = make_flat_response(75.0);
        let spl_rel: Vec<f64> = spl_abs.iter().map(|v| v - 75.0).collect();

        let config = EpaConfig {
            listening_level_phon: 75.0,
            ..EpaConfig::default()
        };

        let score_abs = compute_epa(&freqs, &spl_abs, &config);
        let score_rel = compute_epa_normalized(&freqs, &spl_rel, &config);

        assert!(
            (score_abs.total_loudness_sone - score_rel.total_loudness_sone).abs() < 1e-9,
            "normalized path should match absolute path, got abs={} rel={}",
            score_abs.total_loudness_sone,
            score_rel.total_loudness_sone
        );
        assert!((score_abs.sharpness_acum - score_rel.sharpness_acum).abs() < 1e-9);
        assert!((score_abs.roughness - score_rel.roughness).abs() < 1e-9);
        assert!((score_abs.loudness_balance - score_rel.loudness_balance).abs() < 1e-9);
    }

    #[test]
    fn test_normalized_calibration_prevents_silent_floor() {
        // A level-relative flat curve (~0 dB everywhere) fed through the raw
        // `compute_epa` looks like near-silence to the Zwicker model because
        // its threshold-in-quiet is specified in absolute dB SPL. The
        // `_normalized` variant denormalizes against `listening_level_phon`
        // and must produce a non-trivial total loudness.
        let (freqs, _) = make_flat_response(0.0);
        let spl_rel = vec![0.0_f64; freqs.len()];

        let config = EpaConfig {
            listening_level_phon: 75.0,
            ..EpaConfig::default()
        };

        let raw_score = compute_epa(&freqs, &spl_rel, &config);
        let calibrated_score = compute_epa_normalized(&freqs, &spl_rel, &config);

        // Raw path (uncalibrated) should be at or near the silent floor.
        assert!(
            raw_score.total_loudness_sone < 0.5,
            "raw normalized input should be near-silent, got {}",
            raw_score.total_loudness_sone
        );
        // Calibrated path should show meaningful loudness (flat 75 dB ≈ 30+ sone).
        assert!(
            calibrated_score.total_loudness_sone > 5.0,
            "calibrated 75 phon flat curve should have meaningful loudness, got {}",
            calibrated_score.total_loudness_sone
        );
    }

    #[test]
    fn test_epa_loss_normalized_matches_absolute_equivalent() {
        // Same invariant as the compute_epa test: normalized path with the
        // listening-level offset should match the absolute path exactly.
        let (freqs, spl_abs) = make_flat_response(75.0);
        let spl_rel: Vec<f64> = spl_abs.iter().map(|v| v - 75.0).collect();

        let config = EpaConfig {
            listening_level_phon: 75.0,
            ..EpaConfig::default()
        };

        let loss_abs = epa_loss(&freqs, &spl_abs, &config, 0.25);
        let loss_rel = epa_loss_normalized(&freqs, &spl_rel, &config, 0.25);
        assert!(
            (loss_abs - loss_rel).abs() < 1e-12,
            "epa_loss_normalized should match epa_loss on denormalized input, got abs={} rel={}",
            loss_abs,
            loss_rel
        );
    }

    #[test]
    fn epa_flatness_uses_config_blend() {
        // At the extremes of the blend, `epa_flatness` should equal the
        // pure `erb_weighted_loss` / `band_weighted_loss` helpers.
        use crate::loss::enhanced_weights::{band_weighted_loss, erb_weighted_loss};
        let freqs = ndarray::Array1::from(vec![100.0, 1000.0, 5000.0, 10000.0]);
        let err = ndarray::Array1::from(vec![1.0, 1.0, 1.0, 1.0]);

        let mut cfg = EpaConfig::default();
        cfg.flatness_erb_weight = 1.0;
        cfg.flatness_band_weight = 0.0;
        let got_erb = epa_flatness(&freqs, &err, 20.0, 20000.0, &cfg);
        let expected_erb = erb_weighted_loss(&freqs, &err);
        assert!(
            (got_erb - expected_erb).abs() < 1e-9,
            "pure ERB blend should equal erb_weighted_loss, got {got_erb} vs {expected_erb}"
        );

        cfg.flatness_erb_weight = 0.0;
        cfg.flatness_band_weight = 1.0;
        let got_band = epa_flatness(&freqs, &err, 20.0, 20000.0, &cfg);
        let expected_band = band_weighted_loss(&freqs, &err, &cfg.flatness_band_weights);
        assert!(
            (got_band - expected_band).abs() < 1e-9,
            "pure band blend should equal band_weighted_loss, got {got_band} vs {expected_band}"
        );
    }

    #[test]
    fn epa_flatness_empty_range_returns_infinity() {
        let freqs = ndarray::Array1::from(vec![100.0, 200.0, 500.0]);
        let err = ndarray::Array1::from(vec![1.0, 1.0, 1.0]);
        let cfg = EpaConfig::default();
        assert!(epa_flatness(&freqs, &err, 5000.0, 10000.0, &cfg).is_infinite());
    }
}
