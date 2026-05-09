//! Spatial robustness analysis for multi-position room measurements.
//!
//! Implements spatial robustness optimization:
//! 1. RMS power spectrum averaging across measurement positions
//! 2. Spatial variance computation per frequency bin
//! 3. Correction depth mask: full correction where variance is low (room modes),
//!    reduced correction where variance is high (position-dependent effects)
//! 4. Bootstrap confidence band on the RMS-averaged target — supports
//!    measurement-uncertainty-aware robust optimization downstream.
//!
//! Reference: Brännmark & Sternad, AES 124th Convention (2008)
//! Reference: Patent EP2104374B1 — spatial zero clustering
//! Reference: Efron, "Bootstrap Methods: Another Look at the Jackknife" (1979)

use crate::Curve;
use crate::error::{AutoeqError, Result};
use ndarray::Array1;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Configuration for spatial robustness analysis.
#[derive(Debug, Clone)]
pub struct SpatialRobustnessConfig {
    /// Variance threshold (dB) below which full correction is allowed.
    /// Default: 3.0 dB
    pub variance_threshold_db: f64,

    /// Transition width (dB) for sigmoid blending between full and reduced correction.
    /// Default: 2.0 dB
    pub transition_width_db: f64,

    /// Minimum correction depth (0.0 to 1.0). Even high-variance frequencies get
    /// at least this much correction weight. Default: 0.1
    pub min_correction_depth: f64,

    /// Smoothing width in octaves for the correction depth mask.
    /// Prevents rapid mask changes between adjacent frequencies. Default: 1/6 octave.
    pub mask_smoothing_octaves: f64,
}

impl Default for SpatialRobustnessConfig {
    fn default() -> Self {
        Self {
            variance_threshold_db: 3.0,
            transition_width_db: 2.0,
            min_correction_depth: 0.1,
            mask_smoothing_octaves: 1.0 / 6.0,
        }
    }
}

/// Result of spatial robustness analysis.
#[derive(Debug, Clone)]
pub struct SpatialRobustnessResult {
    /// RMS-averaged frequency response across all positions.
    pub averaged_curve: Curve,

    /// Per-frequency standard deviation across positions (dB).
    pub spatial_variance: Array1<f64>,

    /// Per-frequency correction depth mask (0.0 = no correction, 1.0 = full correction).
    pub correction_depth: Array1<f64>,

    /// Optional bootstrap confidence band on the RMS-averaged curve.
    /// Populated only when [`analyze_spatial_robustness_with_bootstrap`] is used.
    pub bootstrap: Option<BootstrapBand>,
}

/// Configuration for bootstrap confidence-band estimation.
///
/// Implements case-bootstrap on the input measurement curves: each of `num_resamples`
/// resamples draws N curves with replacement from the N input measurements, then
/// computes the RMS-averaged response. Per-frequency percentile bands are extracted
/// from the resulting B resampled means.
///
/// Case-bootstrap is the appropriate choice when each input curve represents one
/// independent measurement (sweep / mic position) — it makes no parametric assumption
/// about the noise distribution, which matters for typical N=3..9 mic surveys where
/// noise is highly non-Gaussian (modal frequencies, comb-filter dips).
#[derive(Debug, Clone)]
pub struct BootstrapConfig {
    /// Number of bootstrap resamples B. Typical: 200..1000. Default: 500.
    pub num_resamples: usize,
    /// Two-sided confidence level α — band covers `[α/2, 1-α/2]`. Default: 0.10 (90 % CI).
    pub alpha: f64,
    /// PRNG seed for determinism.
    pub seed: u64,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            num_resamples: 500,
            alpha: 0.10,
            seed: 0xC0FFEE,
        }
    }
}

/// Per-frequency confidence band on the RMS-averaged curve, plus the per-bin sample
/// standard deviation across bootstrap resamples.
///
/// All four arrays share the input frequency grid. SPL units are dB.
#[derive(Debug, Clone)]
pub struct BootstrapBand {
    /// Lower percentile curve at α/2.
    pub lower: Curve,
    /// Median curve (50th percentile across resamples).
    pub median: Curve,
    /// Upper percentile curve at 1-α/2.
    pub upper: Curve,
    /// Per-bin standard deviation across the B resampled means (dB).
    pub per_bin_std: Array1<f64>,
}

/// Compute RMS power spectrum average across multiple measurement positions.
///
/// Unlike arithmetic averaging of dB values (which underweights loud positions)
/// or complex averaging (which causes phase cancellation), RMS averaging preserves
/// the energy content:
///
///   avg_spl[f] = 10 * log10(mean(10^(spl_i[f] / 10)))
///
/// All curves must share the same frequency axis.
pub fn rms_average(curves: &[Curve]) -> Curve {
    rms_average_weighted(curves, None)
}

