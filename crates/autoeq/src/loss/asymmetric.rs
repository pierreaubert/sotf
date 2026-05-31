//! Asymmetric loss functions that penalize peaks more than dips
//! and de-weight narrow nulls via a precomputed suppression mask.
//!
//! Psychoacoustic motivation:
//! - **Peaks are fixable**: they are audible as tonal colouration and the
//!   optimizer can trim them with a cut filter.
//! - **Low-Q dips are fixable**: broad response deficits (baffle step,
//!   SBIR in the mid-band, driver integration) can be filled with boost.
//! - **High-Q dips are NOT fixable**: narrow nulls come from destructive
//!   interference between the direct sound and a delayed copy
//!   (room modes, early reflections). Boosting into such a null raises
//!   both the direct and the reflected wave by the same ratio — the
//!   cancellation stays, and amplifier headroom is wasted.
//!
//! This module builds a per-sample weight
//! `w(f, sign(error), null_mask)` that captures all three effects and
//! hands a pre-scaled error vector to
//! [`combined_weighted_loss`], so the asymmetric loss inherits the same
//! ERB + bass/mid/treble band blend used by [`flat_loss`](super::flat::flat_loss).

use super::enhanced_weights::{FrequencyBandWeights, combined_weighted_loss};
use ndarray::Array1;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ERB share of the asymmetric loss blend, matching `flat_loss`.
const ASYMMETRIC_ERB_WEIGHT: f64 = 0.7;
/// Band share of the asymmetric loss blend, matching `flat_loss`.
const ASYMMETRIC_BAND_WEIGHT: f64 = 0.3;

/// Configuration for asymmetric loss weighting.
///
/// Weights apply per sample as a multiplier on the squared error. The
/// peak / dip split uses a sigmoid crossfade in log frequency around
/// `transition_freq` so the transition from bass weighting to mid/treble
/// weighting is smooth. Narrow-null suppression is a separate mask passed
/// to the loss function at call time (see
/// [`crate::roomeq::impulse_analysis::build_null_suppression_mask`]).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AsymmetricLossConfig {
    /// Weight for positive errors (peaks above `transition_freq`). Default: 2.0
    pub peak_weight: f64,
    /// Weight for negative errors (dips above `transition_freq`). Default: 1.0
    pub dip_weight: f64,
    /// Weight for bass peaks (below `transition_freq`). Default: 5.0
    pub bass_peak_weight: f64,
    /// Weight for bass dips (below `transition_freq`). Default: 1.0
    ///
    /// Historically this defaulted to 0.2 (near-ignore) as a crude
    /// proxy for "do not fight acoustic nulls". With explicit narrow-null
    /// suppression in place, broad bass dips (SBIR, baffle step) become
    /// legitimate correction targets and the dip weight is aligned with
    /// the mid/treble default.
    pub bass_dip_weight: f64,
    /// Transition frequency between bass and mid/treble weighting. Default: 300.0 Hz
    pub transition_freq: f64,
}

impl Default for AsymmetricLossConfig {
    fn default() -> Self {
        Self {
            peak_weight: 2.0,
            dip_weight: 1.0,
            bass_peak_weight: 5.0,
            bass_dip_weight: 1.0,
            transition_freq: 300.0,
        }
    }
}

/// Compute the per-sample asymmetric weight for an error value.
///
/// Positive error (peak) picks the peak branch, negative error (dip)
/// picks the dip branch. The bass/treble crossover is a smooth sigmoid
/// in log frequency centred on `config.transition_freq` with ~90% of
/// the transition completing within ±0.5 octaves. The dip branch is
/// additionally scaled by `null_mask` when provided.
fn asymmetric_weight(
    freq: f64,
    error: f64,
    config: &AsymmetricLossConfig,
    log_transition: f64,
    sigmoid_k: f64,
    null_mask: f64,
) -> f64 {
    let blend = 1.0 / (1.0 + (-(freq.ln() - log_transition) * sigmoid_k).exp());
    let peak_w = config.bass_peak_weight + blend * (config.peak_weight - config.bass_peak_weight);
    let dip_w = config.bass_dip_weight + blend * (config.dip_weight - config.bass_dip_weight);
    if error > 0.0 {
        peak_w
    } else {
        dip_w * null_mask
    }
}

