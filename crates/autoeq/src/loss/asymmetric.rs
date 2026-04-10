//! Asymmetric loss functions that penalize peaks (positive error)
//! more than dips (negative error).
//!
//! This aligns with psychoacoustic practice for room correction:
//! peaks are audible and can be cut with EQ, while dips are often
//! caused by acoustic cancellation and cannot be fixed with boost.

use super::flat::DEFAULT_BASS_TREBLE_SPLIT_HZ;
use ndarray::{Array1, Zip};

/// Configuration for asymmetric loss weighting
///
/// Peaks (positive deviations from target) should be penalized more than dips
/// because:
/// - We can hear peaks clearly and fix them with EQ cuts
/// - Dips/nulls are often caused by acoustic cancellation and cannot be fixed
///   with EQ boosts (boosting into a null just wastes amplifier power)
#[derive(Debug, Clone, Copy)]
pub struct AsymmetricLossConfig {
    /// Weight for positive errors (peaks above transition_freq) - default: 2.0
    pub peak_weight: f64,
    /// Weight for negative errors (dips above transition_freq) - default: 1.0
    pub dip_weight: f64,
    /// Weight for bass peaks (below transition_freq) - default: 5.0
    pub bass_peak_weight: f64,
    /// Weight for bass dips (below transition_freq) - default: 0.2
    pub bass_dip_weight: f64,
    /// Transition frequency between bass and mid/treble weighting - default: 300.0 Hz
    pub transition_freq: f64,
}

impl Default for AsymmetricLossConfig {
    fn default() -> Self {
        Self {
            peak_weight: 2.0,
            dip_weight: 1.0,
            bass_peak_weight: 5.0,
            bass_dip_weight: 0.2,
            transition_freq: 300.0,
        }
    }
}

/// Compute asymmetric weighted mean squared error
///
/// This loss function penalizes peaks (positive errors) more heavily than dips
/// (negative errors), which aligns with psychoacoustic principles:
/// - Peaks are audible and can be corrected with EQ cuts
/// - Dips/nulls cannot be effectively corrected with EQ boosts
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `error` - Error values at each frequency point (positive = peak, negative = dip)
/// * `min_freq` - Minimum frequency in Hz (inclusive)
/// * `max_freq` - Maximum frequency in Hz (inclusive)
/// * `config` - Asymmetric weighting configuration
///
/// # Returns
/// * Asymmetrically weighted error value
///
/// # Details
/// For each error value:
/// - If error > 0 (peak/overshoot): weighted by `peak_weight`
/// - If error < 0 (dip/undershoot): weighted by `dip_weight`
///
/// Default config uses peak_weight=2.0, dip_weight=1.0 (peaks penalized 2x more)
pub fn weighted_mse_asymmetric(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
    config: &AsymmetricLossConfig,
) -> f64 {
    weighted_mse_asymmetric_with_split(
        freqs,
        error,
        min_freq,
        max_freq,
        config,
        DEFAULT_BASS_TREBLE_SPLIT_HZ,
    )
}

/// Compute asymmetric weighted mean squared error with configurable bass/treble split
pub fn weighted_mse_asymmetric_with_split(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
    config: &AsymmetricLossConfig,
    bass_treble_split_hz: f64,
) -> f64 {
    // Create masks for frequency bands
    let bass_band = freqs.mapv(|f| f < bass_treble_split_hz && f >= min_freq && f <= max_freq);
    let treble_band = freqs.mapv(|f| f >= bass_treble_split_hz && f >= min_freq && f <= max_freq);

    // Count points in each band
    let n1: usize = bass_band.iter().filter(|&&b| b).count();
    let n2: usize = treble_band.iter().filter(|&&b| b).count();

    if n1 == 0 && n2 == 0 {
        return f64::INFINITY;
    }

    // Compute asymmetrically weighted squared errors with frequency-dependent weights.
    // Below transition_freq: bass_peak_weight / bass_dip_weight
    // Above transition_freq: peak_weight / dip_weight
    // Smooth sigmoid crossfade over ~1 octave centered at transition_freq.
    let log_transition = config.transition_freq.ln();
    // Steepness: ~90% transition within +/-0.5 octave => k = 2*ln(9)/ln(2) ~ 6.34
    let sigmoid_k = 2.0 * 9.0_f64.ln() / 2.0_f64.ln();

    let weighted_squared_errors = Zip::from(freqs).and(error).map_collect(|&f, &e| {
        // sigmoid blend: 0 = full bass weights, 1 = full mid/treble weights
        let blend = 1.0 / (1.0 + (-(f.ln() - log_transition) * sigmoid_k).exp());
        let peak_w =
            config.bass_peak_weight + blend * (config.peak_weight - config.bass_peak_weight);
        let dip_w =
            config.bass_dip_weight + blend * (config.dip_weight - config.bass_dip_weight);
        let weight = if e > 0.0 { peak_w } else { dip_w };
        weight * e * e
    });

    let ss1: f64 = Zip::from(&bass_band)
        .and(&weighted_squared_errors)
        .fold(0.0, |acc, &mask, &err| if mask { acc + err } else { acc });

    let ss2: f64 = Zip::from(&treble_band)
        .and(&weighted_squared_errors)
        .fold(0.0, |acc, &mask, &err| if mask { acc + err } else { acc });

    let err1 = if n1 > 0 {
        (ss1 / n1 as f64).sqrt()
    } else {
        0.0
    };
    let err2 = if n2 > 0 {
        (ss2 / n2 as f64).sqrt()
    } else {
        0.0
    };
    match (n1 > 0, n2 > 0) {
        (true, true) => err1 + err2 / 3.0,
        (true, false) => err1,
        (false, true) => err2,
        (false, false) => f64::INFINITY,
    }
}