pub fn rms_average_weighted(curves: &[Curve], weights: Option<&[f64]>) -> Curve {
    validate_spatial_curves(curves).expect("spatial robustness curves must be valid");
    let len = curves[0].freq.len();
    let weights = normalized_weights(curves.len(), weights);

    let mut avg_spl = Array1::zeros(len);
    for bin in 0..len {
        let sum_power: f64 = curves
            .iter()
            .zip(weights.iter())
            .map(|(c, weight)| weight * 10.0_f64.powf(c.spl[bin] / 10.0))
            .sum();
        avg_spl[bin] = 10.0 * sum_power.max(1e-12).log10();
    }

    Curve {
        freq: curves[0].freq.clone(),
        spl: avg_spl,
        phase: None,
        ..Default::default()
    }
}

/// Compute per-frequency standard deviation across positions (in dB).
///
/// A low std dev at a frequency means the feature is spatially consistent
/// (e.g., a room mode). A high std dev means position-dependent (e.g., comb
/// filtering from reflections arriving at different phase per position).
pub fn spatial_std_dev(curves: &[Curve]) -> Array1<f64> {
    spatial_std_dev_weighted(curves, None)
}

pub fn spatial_std_dev_weighted(curves: &[Curve], weights: Option<&[f64]>) -> Array1<f64> {
    validate_spatial_curves(curves).expect("spatial robustness curves must be valid");
    if curves.len() == 1 {
        // Single curve: zero variance everywhere
        return Array1::zeros(curves[0].freq.len());
    }
    let len = curves[0].freq.len();
    let weights = normalized_weights(curves.len(), weights);

    let mut std_dev = Array1::zeros(len);
    for bin in 0..len {
        let mean: f64 = curves
            .iter()
            .zip(weights.iter())
            .map(|(c, weight)| weight * c.spl[bin])
            .sum();
        let variance: f64 = curves
            .iter()
            .zip(weights.iter())
            .map(|(c, weight)| weight * (c.spl[bin] - mean).powi(2))
            .sum::<f64>();
        let unbiased_denominator = 1.0 - weights.iter().map(|w| w * w).sum::<f64>();
        let denominator_floor = 1.0 / curves.len() as f64;
        std_dev[bin] = (variance / unbiased_denominator.max(denominator_floor)).sqrt();
    }

    std_dev
}

/// Build a correction depth mask from spatial variance.
///
/// Uses a sigmoid function to transition smoothly between full correction
/// (where variance < threshold) and minimum correction (where variance >> threshold):
///
///   depth[f] = min_depth + (1 - min_depth) * sigmoid((threshold - var[f]) / width)
///
/// The mask is then smoothed in the log-frequency domain to avoid sharp transitions.
pub fn correction_depth_mask(
    freq: &Array1<f64>,
    spatial_variance: &Array1<f64>,
    config: &SpatialRobustnessConfig,
) -> Array1<f64> {
    let len = freq.len();
    let mut mask = Array1::zeros(len);

    // Sigmoid: maps (threshold - variance) / width through 1/(1+exp(-x))
    for i in 0..len {
        let sigmoid = if config.transition_width_db <= 0.0 {
            // Hard threshold: step function
            if spatial_variance[i] <= config.variance_threshold_db {
                1.0
            } else {
                0.0
            }
        } else {
            let x =
                (config.variance_threshold_db - spatial_variance[i]) / config.transition_width_db;
            1.0 / (1.0 + (-x).exp())
        };
        mask[i] = config.min_correction_depth + (1.0 - config.min_correction_depth) * sigmoid;
    }

    // Smooth the mask in log-frequency domain to prevent rapid oscillations
    if config.mask_smoothing_octaves > 0.0 {
        mask = smooth_log_frequency(&mask, freq, config.mask_smoothing_octaves);
    }

    mask
}

/// Perform full spatial robustness analysis on a set of multi-position measurements.
///
/// Returns the RMS-averaged curve, spatial variance, and correction depth mask.
pub fn analyze_spatial_robustness(
    curves: &[Curve],
    config: &SpatialRobustnessConfig,
) -> Result<SpatialRobustnessResult> {
    try_analyze_spatial_robustness_weighted(curves, config, None)
}

pub fn analyze_spatial_robustness_weighted(
    curves: &[Curve],
    config: &SpatialRobustnessConfig,
    weights: Option<&[f64]>,
) -> Result<SpatialRobustnessResult> {
    try_analyze_spatial_robustness_weighted(curves, config, weights)
}

pub fn try_analyze_spatial_robustness_weighted(
    curves: &[Curve],
    config: &SpatialRobustnessConfig,
    weights: Option<&[f64]>,
) -> Result<SpatialRobustnessResult> {
    validate_spatial_curves(curves)?;

    let averaged_curve = rms_average_weighted(curves, weights);
    let spatial_variance = spatial_std_dev_weighted(curves, weights);
    let correction_depth = correction_depth_mask(&averaged_curve.freq, &spatial_variance, config);

    Ok(SpatialRobustnessResult {
        averaged_curve,
        spatial_variance,
        correction_depth,
        bootstrap: None,
    })
}