/// Compute the ERB + band blended asymmetric loss.
///
/// # Arguments
/// * `freqs` — frequency points in Hz.
/// * `error` — error values (positive = peak, negative = dip).
/// * `min_freq`, `max_freq` — inclusive range. Samples outside are ignored.
/// * `config` — peak/dip weighting.
/// * `null_suppression` — optional per-sample mask that scales the dip
///   branch toward zero at narrow nulls. Must have the same length as
///   `freqs` when provided; otherwise the call returns `f64::INFINITY`.
///
/// # Algorithm
/// The per-sample asymmetric weight `w[i]` is applied by scaling the
/// error to `e'[i] = e[i] * sqrt(w[i])` and delegating to
/// [`combined_weighted_loss`]. Because both the ERB-weighted MSE and the
/// band-weighted RMS accumulate `e'²`, the scaling recovers a true
/// `w[i] * e[i]²` contribution inside each perceptual weighting scheme
/// without re-implementing the ERB and band machinery here.
pub fn weighted_mse_asymmetric(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
    config: &AsymmetricLossConfig,
    null_suppression: Option<&Array1<f64>>,
) -> f64 {
    assert_eq!(freqs.len(), error.len());
    if let Some(mask) = null_suppression
        && mask.len() != freqs.len()
    {
        return f64::INFINITY;
    }

    // Range-filter freqs, error, and (if provided) the null mask in lockstep.
    let mut f_kept: Vec<f64> = Vec::with_capacity(freqs.len());
    let mut e_kept: Vec<f64> = Vec::with_capacity(freqs.len());
    let mut m_kept: Vec<f64> = Vec::with_capacity(freqs.len());
    let have_mask = null_suppression.is_some();
    for i in 0..freqs.len() {
        let f = freqs[i];
        if f >= min_freq && f <= max_freq {
            f_kept.push(f);
            e_kept.push(error[i]);
            if let Some(mask) = null_suppression {
                m_kept.push(mask[i].clamp(0.0, 1.0));
            }
        }
    }
    if f_kept.is_empty() {
        return f64::INFINITY;
    }

    let f_in = Array1::from(f_kept);
    let e_in = Array1::from(e_kept);

    let log_transition = config.transition_freq.ln();
    // Sigmoid steepness: ~90% of the crossover completes within ±0.5 octaves.
    let sigmoid_k = 2.0 * 9.0_f64.ln() / 2.0_f64.ln();

    // Build the sqrt-weighted error vector. `sqrt(w) * e` means
    // `combined_weighted_loss`'s accumulator sees `w * e²`, which is
    // exactly the per-sample weighting we want.
    let mut weighted_buf: Vec<f64> = Vec::with_capacity(f_in.len());
    for i in 0..f_in.len() {
        let f = f_in[i];
        let e = e_in[i];
        let null_mask = if have_mask { m_kept[i] } else { 1.0 };
        let w = asymmetric_weight(f, e, config, log_transition, sigmoid_k, null_mask);
        weighted_buf.push(e * w.max(0.0).sqrt());
    }
    let weighted_error = Array1::from(weighted_buf);

    let bands = FrequencyBandWeights::default();
    combined_weighted_loss(
        &f_in,
        &weighted_error,
        &bands,
        ASYMMETRIC_ERB_WEIGHT,
        ASYMMETRIC_BAND_WEIGHT,
    )
}