/// Compute flat loss with asymmetric weighting (peaks penalized more than dips)
///
/// This is the asymmetric version of `flat_loss()`. Use this when you want the
/// optimizer to prioritize reducing peaks over filling dips.
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `error` - Error values (positive = peak, negative = dip)
/// * `min_freq` - Minimum frequency in Hz
/// * `max_freq` - Maximum frequency in Hz
///
/// # Returns
/// * Loss value with peaks penalized 2x more than dips
pub fn flat_loss_asymmetric(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
) -> f64 {
    weighted_mse_asymmetric(
        freqs,
        error,
        min_freq,
        max_freq,
        &AsymmetricLossConfig::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bass_asymmetry_penalizes_peaks_heavily() {
        // A 10dB peak at 80Hz should produce ~5x the penalty of same peak at 2kHz
        // because bass_peak_weight=5.0 vs peak_weight=2.0
        let config = AsymmetricLossConfig::default();

        // Single-frequency test at 80Hz (deep bass)
        let freqs_bass = Array1::from_vec(vec![80.0]);
        let error_bass = Array1::from_vec(vec![10.0]); // 10dB peak

        let loss_bass = weighted_mse_asymmetric_with_split(
            &freqs_bass,
            &error_bass,
            20.0,
            20000.0,
            &config,
            3000.0,
        );

        // Single-frequency test at 2kHz (mid range)
        let freqs_mid = Array1::from_vec(vec![2000.0]);
        let error_mid = Array1::from_vec(vec![10.0]); // 10dB peak

        let loss_mid = weighted_mse_asymmetric_with_split(
            &freqs_mid,
            &error_mid,
            20.0,
            20000.0,
            &config,
            3000.0,
        );

        // 80Hz is well below 300Hz transition, so weight ~ bass_peak_weight = 5.0
        // 2kHz is well above 300Hz transition, so weight ~ peak_weight = 2.0
        // The loss returns sqrt(weight * e^2) = e * sqrt(weight)
        // So ratio = sqrt(5.0) / sqrt(2.0) = sqrt(2.5) ~ 1.58
        let ratio = loss_bass / loss_mid;
        let expected_ratio = (5.0_f64 / 2.0).sqrt();
        assert!(
            (ratio - expected_ratio).abs() < 0.1,
            "bass peak penalty ratio should be ~{:.2}x (sqrt(5/2)), got {:.2}",
            expected_ratio,
            ratio
        );
    }

    #[test]
    fn test_bass_dips_nearly_ignored() {
        // 10dB dip at 80Hz should produce near-zero penalty (bass_dip_weight=0.2)
        let config = AsymmetricLossConfig::default();

        // 10dB dip at 80Hz
        let freqs = Array1::from_vec(vec![80.0]);
        let error_dip = Array1::from_vec(vec![-10.0]);

        let loss_dip = weighted_mse_asymmetric_with_split(
            &freqs,
            &error_dip,
            20.0,
            20000.0,
            &config,
            3000.0,
        );

        // 10dB peak at 80Hz for comparison
        let error_peak = Array1::from_vec(vec![10.0]);
        let loss_peak = weighted_mse_asymmetric_with_split(
            &freqs,
            &error_peak,
            20.0,
            20000.0,
            &config,
            3000.0,
        );

        // dip weight 0.2 vs peak weight 5.0, so dip loss / peak loss ~ 0.2/5.0 = 0.04
        let ratio = loss_dip / loss_peak;
        assert!(
            ratio < 0.25,
            "bass dip penalty should be much smaller than bass peak, ratio={:.4}",
            ratio
        );
    }
}