/// Perform spatial robustness analysis and additionally compute a bootstrap
/// confidence band on the RMS-averaged target.
///
/// This is the entry point used by measurement-uncertainty-aware robust
/// optimization (`MultiMeasurementStrategy::MinimaxUncertainty`). The returned
/// [`BootstrapBand`] is suitable both for plotting and for materialising a bank
/// of bootstrap-resampled `ObjectiveData` upstream in the optimizer pipeline.
pub fn analyze_spatial_robustness_with_bootstrap(
    curves: &[Curve],
    config: &SpatialRobustnessConfig,
    bootstrap: &BootstrapConfig,
    weights: Option<&[f64]>,
) -> Result<SpatialRobustnessResult> {
    let mut result = try_analyze_spatial_robustness_weighted(curves, config, weights)?;
    result.bootstrap = Some(bootstrap_band(curves, bootstrap, weights)?);
    Ok(result)
}

/// Generate B bootstrap-resampled RMS-averaged curves and return per-frequency
/// confidence band statistics.
///
/// Uses case-bootstrap: at each iteration, resample N indices with replacement
/// from the N input curves, compute [`rms_average_weighted`] on the resampled
/// set, and accumulate. The returned band reports per-bin α/2 and 1-α/2
/// percentiles, the median, and the sample standard deviation across resamples.
///
/// Constraints:
/// - All curves must share the same frequency grid (validated upstream).
/// - `num_resamples` must be > 0.
/// - `alpha` must be in (0, 1).
/// - Single-curve input degenerates: the band collapses to the input curve
///   with zero width (the only resample possible is the curve itself).
pub fn bootstrap_band(
    curves: &[Curve],
    config: &BootstrapConfig,
    weights: Option<&[f64]>,
) -> Result<BootstrapBand> {
    validate_spatial_curves(curves)?;
    if config.num_resamples == 0 {
        return Err(AutoeqError::InvalidConfiguration {
            message: "bootstrap num_resamples must be > 0".to_string(),
        });
    }
    if !(0.0..1.0).contains(&config.alpha) || config.alpha <= 0.0 {
        return Err(AutoeqError::InvalidConfiguration {
            message: format!("bootstrap alpha must be in (0, 1), got {}", config.alpha),
        });
    }

    let n = curves.len();
    let num_bins = curves[0].freq.len();
    let b = config.num_resamples;

    // Storage: resampled_means[bin][resample] — reshaped at percentile time.
    let mut resampled_means: Vec<Vec<f64>> = vec![Vec::with_capacity(b); num_bins];

    let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
    let mut indices: Vec<usize> = vec![0; n];
    let mut resampled: Vec<Curve> = Vec::with_capacity(n);
    let mut resampled_weights: Option<Vec<f64>> = weights.map(|_| Vec::with_capacity(n));

    for _ in 0..b {
        for slot in indices.iter_mut() {
            *slot = rng.random_range(0..n);
        }

        resampled.clear();
        if let Some(buf) = resampled_weights.as_mut() {
            buf.clear();
        }
        for &idx in &indices {
            resampled.push(curves[idx].clone());
            if let (Some(buf), Some(src)) = (resampled_weights.as_mut(), weights) {
                buf.push(src[idx]);
            }
        }

        let mean_curve = rms_average_weighted(&resampled, resampled_weights.as_deref());
        for (bin, samples) in resampled_means.iter_mut().enumerate() {
            samples.push(mean_curve.spl[bin]);
        }
    }

    let lower_q = config.alpha / 2.0;
    let upper_q = 1.0 - config.alpha / 2.0;

    let mut lower_spl = Array1::<f64>::zeros(num_bins);
    let mut median_spl = Array1::<f64>::zeros(num_bins);
    let mut upper_spl = Array1::<f64>::zeros(num_bins);
    let mut per_bin_std = Array1::<f64>::zeros(num_bins);

    for bin in 0..num_bins {
        let samples = &mut resampled_means[bin];
        // Percentile via sort + interpolation.
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        lower_spl[bin] = percentile_sorted(samples, lower_q);
        median_spl[bin] = percentile_sorted(samples, 0.5);
        upper_spl[bin] = percentile_sorted(samples, upper_q);

        // Sample std (unbiased, /(B-1)) across resamples.
        let mean: f64 = samples.iter().copied().sum::<f64>() / samples.len() as f64;
        let var = if samples.len() > 1 {
            samples.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / (samples.len() - 1) as f64
        } else {
            0.0
        };
        per_bin_std[bin] = var.sqrt();
    }

    let freq = curves[0].freq.clone();
    let make_curve = |spl: Array1<f64>| Curve {
        freq: freq.clone(),
        spl,
        phase: None,
        ..Default::default()
    };

    Ok(BootstrapBand {
        lower: make_curve(lower_spl),
        median: make_curve(median_spl),
        upper: make_curve(upper_spl),
        per_bin_std,
    })
}