/// Compute flat loss with asymmetric weighting and narrow-null suppression.
///
/// Thin wrapper around [`weighted_mse_asymmetric`] that uses
/// `AsymmetricLossConfig::default()`. Pass `null_suppression` when you
/// want high-Q dips to be excluded from the dip penalty.
pub fn flat_loss_asymmetric(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
    null_suppression: Option<&Array1<f64>>,
) -> f64 {
    weighted_mse_asymmetric(
        freqs,
        error,
        min_freq,
        max_freq,
        &AsymmetricLossConfig::default(),
        null_suppression,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loss::enhanced_weights::combined_weighted_loss;

    fn linspace_log(f_min: f64, f_max: f64, n: usize) -> Array1<f64> {
        let lo = f_min.ln();
        let hi = f_max.ln();
        Array1::from_iter((0..n).map(|i| (lo + (hi - lo) * i as f64 / (n - 1) as f64).exp()))
    }

    fn closest_index(freqs: &Array1<f64>, target: f64) -> usize {
        let mut best_idx = 0;
        let mut best_diff = f64::INFINITY;
        for (i, &f) in freqs.iter().enumerate() {
            let diff = (f - target).abs();
            if diff < best_diff {
                best_diff = diff;
                best_idx = i;
            }
        }
        best_idx
    }

    #[test]
    fn zero_error_gives_zero_loss() {
        let freqs = linspace_log(20.0, 20000.0, 64);
        let error = Array1::zeros(freqs.len());
        let loss = flat_loss_asymmetric(&freqs, &error, 20.0, 20000.0, None);
        assert!(loss.abs() < 1e-12, "expected zero loss, got {loss}");
    }

    #[test]
    fn asymmetric_equals_combined_when_weights_are_unit() {
        // With every peak/dip weight set to 1.0 and no null mask, the
        // asymmetric loss must reduce exactly to combined_weighted_loss
        // at the default 0.7/0.3 blend. Range is padded beyond the
        // log-linspace endpoints so floating-point at the interval
        // bounds can't drop samples from the asymmetric range filter.
        let freqs = linspace_log(50.0, 15000.0, 128);
        let error: Array1<f64> = freqs.mapv(|f| ((f.ln() * 3.0).sin()) * 2.0);
        let config = AsymmetricLossConfig {
            peak_weight: 1.0,
            dip_weight: 1.0,
            bass_peak_weight: 1.0,
            bass_dip_weight: 1.0,
            transition_freq: 300.0,
        };
        let asym = weighted_mse_asymmetric(&freqs, &error, 20.0, 20000.0, &config, None);
        let expected =
            combined_weighted_loss(&freqs, &error, &FrequencyBandWeights::default(), 0.7, 0.3);
        assert!(
            (asym - expected).abs() < 1e-9,
            "asymmetric with unit weights must equal combined_weighted_loss ({asym} vs {expected})"
        );
    }

    #[test]
    fn bass_peaks_penalized_more_than_bass_dips() {
        let freqs = Array1::from_vec(vec![80.0]);
        let err_peak = Array1::from_vec(vec![10.0]);
        let err_dip = Array1::from_vec(vec![-10.0]);
        let loss_peak = flat_loss_asymmetric(&freqs, &err_peak, 20.0, 20000.0, None);
        let loss_dip = flat_loss_asymmetric(&freqs, &err_dip, 20.0, 20000.0, None);
        assert!(
            loss_peak > loss_dip,
            "bass peak must be penalized more than bass dip (peak={loss_peak}, dip={loss_dip})"
        );
    }

    #[test]
    fn custom_asymmetric_weights_override_defaults() {
        let freqs = Array1::from_vec(vec![80.0, 1000.0]);
        let error = Array1::from_vec(vec![4.0, -4.0]);
        let default_loss = weighted_mse_asymmetric(
            &freqs,
            &error,
            20.0,
            20000.0,
            &AsymmetricLossConfig::default(),
            None,
        );
        let custom = AsymmetricLossConfig {
            peak_weight: 8.0,
            dip_weight: 0.25,
            bass_peak_weight: 8.0,
            bass_dip_weight: 0.25,
            transition_freq: 300.0,
        };
        let custom_loss = weighted_mse_asymmetric(&freqs, &error, 20.0, 20000.0, &custom, None);

        assert!(
            custom_loss > default_loss,
            "larger custom peak weights should affect loss ({custom_loss} vs {default_loss})"
        );
    }

    #[test]
    fn null_mask_suppresses_bass_dip() {
        // Error vector with a single -15 dB dip at 80 Hz. The mask zeroes
        // the sample at 80 Hz. The masked loss must be strictly less than
        // the unmasked loss and close to zero.
        let freqs = linspace_log(20.0, 20000.0, 256);
        let mut error: Array1<f64> = Array1::zeros(freqs.len());
        let dip_idx = closest_index(&freqs, 80.0);
        error[dip_idx] = -15.0;

        let mut mask: Array1<f64> = Array1::ones(freqs.len());
        mask[dip_idx] = 0.0;

        let loss_unmasked = flat_loss_asymmetric(&freqs, &error, 20.0, 20000.0, None);
        let loss_masked = flat_loss_asymmetric(&freqs, &error, 20.0, 20000.0, Some(&mask));
        assert!(
            loss_masked < loss_unmasked,
            "null-masked loss must be smaller (masked={loss_masked}, unmasked={loss_unmasked})"
        );
        assert!(
            loss_masked < 1e-6,
            "fully masked single-point dip should collapse to ~0 (got {loss_masked})"
        );
    }

    #[test]
    fn null_mask_does_not_suppress_peaks() {
        // Same setup but with a positive error. The mask only affects
        // the dip branch, so the loss must be unchanged.
        let freqs = linspace_log(20.0, 20000.0, 256);
        let mut error: Array1<f64> = Array1::zeros(freqs.len());
        let peak_idx = closest_index(&freqs, 80.0);
        error[peak_idx] = 10.0;

        let mut mask: Array1<f64> = Array1::ones(freqs.len());
        mask[peak_idx] = 0.0;

        let unmasked = flat_loss_asymmetric(&freqs, &error, 20.0, 20000.0, None);
        let masked = flat_loss_asymmetric(&freqs, &error, 20.0, 20000.0, Some(&mask));
        assert!(
            (unmasked - masked).abs() < 1e-9,
            "peak branch must not be affected by the null mask ({unmasked} vs {masked})"
        );
    }

    #[test]
    fn broad_bass_dip_is_penalized_under_new_defaults() {
        // Broad -5 dB dip across 60-120 Hz. The detection step is not
        // run here (no narrow null), so the mask stays at 1.0 everywhere.
        // With the new default bass_dip_weight = 1.0 the loss must be
        // strictly larger than the loss that the old default (0.2) would
        // have produced.
        let freqs = linspace_log(20.0, 20000.0, 256);
        let error: Array1<f64> = freqs.mapv(|f| {
            if (60.0..=120.0).contains(&f) {
                -5.0
            } else {
                0.0
            }
        });

        let old_config = AsymmetricLossConfig {
            bass_dip_weight: 0.2,
            ..AsymmetricLossConfig::default()
        };
        let new_loss = flat_loss_asymmetric(&freqs, &error, 20.0, 20000.0, None);
        let old_loss = weighted_mse_asymmetric(&freqs, &error, 20.0, 20000.0, &old_config, None);
        assert!(
            new_loss > old_loss,
            "new default must penalize broad bass dips more (new={new_loss}, old={old_loss})"
        );
    }
}