/// Generate a bank of B bootstrap-resampled RMS-averaged curves.
///
/// Used by upstream optimizer wiring (`compute_multi_objective_fitness`) to build
/// a fixed sample bank once at setup time. Pass the same `seed` as `bootstrap_band`
/// for reproducibility of CI plots vs optimizer state.
pub fn bootstrap_resampled_curves(
    curves: &[Curve],
    config: &BootstrapConfig,
    weights: Option<&[f64]>,
) -> Result<Vec<Curve>> {
    validate_spatial_curves(curves)?;
    if config.num_resamples == 0 {
        return Err(AutoeqError::InvalidConfiguration {
            message: "bootstrap num_resamples must be > 0".to_string(),
        });
    }

    let n = curves.len();
    let b = config.num_resamples;
    let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
    let mut indices: Vec<usize> = vec![0; n];
    let mut resampled: Vec<Curve> = Vec::with_capacity(n);
    let mut resampled_weights: Option<Vec<f64>> = weights.map(|_| Vec::with_capacity(n));
    let mut output: Vec<Curve> = Vec::with_capacity(b);

    for _ in 0..b {
        for slot in indices.iter_mut() {
            *slot = rng.random_range(0..n);
        }

        resampled.clear();
        if let Some(buf) = resampled_weights.as_mut() {
            buf.clear();
        }
        for &idx in &indices {
            resampled.push(curves[idx].clone());
            if let (Some(buf), Some(src)) = (resampled_weights.as_mut(), weights) {
                buf.push(src[idx]);
            }
        }

        output.push(rms_average_weighted(
            &resampled,
            resampled_weights.as_deref(),
        ));
    }

    Ok(output)
}

/// Linearly-interpolated percentile on a sorted slice.
///
/// `q` in [0, 1]; returns `samples[0]` for empty/q=0, `samples[last]` for q=1.
fn percentile_sorted(samples: &[f64], q: f64) -> f64 {
    if samples.is_empty() {
        return f64::NAN;
    }
    let q = q.clamp(0.0, 1.0);
    let last = samples.len() - 1;
    let pos = q * last as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        samples[lo]
    } else {
        let t = pos - lo as f64;
        samples[lo] * (1.0 - t) + samples[hi] * t
    }
}

fn validate_spatial_curves(curves: &[Curve]) -> Result<()> {
    if curves.is_empty() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "spatial robustness needs at least one curve".to_string(),
        });
    }

    let reference = &curves[0].freq;
    if !is_valid_spatial_frequency_grid(reference) || curves[0].spl.len() != reference.len() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "spatial robustness reference curve has an invalid frequency grid".to_string(),
        });
    }

    for (idx, curve) in curves.iter().enumerate().skip(1) {
        if !is_valid_spatial_frequency_grid(&curve.freq) || curve.spl.len() != curve.freq.len() {
            return Err(AutoeqError::InvalidMeasurement {
                message: format!(
                    "spatial robustness curve {} has an invalid frequency grid",
                    idx
                ),
            });
        }
        if !super::frequency_grid::same_frequency_grid(reference, &curve.freq) {
            return Err(AutoeqError::InvalidMeasurement {
                message: format!(
                    "spatial robustness curves must share the same frequency grid; curve {} differs",
                    idx
                ),
            });
        }
    }

    Ok(())
}

fn is_valid_spatial_frequency_grid(freq: &Array1<f64>) -> bool {
    !freq.is_empty()
        && freq.iter().all(|f| f.is_finite() && *f > 0.0)
        && freq.windows(2).into_iter().all(|pair| pair[0] < pair[1])
}

fn normalized_weights(len: usize, weights: Option<&[f64]>) -> Vec<f64> {
    let Some(weights) = weights else {
        return vec![1.0 / len as f64; len];
    };
    if weights.len() != len {
        return vec![1.0 / len as f64; len];
    }
    let mut clean: Vec<f64> = weights
        .iter()
        .map(|w| if w.is_finite() && *w > 0.0 { *w } else { 0.0 })
        .collect();
    let sum: f64 = clean.iter().sum();
    if sum <= f64::EPSILON {
        return vec![1.0 / len as f64; len];
    }
    for weight in &mut clean {
        *weight /= sum;
    }
    clean
}

/// Smooth an array using a sliding window in log-frequency domain.
///
/// Window width is specified in octaves. Each output sample is the average
/// of all input samples within +/- half_width octaves.
fn smooth_log_frequency(data: &Array1<f64>, freq: &Array1<f64>, width_octaves: f64) -> Array1<f64> {
    let len = data.len();
    let half_width = width_octaves / 2.0;
    let mut smoothed = Array1::zeros(len);

    if !freq.windows(2).into_iter().all(|pair| pair[0] <= pair[1]) {
        for i in 0..len {
            let center_log = freq[i].log2();
            let low_log = center_log - half_width;
            let high_log = center_log + half_width;

            let mut sum = 0.0;
            let mut count = 0.0;
            for j in 0..len {
                let f_log = freq[j].log2();
                if f_log >= low_log && f_log <= high_log {
                    sum += data[j];
                    count += 1.0;
                }
            }

            smoothed[i] = if count > 0.0 { sum / count } else { data[i] };
        }
        return smoothed;
    }

    let logs: Vec<f64> = freq.iter().map(|f| f.log2()).collect();
    let mut left = 0usize;
    let mut right = 0usize;
    let mut sum = 0.0;

    for i in 0..len {
        let low_log = logs[i] - half_width;
        let high_log = logs[i] + half_width;

        while right < len && logs[right] <= high_log {
            sum += data[right];
            right += 1;
        }

        while left < right && logs[left] < low_log {
            sum -= data[left];
            left += 1;
        }

        let count = right - left;
        smoothed[i] = if count > 0 {
            sum / count as f64
        } else {
            data[i]
        };
    }

    smoothed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_curve(freq: Vec<f64>, spl: Vec<f64>) -> Curve {
        Curve {
            freq: Array1::from_vec(freq),
            spl: Array1::from_vec(spl),
            phase: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_rms_average_identical_curves() {
        let curve = make_curve(vec![100.0, 1000.0, 10000.0], vec![80.0, 85.0, 75.0]);
        let avg = rms_average(&[curve.clone(), curve.clone()]);

        // RMS average of identical curves should equal the original
        for i in 0..3 {
            assert!(
                (avg.spl[i] - curve.spl[i]).abs() < 0.01,
                "bin {}: expected {}, got {}",
                i,
                curve.spl[i],
                avg.spl[i]
            );
        }
    }

    #[test]
    fn test_rms_average_vs_arithmetic() {
        // RMS average should be higher than arithmetic mean of dB values
        // because averaging in power domain weights louder values more
        let c1 = make_curve(vec![100.0], vec![80.0]);
        let c2 = make_curve(vec![100.0], vec![90.0]);
        let avg = rms_average(&[c1, c2]);

        let arithmetic_mean = (80.0 + 90.0) / 2.0; // = 85.0
        assert!(
            avg.spl[0] > arithmetic_mean,
            "RMS average ({:.2}) should be > arithmetic mean ({:.2})",
            avg.spl[0],
            arithmetic_mean
        );
    }

    #[test]
    fn test_spatial_std_dev_identical() {
        let curve = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let std = spatial_std_dev(&[curve.clone(), curve.clone()]);
        assert!(std[0] < 0.01);
        assert!(std[1] < 0.01);
    }

    #[test]
    fn test_spatial_std_dev_different() {
        let c1 = make_curve(vec![100.0], vec![80.0]);
        let c2 = make_curve(vec![100.0], vec![86.0]);
        let std = spatial_std_dev(&[c1, c2]);

        // std_dev of [80, 86] = sqrt(((80-83)^2 + (86-83)^2) / 1) = sqrt(18) ≈ 4.24
        assert!(
            (std[0] - 4.24).abs() < 0.1,
            "expected ~4.24, got {}",
            std[0]
        );
    }

    #[test]
    fn test_spatial_std_dev_skewed_weights_do_not_zero_variance() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 80.0]);
        let c2 = make_curve(vec![100.0, 1000.0], vec![100.0, 100.0]);
        let c3 = make_curve(vec![100.0, 1000.0], vec![100.0, 100.0]);
        let std = spatial_std_dev_weighted(&[c1, c2, c3], Some(&[1.0, 1e-18, 1e-18]));

        assert!(
            std[0] > 0.0 && std[0].is_finite(),
            "skewed non-zero weights should not collapse variance to zero, got {}",
            std[0]
        );
    }

    #[test]
    #[should_panic(expected = "invalid frequency grid")]
    fn test_spatial_std_dev_rejects_mismatched_spl_lengths() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let c2 = make_curve(vec![100.0, 1000.0], vec![80.0]);
        let _ = spatial_std_dev(&[c1, c2]);
    }

    #[test]
    #[should_panic(expected = "invalid frequency grid")]
    fn test_rms_average_rejects_mismatched_spl_lengths() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let c2 = make_curve(vec![100.0, 1000.0], vec![80.0]);
        let _ = rms_average(&[c1, c2]);
    }

    #[test]
    fn test_correction_depth_low_variance() {
        let freq = Array1::from_vec(vec![100.0]);
        let variance = Array1::from_vec(vec![0.5]); // well below threshold
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0, // disable smoothing for test
            ..Default::default()
        };

        let depth = correction_depth_mask(&freq, &variance, &config);
        // sigmoid((3.0 - 0.5) / 2.0) = sigmoid(1.25) ≈ 0.777
        // depth = 0.1 + 0.9 * 0.777 ≈ 0.80
        assert!(
            depth[0] > 0.75,
            "low variance should give high correction, got {}",
            depth[0]
        );
    }

    #[test]
    fn test_correction_depth_high_variance() {
        let freq = Array1::from_vec(vec![100.0]);
        let variance = Array1::from_vec(vec![10.0]); // well above threshold
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };

        let depth = correction_depth_mask(&freq, &variance, &config);
        assert!(
            depth[0] < 0.3,
            "high variance should give reduced correction, got {}",
            depth[0]
        );
        assert!(
            depth[0] >= config.min_correction_depth,
            "should never go below min_correction_depth"
        );
    }

    #[test]
    fn test_correction_depth_at_threshold() {
        let freq = Array1::from_vec(vec![100.0]);
        let variance = Array1::from_vec(vec![3.0]); // exactly at threshold
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };

        let depth = correction_depth_mask(&freq, &variance, &config);
        // At threshold, sigmoid(0) = 0.5, so depth = min + (1-min)*0.5
        let expected = 0.1 + 0.9 * 0.5;
        assert!(
            (depth[0] - expected).abs() < 0.01,
            "expected ~{:.2}, got {:.2}",
            expected,
            depth[0]
        );
    }

    #[test]
    fn test_correction_depth_zero_transition_width() {
        // Bug fix: transition_width_db = 0.0 should use hard threshold (not divide by zero)
        let freq = Array1::from_vec(vec![100.0, 200.0]);
        let variance = Array1::from_vec(vec![1.0, 5.0]); // below and above threshold
        let config = SpatialRobustnessConfig {
            variance_threshold_db: 3.0,
            transition_width_db: 0.0, // hard threshold
            min_correction_depth: 0.1,
            mask_smoothing_octaves: 0.0,
        };

        let depth = correction_depth_mask(&freq, &variance, &config);

        // Below threshold → full correction
        assert!(
            depth[0] > 0.9,
            "below threshold should give full correction, got {}",
            depth[0]
        );
        // Above threshold → min correction
        assert!(
            (depth[1] - 0.1).abs() < 0.01,
            "above threshold should give min correction, got {}",
            depth[1]
        );
    }

    #[test]
    fn test_spatial_std_dev_single_curve() {
        // Bug fix: single curve should return zero variance (not panic)
        let curve = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let std = spatial_std_dev(&[curve]);
        assert_eq!(std[0], 0.0);
        assert_eq!(std[1], 0.0);
    }

    #[test]
    fn test_analyze_spatial_robustness_single_curve() {
        // Bug fix: single-curve analysis should work (not panic in spatial_std_dev)
        let curve = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };
        let result = analyze_spatial_robustness(&[curve], &config).expect("analysis");

        // Zero variance → high correction everywhere
        // sigmoid((3.0 - 0.0) / 2.0) ≈ 0.818, depth = 0.1 + 0.9 * 0.818 ≈ 0.836
        assert!(result.spatial_variance.iter().all(|&v| v == 0.0));
        assert!(
            result.correction_depth.iter().all(|&d| d > 0.8),
            "single curve should have high correction depth, got min={:.3}",
            result
                .correction_depth
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min)
        );
    }

    #[test]
    fn test_analyze_spatial_robustness_rejects_mismatched_frequency_grids() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let c2 = make_curve(vec![110.0, 1100.0], vec![80.0, 85.0]);
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };

        let err = analyze_spatial_robustness(&[c1, c2], &config).unwrap_err();
        assert!(
            err.to_string().contains("same frequency grid"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_full_analysis() {
        // Room mode at 100 Hz (consistent), comb filter at 5 kHz (inconsistent)
        let c1 = make_curve(vec![100.0, 5000.0], vec![90.0, 80.0]);
        let c2 = make_curve(vec![100.0, 5000.0], vec![91.0, 72.0]);
        let c3 = make_curve(vec![100.0, 5000.0], vec![89.0, 85.0]);

        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };
        let result = analyze_spatial_robustness(&[c1, c2, c3], &config).expect("analysis");

        // 100 Hz should have low variance → high correction depth
        assert!(result.spatial_variance[0] < 2.0);
        assert!(result.correction_depth[0] > 0.7);

        // 5 kHz should have high variance → low correction depth
        assert!(result.spatial_variance[1] > 5.0);
        assert!(result.correction_depth[1] < 0.5);
    }

    #[test]
    fn test_rms_average_negative_spl() {
        // Negative SPL values (relative measurements) should work correctly
        let c1 = make_curve(vec![100.0], vec![-10.0]);
        let c2 = make_curve(vec![100.0], vec![-20.0]);
        let avg = rms_average(&[c1, c2]);
        // RMS average in power domain: 10*log10(mean(10^(-10/10), 10^(-20/10)))
        // = 10*log10(mean(0.1, 0.01)) = 10*log10(0.055) ≈ -12.6 dB
        assert!(avg.spl[0] > -20.0 && avg.spl[0] < -10.0);
        assert!(avg.spl[0].is_finite());
    }

    #[test]
    fn test_smooth_log_frequency_reduces_variation() {
        // Smoothing should reduce rapid oscillations
        // Use wider frequency span so the window doesn't cover everything
        let freq = Array1::from_vec(vec![
            50.0, 70.0, 100.0, 140.0, 200.0, 280.0, 400.0, 560.0, 800.0, 1120.0, 1600.0,
        ]);
        let data = Array1::from_vec(vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);

        let smoothed = smooth_log_frequency(&data, &freq, 1.5); // 1.5 octave window

        // Smoothed should have less variation than original
        let orig_range = 1.0;
        let smooth_range = smoothed.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - smoothed.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(
            smooth_range < orig_range,
            "smoothing should reduce range: orig={:.2}, smoothed={:.2}",
            orig_range,
            smooth_range
        );
    }

    #[test]
    fn test_smooth_log_frequency_preserves_constant() {
        // Smoothing a constant array should return the same constant
        let freq = Array1::from_vec(vec![100.0, 200.0, 400.0, 800.0]);
        let data = Array1::from_vec(vec![0.5, 0.5, 0.5, 0.5]);
        let smoothed = smooth_log_frequency(&data, &freq, 1.0);
        for &v in smoothed.iter() {
            assert!((v - 0.5).abs() < 0.001);
        }
    }

    #[test]
    fn test_bootstrap_band_identical_curves_zero_width() {
        // All N curves identical → every resample produces the same RMS-mean →
        // band width must be zero at every frequency bin.
        let curve = make_curve(vec![100.0, 1000.0, 5000.0], vec![80.0, 85.0, 75.0]);
        let curves = vec![curve.clone(), curve.clone(), curve];
        let cfg = BootstrapConfig {
            num_resamples: 64,
            alpha: 0.10,
            seed: 1,
        };
        let band = bootstrap_band(&curves, &cfg, None).expect("bootstrap succeeds");
        for bin in 0..band.lower.spl.len() {
            assert!(
                (band.upper.spl[bin] - band.lower.spl[bin]).abs() < 1e-9,
                "bin {}: band width should be ~0, got {} - {}",
                bin,
                band.upper.spl[bin],
                band.lower.spl[bin]
            );
            assert!(
                band.per_bin_std[bin] < 1e-9,
                "bin {}: std should be ~0, got {}",
                bin,
                band.per_bin_std[bin]
            );
        }
    }

    #[test]
    fn test_bootstrap_band_determinism_under_fixed_seed() {
        // Same seed → identical bands; different seed → different bands.
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let c2 = make_curve(vec![100.0, 1000.0], vec![82.0, 90.0]);
        let c3 = make_curve(vec![100.0, 1000.0], vec![78.0, 80.0]);
        let curves = vec![c1, c2, c3];

        let cfg_a = BootstrapConfig {
            num_resamples: 100,
            alpha: 0.10,
            seed: 42,
        };
        let band_a1 = bootstrap_band(&curves, &cfg_a, None).expect("ok");
        let band_a2 = bootstrap_band(&curves, &cfg_a, None).expect("ok");
        for bin in 0..band_a1.lower.spl.len() {
            assert_eq!(band_a1.lower.spl[bin], band_a2.lower.spl[bin]);
            assert_eq!(band_a1.upper.spl[bin], band_a2.upper.spl[bin]);
        }

        let cfg_b = BootstrapConfig { seed: 7, ..cfg_a };
        let band_b = bootstrap_band(&curves, &cfg_b, None).expect("ok");
        // At least one bin should differ across seeds (with N=3, B=100, this is
        // overwhelmingly likely).
        let differs = (0..band_a1.lower.spl.len())
            .any(|bin| (band_a1.lower.spl[bin] - band_b.lower.spl[bin]).abs() > 1e-9);
        assert!(differs, "different seeds should produce different bands");
    }

    #[test]
    fn test_bootstrap_band_brackets_input_range() {
        // The α/2 lower percentile should be ≥ the min input SPL,
        // the 1-α/2 upper percentile should be ≤ the max input SPL,
        // both in dB after RMS-averaging (since every resample is a power-mean
        // of the input curves and a power-mean is bounded by min..max in dB).
        let c1 = make_curve(vec![100.0], vec![70.0]);
        let c2 = make_curve(vec![100.0], vec![80.0]);
        let c3 = make_curve(vec![100.0], vec![90.0]);
        let curves = vec![c1, c2, c3];
        let cfg = BootstrapConfig {
            num_resamples: 200,
            alpha: 0.10,
            seed: 99,
        };
        let band = bootstrap_band(&curves, &cfg, None).expect("ok");
        assert!(
            band.lower.spl[0] >= 70.0 - 1e-9,
            "lower {} should be >= 70.0",
            band.lower.spl[0]
        );
        assert!(
            band.upper.spl[0] <= 90.0 + 1e-9,
            "upper {} should be <= 90.0",
            band.upper.spl[0]
        );
        assert!(band.lower.spl[0] <= band.median.spl[0]);
        assert!(band.median.spl[0] <= band.upper.spl[0]);
    }

    #[test]
    fn test_bootstrap_band_alpha_widens_band() {
        // Smaller α → wider band (more conservative coverage).
        let c1 = make_curve(vec![100.0], vec![70.0]);
        let c2 = make_curve(vec![100.0], vec![90.0]);
        let curves = vec![c1, c2];
        let mk_cfg = |alpha| BootstrapConfig {
            num_resamples: 400,
            alpha,
            seed: 1,
        };
        let wide = bootstrap_band(&curves, &mk_cfg(0.01), None).expect("ok");
        let narrow = bootstrap_band(&curves, &mk_cfg(0.40), None).expect("ok");
        let wide_w = wide.upper.spl[0] - wide.lower.spl[0];
        let narrow_w = narrow.upper.spl[0] - narrow.lower.spl[0];
        assert!(
            wide_w >= narrow_w - 1e-9,
            "α=0.01 band width {} should be ≥ α=0.40 band width {}",
            wide_w,
            narrow_w
        );
    }

    #[test]
    fn test_bootstrap_resampled_curves_count() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let c2 = make_curve(vec![100.0, 1000.0], vec![82.0, 88.0]);
        let curves = vec![c1, c2];
        let cfg = BootstrapConfig {
            num_resamples: 13,
            alpha: 0.10,
            seed: 5,
        };
        let bank = bootstrap_resampled_curves(&curves, &cfg, None).expect("ok");
        assert_eq!(bank.len(), 13);
        for c in &bank {
            assert_eq!(c.freq.len(), 2);
            assert_eq!(c.spl.len(), 2);
        }
    }

    #[test]
    fn test_bootstrap_rejects_zero_resamples() {
        let curve = make_curve(vec![100.0], vec![80.0]);
        let cfg = BootstrapConfig {
            num_resamples: 0,
            alpha: 0.10,
            seed: 0,
        };
        assert!(bootstrap_band(&[curve.clone()], &cfg, None).is_err());
        assert!(bootstrap_resampled_curves(&[curve], &cfg, None).is_err());
    }

    #[test]
    fn test_bootstrap_rejects_alpha_out_of_range() {
        let curve = make_curve(vec![100.0], vec![80.0]);
        let cfg = BootstrapConfig {
            num_resamples: 10,
            alpha: 1.5,
            seed: 0,
        };
        assert!(bootstrap_band(&[curve], &cfg, None).is_err());
    }

    #[test]
    fn test_analyze_with_bootstrap_populates_field() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let c2 = make_curve(vec![100.0, 1000.0], vec![78.0, 88.0]);
        let cfg = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };
        let bcfg = BootstrapConfig {
            num_resamples: 32,
            alpha: 0.10,
            seed: 11,
        };
        let res =
            analyze_spatial_robustness_with_bootstrap(&[c1, c2], &cfg, &bcfg, None).expect("ok");
        assert!(res.bootstrap.is_some());
        let band = res.bootstrap.unwrap();
        assert_eq!(band.lower.spl.len(), 2);
        assert_eq!(band.median.spl.len(), 2);
        assert_eq!(band.upper.spl.len(), 2);
        assert_eq!(band.per_bin_std.len(), 2);
    }

    #[test]
    fn test_correction_depth_with_smoothing_enabled() {
        // Test that smoothing doesn't produce NaN or out-of-range values
        let freq = Array1::from_vec(vec![50.0, 100.0, 200.0, 500.0, 1000.0]);
        let variance = Array1::from_vec(vec![1.0, 8.0, 1.0, 8.0, 1.0]);
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.5, // smoothing enabled
            ..Default::default()
        };

        let depth = correction_depth_mask(&freq, &variance, &config);
        for &d in depth.iter() {
            assert!(d.is_finite(), "depth should be finite");
            assert!(
                (0.0..=1.0).contains(&d),
                "depth should be in [0, 1], got {}",
                d
            );
        }
    }
}
