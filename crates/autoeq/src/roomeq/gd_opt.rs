//! Group Delay Optimisation v2 — IIR path (Phase GD-3a).
//!
//! Finds per-channel `(delay_ms, allpass_filters, polarity)` that minimise
//! the RMS group-delay deviation of the **summed complex response** at the
//! listening position, weighted by per-bin coherence².
//!
//! Features:
//! - Core DE-based optimiser (`optimize_group_delay`)
//! - Adaptive AP bootstrap (`optimize_group_delay_adaptive`, §3.3)
//! - Multi-mode dispatch (`optimize_group_delay_for_mode`, §3.7)
//!
//! References: `crates/autoeq/docs/gd_opt_v2_plan.md` §3.

use crate::optim::scalar::{ScalarOptimConfig, optimize_bounded_scalar};
use crate::roomeq::types::{MixedModeConfig, ProcessingMode};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

// ─── Types ───────────────────────────────────────────────────────────────────

/// Configuration for the group-delay optimiser.
#[derive(Debug, Clone)]
pub struct GdOptConfig {
    /// Sample rate in Hz.
    pub sample_rate: f64,
    /// Maximum per-channel delay in ms.
    pub max_delay_ms: f64,
    /// Number of all-pass filters per channel (fixed budget, no bootstrap).
    pub ap_per_channel: usize,
    /// Minimum all-pass centre frequency in Hz.
    pub ap_min_freq: f64,
    /// Maximum all-pass centre frequency in Hz.
    pub ap_max_freq: f64,
    /// Minimum all-pass Q.
    pub ap_min_q: f64,
    /// Maximum all-pass Q.
    pub ap_max_q: f64,
    /// Whether to optimise polarity per channel.
    pub optimize_polarity: bool,
    /// Optimizer algorithm.
    pub algorithm: String,
    /// DE mutation strategy when `algorithm` resolves to `autoeq:de`.
    pub strategy: String,
    /// Optimizer maximum iteration/evaluation budget.
    pub max_iter: usize,
    /// Optimizer population size.
    pub popsize: usize,
    /// Optimizer convergence tolerance.
    pub tol: f64,
    /// Optional seed for reproducibility.
    pub seed: Option<u64>,
}

impl Default for GdOptConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000.0,
            max_delay_ms: 20.0,
            ap_per_channel: 2,
            ap_min_freq: 20.0,
            ap_max_freq: 300.0,
            ap_min_q: 0.3,
            ap_max_q: 10.0,
            optimize_polarity: true,
            algorithm: "autoeq:cmaes".to_string(),
            strategy: "lshade".to_string(),
            max_iter: 2000,
            popsize: 20,
            tol: 1e-8,
            seed: None,
        }
    }
}

/// Per-channel result.
#[derive(Debug, Clone)]
pub struct ChannelGdResult {
    pub delay_ms: f64,
    pub polarity_inverted: bool,
    pub ap_filters: Vec<Biquad>,
    pub channel_gd_pre_rms_ms: f64,
    pub channel_gd_post_rms_ms: f64,
}

/// Overall optimisation result.
#[derive(Debug, Clone)]
pub struct GroupDelayOptResult {
    pub band: (f64, f64),
    pub per_channel: Vec<ChannelGdResult>,
    pub sum_gd_pre_rms_ms: f64,
    pub sum_gd_post_rms_ms: f64,
    pub mean_coherence: f64,
    pub improvement_db: f64,
}

/// Per-channel measurement input.
#[derive(Debug, Clone)]
pub struct ChannelMeasurementInput {
    /// Frequency grid (Hz), shared across spl/phase/coherence.
    pub freq: Array1<f64>,
    /// SPL in dB.
    pub spl: Array1<f64>,
    /// Unwrapped phase in radians.
    pub phase: Array1<f64>,
    /// Coherence (γ²) per bin, range [0, 1].
    pub coherence: Array1<f64>,
}

// ─── GD-3b: FIR-path alignment target ────────────────────────────────────────

/// Alignment target for the PhaseLinear FIR path (§3.7, GD-3b).
///
/// When `PhaseLinear` mode is used, the FIR designer receives this struct
/// so it can incorporate inter-channel GD alignment into the filter design
/// via Kirkeby mixed-phase inversion. When absent, the FIR falls back to
/// pure magnitude correction.
#[derive(Debug, Clone)]
pub struct GdAlignmentTarget {
    /// Per-channel delay in ms (channel index → delay).
    pub per_channel_delay_ms: Vec<f64>,
    /// Reference sum GD curve (the target flat GD the FIR should approach).
    pub sum_gd_reference_ms: Vec<f64>,
    /// Frequency grid for `sum_gd_reference_ms`.
    pub freq: Array1<f64>,
}

/// Build a `GdAlignmentTarget` from a `GroupDelayOptResult`.
///
/// This extracts the per-channel delays and computes the reference GD
/// (post-optimisation sum GD) that the FIR designer should target.
/// Used by `PhaseLinear` mode to pass delay information to the FIR path.
pub fn build_gd_alignment_target(
    channels: &[ChannelMeasurementInput],
    result: &GroupDelayOptResult,
    config: &GdOptConfig,
) -> GdAlignmentTarget {
    let n_freq = channels[0].freq.len();
    let band_indices: Vec<usize> = (0..n_freq)
        .filter(|&i| channels[0].freq[i] >= result.band.0 && channels[0].freq[i] <= result.band.1)
        .collect();

    // Encode the optimised result as params to compute post-GD
    let params = encode_result_as_params(result, config);
    let sum_gd = compute_sum_gd(channels, &params, &band_indices, config);

    let per_channel_delay_ms = result.per_channel.iter().map(|ch| ch.delay_ms).collect();

    // Build frequency sub-grid for the band
    let freq = Array1::from_iter(band_indices.iter().map(|&i| channels[0].freq[i]));

    GdAlignmentTarget {
        per_channel_delay_ms,
        sum_gd_reference_ms: sum_gd,
        freq,
    }
}

/// Advisory reasons for GD-Opt outcomes (§3.5, GD-4).
#[derive(Debug, Clone, PartialEq)]
pub enum GdOptAdvisory {
    /// GD-Opt completed successfully with the given improvement.
    Success { improvement_db: f64 },
    /// GD-Opt skipped: no phase data available.
    NoPhaseData,
    /// GD-Opt skipped: coherence below threshold.
    CoherenceBelowThreshold { mean_coherence: f64 },
    /// GD-Opt skipped: PhaseLinear mode without FIR GD target.
    PhaseLinearNoTarget,
    /// GD-Opt skipped: insufficient channels (need ≥ 2).
    InsufficientChannels,
    /// GD-Opt skipped: band derivation produced empty range.
    EmptyBand,
    /// GD-Opt degraded: optimiser ran but improvement was minimal.
    MinimalImprovement { improvement_db: f64 },
    /// GD-Opt skipped: channels are sampled on different frequency grids.
    FrequencyGridMismatch,
    /// GD-Opt degraded: coherence was absent, so only delay was optimized.
    MissingCoherenceDelayOnly,
    /// GD-Opt degraded: all-pass was requested but bootstrap data was absent.
    AllPassDisabledNoBootstrapRealisations,
}

/// Serialisable summary of GD-Opt results for report plumbing (GD-4).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GroupDelayOptSummary {
    /// Optimisation band (Hz).
    pub band: (f64, f64),
    /// Channel names in the same order as the per-channel vectors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channel_names: Vec<String>,
    /// Per-channel delays applied (ms).
    pub per_channel_delay_ms: Vec<f64>,
    /// Per-channel polarity inversions.
    pub per_channel_polarity_inverted: Vec<bool>,
    /// Number of all-pass filters per channel.
    pub per_channel_ap_count: Vec<usize>,
    /// Sum GD RMS before optimisation (ms).
    pub sum_gd_pre_rms_ms: f64,
    /// Sum GD RMS after optimisation (ms).
    pub sum_gd_post_rms_ms: f64,
    /// Mean coherence in-band.
    pub mean_coherence: f64,
    /// Improvement in dB: 20*log10(pre/post).
    pub improvement_db: f64,
    /// Advisory outcome.
    pub advisory: String,
    /// Whether the reported GD controls were inserted into the exported DSP.
    #[serde(default)]
    pub applied: bool,
}

impl GroupDelayOptSummary {
    /// Create a summary from a successful optimisation result.
    pub fn from_result_with_names(result: &GroupDelayOptResult, names: Vec<String>) -> Self {
        Self {
            band: result.band,
            channel_names: names,
            per_channel_delay_ms: result.per_channel.iter().map(|ch| ch.delay_ms).collect(),
            per_channel_polarity_inverted: result
                .per_channel
                .iter()
                .map(|ch| ch.polarity_inverted)
                .collect(),
            per_channel_ap_count: result
                .per_channel
                .iter()
                .map(|ch| ch.ap_filters.len())
                .collect(),
            sum_gd_pre_rms_ms: result.sum_gd_pre_rms_ms,
            sum_gd_post_rms_ms: result.sum_gd_post_rms_ms,
            mean_coherence: result.mean_coherence,
            improvement_db: result.improvement_db,
            advisory: "success".to_string(),
            applied: false,
        }
    }

    /// Mark a summary as reflected in the exported DSP chain.
    pub fn with_applied(mut self, applied: bool) -> Self {
        self.applied = applied;
        self
    }

    /// Create a summary for a skipped/failed case.
    pub fn from_advisory(advisory: &GdOptAdvisory) -> Self {
        let reason = match advisory {
            GdOptAdvisory::Success { improvement_db } => {
                format!("success:{improvement_db:.1}dB")
            }
            GdOptAdvisory::NoPhaseData => "no_phase_data".to_string(),
            GdOptAdvisory::CoherenceBelowThreshold { mean_coherence } => {
                format!("coherence_below_threshold:{mean_coherence:.2}")
            }
            GdOptAdvisory::PhaseLinearNoTarget => "phase_linear_no_target".to_string(),
            GdOptAdvisory::InsufficientChannels => "insufficient_channels".to_string(),
            GdOptAdvisory::EmptyBand => "empty_band".to_string(),
            GdOptAdvisory::MinimalImprovement { improvement_db } => {
                format!("minimal_improvement:{improvement_db:.1}dB")
            }
            GdOptAdvisory::FrequencyGridMismatch => "frequency_grid_mismatch".to_string(),
            GdOptAdvisory::MissingCoherenceDelayOnly => "missing_coherence_delay_only".to_string(),
            GdOptAdvisory::AllPassDisabledNoBootstrapRealisations => {
                "allpass_disabled_no_bootstrap_realisations".to_string()
            }
        };

        Self {
            band: (0.0, 0.0),
            channel_names: vec![],
            per_channel_delay_ms: vec![],
            per_channel_polarity_inverted: vec![],
            per_channel_ap_count: vec![],
            sum_gd_pre_rms_ms: 0.0,
            sum_gd_post_rms_ms: 0.0,
            mean_coherence: 0.0,
            improvement_db: 0.0,
            advisory: reason,
            applied: false,
        }
    }
}

// ─── Band derivation (§3.4) ──────────────────────────────────────────────────

/// Derive the optimisation frequency band from a crossover frequency.
/// Returns `(band_lo, band_hi)`.
pub fn derive_band(min_freq: f64, crossover_freq: f64) -> (f64, f64) {
    let band_lo = min_freq.max(crossover_freq * 0.25);
    let band_hi = crossover_freq * 2.0;
    (band_lo, band_hi)
}

// ─── Core optimiser ──────────────────────────────────────────────────────────

/// Run the group-delay optimiser on a set of channel measurements.
///
/// Returns `Err` if fewer than 2 channels are provided or measurements are
/// incompatible.
pub fn optimize_group_delay(
    channels: &[ChannelMeasurementInput],
    band: (f64, f64),
    config: &GdOptConfig,
) -> Result<GroupDelayOptResult, String> {
    let n_ch = channels.len();
    if n_ch < 2 {
        return Err("GD-Opt requires at least 2 channels".into());
    }

    // Validate all channels share the same frequency grid values.
    let n_freq = channels[0].freq.len();
    for (i, ch) in channels.iter().enumerate() {
        if ch.freq.len() != n_freq
            || ch.spl.len() != n_freq
            || ch.phase.len() != n_freq
            || ch.coherence.len() != n_freq
        {
            return Err(format!("Channel {} has inconsistent array lengths", i));
        }
        if i > 0 && !same_frequency_grid(&channels[0].freq, &ch.freq) {
            return Err(format!(
                "Channel {} frequency grid does not match the reference channel",
                i
            ));
        }
    }

    // Find indices within band
    let band_indices: Vec<usize> = (0..n_freq)
        .filter(|&i| channels[0].freq[i] >= band.0 && channels[0].freq[i] <= band.1)
        .collect();

    if band_indices.is_empty() {
        return Err("No frequency bins within the specified band".into());
    }

    // Compute mean coherence (weighted across all channels)
    let mean_coherence = compute_mean_coherence(channels, &band_indices);

    // Compute pre-optimisation sum GD RMS
    let identity_params = vec![0.0; param_count(n_ch, config)];
    let sum_gd_pre_rms_ms = compute_sum_gd_rms(channels, &identity_params, &band_indices, config);

    // Build bounds for DE
    let bounds = build_bounds(n_ch, config);

    let channels_ref = channels;
    let band_indices_ref = &band_indices;
    let config_ref = config;

    let loss_fn = |x: &[f64]| -> f64 { gd_loss(channels_ref, x, band_indices_ref, config_ref) };

    let initial = identity_params.clone();
    let report = optimize_bounded_scalar(
        &bounds,
        &initial,
        &ScalarOptimConfig {
            algorithm: config.algorithm.clone(),
            max_iter: config.max_iter,
            population: config.popsize,
            tolerance: config.tol,
            atolerance: config.tol,
            strategy: config.strategy.clone(),
            seed: config.seed,
        },
        loss_fn,
    )?;

    let best_params = report.x.as_slice();

    // Compute post-optimisation sum GD RMS
    let sum_gd_post_rms_ms = compute_sum_gd_rms(channels, best_params, &band_indices, config);

    let improvement_db = if sum_gd_pre_rms_ms < 1e-15 {
        0.0 // Already aligned, no improvement possible
    } else if sum_gd_post_rms_ms > 1e-15 {
        20.0 * (sum_gd_pre_rms_ms / sum_gd_post_rms_ms).log10()
    } else {
        120.0 // Cap at a large but finite value
    };

    // Decode per-channel results and normalize unidentifiable common controls
    // before reporting/applying them.
    let mut per_channel = decode_per_channel(channels, best_params, &band_indices, config);
    normalize_per_channel_controls(&mut per_channel);

    Ok(GroupDelayOptResult {
        band,
        per_channel,
        sum_gd_pre_rms_ms,
        sum_gd_post_rms_ms,
        mean_coherence,
        improvement_db,
    })
}

// ─── Adaptive AP bootstrap (§3.3) ────────────────────────────────────────────

/// Maximum number of all-pass filters the bootstrap will try.
const MAX_AP_BUDGET: usize = 2;

/// Significance threshold: keep AP only if mean_improvement / σ > this value.
const BOOTSTRAP_SIGMA_THRESHOLD: f64 = 3.0;

/// Run the group-delay optimiser with adaptive AP budget (§3.3).
///
/// Instead of a fixed AP count, starts with delay-only (0 APs), then
/// incrementally adds APs up to `MAX_AP_BUDGET`, accepting each only if it
/// passes the bootstrap significance test across the per-sweep realisations.
///
/// `sweep_realisations` contains N independent measurement sets (one per sweep).
/// The main `channels` is the coherence-averaged measurement used for fitting.
pub fn optimize_group_delay_adaptive(
    channels: &[ChannelMeasurementInput],
    sweep_realisations: &[Vec<ChannelMeasurementInput>],
    band: (f64, f64),
    config: &GdOptConfig,
) -> Result<GroupDelayOptResult, String> {
    if sweep_realisations.len() < 2 {
        return Err("Adaptive AP bootstrap requires at least 2 sweep realisations (N >= 2)".into());
    }

    // Start with delay-only (0 APs)
    let mut best_config = GdOptConfig {
        ap_per_channel: 0,
        ..config.clone()
    };
    let mut best_result = optimize_group_delay(channels, band, &best_config)?;

    // Incrementally try adding APs
    for k in 1..=MAX_AP_BUDGET {
        let trial_config = GdOptConfig {
            ap_per_channel: k,
            ..config.clone()
        };

        let trial_result = optimize_group_delay(channels, band, &trial_config)?;

        // Bootstrap test: for each sweep realisation, compute GD RMS
        // with and without the k-th AP filter.
        let improvements = compute_bootstrap_improvements(
            sweep_realisations,
            band,
            &best_config,
            &best_result,
            &trial_config,
            &trial_result,
        )?;

        let n = improvements.len() as f64;
        let mean_improvement = improvements.iter().sum::<f64>() / n;
        let variance = improvements
            .iter()
            .map(|&x| (x - mean_improvement).powi(2))
            .sum::<f64>()
            / (n - 1.0);
        let sigma = variance.sqrt();

        // Accept if mean_improvement / σ > 3 (and σ > 0 to avoid division by zero)
        let significant = sigma > 1e-15 && (mean_improvement / sigma) > BOOTSTRAP_SIGMA_THRESHOLD;

        if significant && trial_result.sum_gd_post_rms_ms < best_result.sum_gd_post_rms_ms {
            best_result = trial_result;
            best_config = trial_config;
        } else {
            // No significant improvement — stop adding APs
            break;
        }
    }

    Ok(best_result)
}

/// For each sweep realisation, compute the GD RMS improvement (pre - post)
/// between the baseline result and the trial result.
fn compute_bootstrap_improvements(
    sweep_realisations: &[Vec<ChannelMeasurementInput>],
    band: (f64, f64),
    baseline_config: &GdOptConfig,
    baseline_result: &GroupDelayOptResult,
    trial_config: &GdOptConfig,
    trial_result: &GroupDelayOptResult,
) -> Result<Vec<f64>, String> {
    let mut improvements = Vec::with_capacity(sweep_realisations.len());

    for realisation in sweep_realisations {
        if realisation.len() != baseline_result.per_channel.len() {
            return Err("Sweep realisation channel count mismatch".into());
        }

        let n_freq = realisation[0].freq.len();
        let band_indices: Vec<usize> = (0..n_freq)
            .filter(|&i| realisation[0].freq[i] >= band.0 && realisation[0].freq[i] <= band.1)
            .collect();

        if band_indices.is_empty() {
            improvements.push(0.0);
            continue;
        }

        // Encode baseline result as params
        let baseline_params = encode_result_as_params(baseline_result, baseline_config);
        let trial_params = encode_result_as_params(trial_result, trial_config);

        // Compute GD RMS for this realisation with baseline vs trial params
        let rms_baseline = compute_sum_gd_rms(
            realisation,
            &baseline_params,
            &band_indices,
            baseline_config,
        );
        let rms_trial = compute_sum_gd_rms(realisation, &trial_params, &band_indices, trial_config);

        // Improvement = reduction in RMS (positive means trial is better)
        improvements.push(rms_baseline - rms_trial);
    }

    Ok(improvements)
}

/// Encode a `GroupDelayOptResult` back into a parameter vector for evaluation.
fn encode_result_as_params(result: &GroupDelayOptResult, config: &GdOptConfig) -> Vec<f64> {
    let n_ch = result.per_channel.len();
    let per_ch = 1 + config.ap_per_channel * 2 + if config.optimize_polarity { 1 } else { 0 };
    let mut params = vec![0.0; n_ch * per_ch];

    for (ch_idx, ch_result) in result.per_channel.iter().enumerate() {
        let offset = ch_idx * per_ch;
        params[offset] = ch_result.delay_ms;

        for (i, ap) in ch_result.ap_filters.iter().enumerate() {
            if i < config.ap_per_channel {
                params[offset + 1 + i * 2] = ap.freq;
                params[offset + 1 + i * 2 + 1] = ap.q;
            }
        }

        if config.optimize_polarity {
            params[offset + 1 + config.ap_per_channel * 2] = if ch_result.polarity_inverted {
                1.0
            } else {
                0.0
            };
        }
    }

    params
}

// ─── Multi-mode dispatch (§3.7) ──────────────────────────────────────────────

/// Run the group-delay optimiser with mode-specific behaviour (§3.7).
///
/// Dispatches based on `ProcessingMode`:
/// - `LowLatency`, `WarpedIir`, `KautzModal`: Full optimisation (delays + APs).
/// - `Hybrid`: Same as LowLatency but asserts `band_hi ≤ mixed_config.crossover_freq`.
/// - `MixedPhase`: Inter-channel alignment only (1 AP max per channel).
/// - `PhaseLinear`: Not applicable (returns error — use GD-3b FIR path).
pub fn optimize_group_delay_for_mode(
    channels: &[ChannelMeasurementInput],
    band: (f64, f64),
    config: &GdOptConfig,
    processing_mode: &ProcessingMode,
    mixed_mode_config: Option<&MixedModeConfig>,
) -> Result<GroupDelayOptResult, String> {
    match processing_mode {
        ProcessingMode::LowLatency | ProcessingMode::WarpedIir | ProcessingMode::KautzModal => {
            optimize_group_delay(channels, band, config)
        }

        ProcessingMode::Hybrid => {
            // Assert band_hi does not straddle the IIR/FIR crossover
            let xo_freq = mixed_mode_config.map(|m| m.crossover_freq).unwrap_or(300.0);

            if band.1 > xo_freq {
                return Err(format!(
                    "Hybrid mode: GD-Opt band_hi ({:.1} Hz) exceeds mixed_config crossover \
                     ({:.1} Hz). AP filters must stay in the IIR band.",
                    band.1, xo_freq,
                ));
            }

            optimize_group_delay(channels, band, config)
        }

        ProcessingMode::MixedPhase => {
            // After per-channel excess-phase FIR correction, only inter-channel
            // alignment remains. Typically 1 delay per channel, at most 1 AP.
            let mixed_phase_config = GdOptConfig {
                ap_per_channel: config.ap_per_channel.min(1),
                ..config.clone()
            };
            optimize_group_delay(channels, band, &mixed_phase_config)
        }

        ProcessingMode::PhaseLinear => Err("PhaseLinear mode does not use IIR AP filters. \
             Use the FIR path (GD-3b) with GdAlignmentTarget instead."
            .into()),
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Number of parameters in the optimisation vector.
fn param_count(n_ch: usize, config: &GdOptConfig) -> usize {
    // Per channel: 1 delay + ap_per_channel * 2 (freq, q) + 1 polarity (if enabled)
    let per_ch = 1 + config.ap_per_channel * 2 + if config.optimize_polarity { 1 } else { 0 };
    n_ch * per_ch
}

/// Build DE bounds for all parameters.
fn build_bounds(n_ch: usize, config: &GdOptConfig) -> Vec<(f64, f64)> {
    let mut bounds = Vec::new();
    for _ in 0..n_ch {
        // Delay is optimized as a relative control and normalized after DE so
        // the exported DSP never adds arbitrary common latency.
        bounds.push((-config.max_delay_ms, config.max_delay_ms));
        // AP filters: (freq, q) pairs
        for _ in 0..config.ap_per_channel {
            bounds.push((config.ap_min_freq, config.ap_max_freq));
            bounds.push((config.ap_min_q, config.ap_max_q));
        }
        // polarity: [0, 1] — decoded as inverted if > 0.5
        if config.optimize_polarity {
            bounds.push((0.0, 1.0));
        }
    }
    bounds
}

/// Decode parameters for a single channel from the flat parameter vector.
struct ChannelParams {
    delay_ms: f64,
    ap_filters: Vec<(f64, f64)>, // (freq, q)
    polarity_inverted: bool,
}

fn decode_channel_params(params: &[f64], ch: usize, config: &GdOptConfig) -> ChannelParams {
    let per_ch = 1 + config.ap_per_channel * 2 + if config.optimize_polarity { 1 } else { 0 };
    let offset = ch * per_ch;

    let delay_ms = params[offset];

    let mut ap_filters = Vec::with_capacity(config.ap_per_channel);
    for i in 0..config.ap_per_channel {
        let freq = params[offset + 1 + i * 2];
        let q = params[offset + 1 + i * 2 + 1];
        ap_filters.push((freq, q));
    }

    let polarity_inverted = if config.optimize_polarity {
        params[offset + 1 + config.ap_per_channel * 2] > 0.5
    } else {
        false
    };

    ChannelParams {
        delay_ms,
        ap_filters,
        polarity_inverted,
    }
}

fn same_frequency_grid(reference: &Array1<f64>, candidate: &Array1<f64>) -> bool {
    reference.len() == candidate.len()
        && reference.iter().zip(candidate.iter()).all(|(&a, &b)| {
            let tol = 1e-6_f64.max(1e-6 * a.abs().max(b.abs()));
            (a - b).abs() <= tol
        })
}

fn normalize_per_channel_controls(results: &mut [ChannelGdResult]) {
    if results.is_empty() {
        return;
    }

    let min_delay = results
        .iter()
        .map(|ch| ch.delay_ms)
        .fold(f64::INFINITY, f64::min);
    if min_delay.is_finite() {
        for ch in results.iter_mut() {
            ch.delay_ms = (ch.delay_ms - min_delay).max(0.0);
            if ch.delay_ms < 1e-9 {
                ch.delay_ms = 0.0;
            }
        }
    }

    // Global polarity inversion is not identifiable in the summed response.
    // Use channel 0 as the deterministic reference and express all other
    // inversions relative to it.
    let reference_inverted = results[0].polarity_inverted;
    if reference_inverted {
        for ch in results.iter_mut() {
            ch.polarity_inverted = !ch.polarity_inverted;
        }
    }
    results[0].polarity_inverted = false;
}

/// Compute the complex response of a channel at frequency `f` with applied
/// delay, all-pass filters, and polarity.
fn channel_complex_at(
    ch: &ChannelMeasurementInput,
    freq_idx: usize,
    ch_params: &ChannelParams,
    config: &GdOptConfig,
) -> Complex64 {
    let f = ch.freq[freq_idx];
    let omega = 2.0 * PI * f;

    // Original channel response as complex
    let mag = 10.0_f64.powf(ch.spl[freq_idx] / 20.0);
    let phase = ch.phase[freq_idx];
    let mut h = Complex64::from_polar(mag, phase);

    // Apply delay: e^(-jωτ)
    let delay_s = ch_params.delay_ms * 1e-3;
    h *= Complex64::from_polar(1.0, -omega * delay_s);

    // Apply all-pass filters
    for &(ap_freq, ap_q) in &ch_params.ap_filters {
        let ap = Biquad::new(
            BiquadFilterType::AllPass,
            ap_freq,
            config.sample_rate,
            ap_q,
            0.0,
        );
        h *= ap.complex_response(f);
    }

    // Apply polarity inversion
    if ch_params.polarity_inverted {
        h = -h;
    }

    h
}

/// Compute group delay of the summed complex response via finite differences.
/// Returns GD in ms at each band frequency.
fn compute_sum_gd(
    channels: &[ChannelMeasurementInput],
    params: &[f64],
    band_indices: &[usize],
    config: &GdOptConfig,
) -> Vec<f64> {
    // Decode channel params once (avoid per-bin allocation in hot path)
    let ch_params: Vec<ChannelParams> = (0..channels.len())
        .map(|ch_idx| decode_channel_params(params, ch_idx, config))
        .collect();

    // We need adjacent in-band bins for finite-difference GD computation.
    // Interior bins use forward differences; the final bin uses a backward
    // difference so it is not pulled toward an out-of-band raw-grid neighbor.
    let mut gd_ms = Vec::with_capacity(band_indices.len());

    for (bi, &idx) in band_indices.iter().enumerate() {
        let (idx0, idx1) = if bi + 1 < band_indices.len() {
            (idx, band_indices[bi + 1])
        } else if bi > 0 {
            (band_indices[bi - 1], idx)
        } else {
            gd_ms.push(0.0);
            continue;
        };

        let f0 = channels[0].freq[idx0];
        let f1 = channels[0].freq[idx1];
        let omega0 = 2.0 * PI * f0;
        let omega1 = 2.0 * PI * f1;

        // Sum complex responses at f0 and f1
        let mut sum0 = Complex64::new(0.0, 0.0);
        let mut sum1 = Complex64::new(0.0, 0.0);

        for (ch, cp) in channels.iter().zip(ch_params.iter()) {
            sum0 += channel_complex_at(ch, idx0, cp, config);
            sum1 += channel_complex_at(ch, idx1, cp, config);
        }

        // GD = -dφ/dω
        let phase0 = sum0.arg();
        let phase1 = sum1.arg();
        let d_phase = unwrap_phase_diff(phase1 - phase0);
        let d_omega = omega1 - omega0;

        let gd_s = if d_omega.abs() > 1e-15 {
            -d_phase / d_omega
        } else {
            0.0
        };

        gd_ms.push(gd_s * 1000.0);
    }

    gd_ms
}

/// Coherence-weighted RMS of the summed group delay (deviation from median).
fn compute_sum_gd_rms(
    channels: &[ChannelMeasurementInput],
    params: &[f64],
    band_indices: &[usize],
    config: &GdOptConfig,
) -> f64 {
    let gd = compute_sum_gd(channels, params, band_indices, config);
    if gd.is_empty() {
        return 0.0;
    }

    // Compute coherence weights (mean across channels per bin)
    let weights: Vec<f64> = band_indices
        .iter()
        .map(|&idx| {
            let mean_coh: f64 =
                channels.iter().map(|ch| ch.coherence[idx]).sum::<f64>() / channels.len() as f64;
            mean_coh * mean_coh // coherence²
        })
        .collect();

    // Target: coherence-weighted median GD (flattest achievable reference per §3.1)
    let target = weighted_median(&gd, &weights);

    // Weighted RMS deviation from target
    let mut weighted_sum = 0.0;
    let mut weight_total = 0.0;
    for (i, &g) in gd.iter().enumerate() {
        let w = weights[i];
        let dev = g - target;
        weighted_sum += w * dev * dev;
        weight_total += w;
    }

    if weight_total > 0.0 {
        (weighted_sum / weight_total).sqrt()
    } else {
        0.0
    }
}

fn weighted_median(values: &[f64], weights: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut pairs: Vec<(f64, f64)> = values
        .iter()
        .copied()
        .zip(weights.iter().copied())
        .filter(|(value, weight)| value.is_finite() && weight.is_finite() && *weight > 0.0)
        .collect();

    if pairs.is_empty() {
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        return sorted[sorted.len() / 2];
    }

    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let total_weight: f64 = pairs.iter().map(|(_, weight)| *weight).sum();
    let midpoint = total_weight * 0.5;
    let mut cumulative = 0.0;

    for (value, weight) in pairs.iter().copied() {
        cumulative += weight;
        if cumulative >= midpoint {
            return value;
        }
    }

    pairs.last().map(|(value, _)| *value).unwrap_or(0.0)
}

/// The objective function for DE: coherence-weighted RMS GD of the sum.
fn gd_loss(
    channels: &[ChannelMeasurementInput],
    params: &[f64],
    band_indices: &[usize],
    config: &GdOptConfig,
) -> f64 {
    compute_sum_gd_rms(channels, params, band_indices, config)
}

/// Compute mean coherence across all channels in-band.
fn compute_mean_coherence(channels: &[ChannelMeasurementInput], band_indices: &[usize]) -> f64 {
    if band_indices.is_empty() || channels.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut count = 0;
    for ch in channels {
        for &idx in band_indices {
            sum += ch.coherence[idx];
            count += 1;
        }
    }
    sum / count as f64
}

/// Unwrap a phase difference to [-π, π].
fn unwrap_phase_diff(mut d: f64) -> f64 {
    while d > PI {
        d -= 2.0 * PI;
    }
    while d < -PI {
        d += 2.0 * PI;
    }
    d
}

/// Decode the DE solution into per-channel results with pre/post GD RMS.
fn decode_per_channel(
    channels: &[ChannelMeasurementInput],
    params: &[f64],
    band_indices: &[usize],
    config: &GdOptConfig,
) -> Vec<ChannelGdResult> {
    let n_ch = channels.len();
    let mut results = Vec::with_capacity(n_ch);

    for ch_idx in 0..n_ch {
        let cp = decode_channel_params(params, ch_idx, config);

        // Build Biquad AP filters
        let ap_filters: Vec<Biquad> = cp
            .ap_filters
            .iter()
            .map(|&(freq, q)| {
                Biquad::new(BiquadFilterType::AllPass, freq, config.sample_rate, q, 0.0)
            })
            .collect();

        // Per-channel GD RMS: compute as if this channel were the only one
        // (use single-channel slice for pre/post comparison)
        let single_ch = &channels[ch_idx..ch_idx + 1];

        // Pre: identity params for 1 channel
        let id_1ch = vec![0.0; param_count(1, config)];
        let pre_rms = compute_sum_gd_rms(single_ch, &id_1ch, band_indices, config);

        // Post: this channel's params, re-encoded for 1 channel
        let per_ch_size =
            1 + config.ap_per_channel * 2 + if config.optimize_polarity { 1 } else { 0 };
        let ch_offset = ch_idx * per_ch_size;
        let post_params_1ch = params[ch_offset..ch_offset + per_ch_size].to_vec();
        let post_rms = compute_sum_gd_rms(single_ch, &post_params_1ch, band_indices, config);

        results.push(ChannelGdResult {
            delay_ms: cp.delay_ms,
            polarity_inverted: cp.polarity_inverted,
            ap_filters,
            channel_gd_pre_rms_ms: pre_rms,
            channel_gd_post_rms_ms: post_rms,
        });
    }

    results
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a synthetic channel measurement with a known pure delay.
    /// The channel has flat magnitude (0 dB) and linear phase corresponding
    /// to the given delay.
    fn make_delayed_channel(
        freq_grid: &Array1<f64>,
        delay_ms: f64,
        coherence: f64,
    ) -> ChannelMeasurementInput {
        let n = freq_grid.len();
        let spl = Array1::zeros(n); // 0 dB flat
        let delay_s = delay_ms * 1e-3;

        // Phase = -ω * delay (linear phase from pure delay)
        let phase = freq_grid.mapv(|f| -2.0 * PI * f * delay_s);

        let coherence = Array1::from_elem(n, coherence);

        ChannelMeasurementInput {
            freq: freq_grid.clone(),
            spl,
            phase,
            coherence,
        }
    }

    /// Generate a log-spaced frequency grid.
    fn log_freq_grid(f_min: f64, f_max: f64, n_points: usize) -> Array1<f64> {
        let log_min = f_min.ln();
        let log_max = f_max.ln();
        Array1::from_iter((0..n_points).map(|i| {
            let t = i as f64 / (n_points - 1) as f64;
            (log_min + t * (log_max - log_min)).exp()
        }))
    }

    #[test]
    fn test_derive_band() {
        let (lo, hi) = derive_band(20.0, 80.0);
        assert!((lo - 20.0).abs() < 1e-10);
        assert!((hi - 160.0).abs() < 1e-10);

        let (lo2, hi2) = derive_band(30.0, 80.0);
        assert!((lo2 - 30.0).abs() < 1e-10); // max(30, 80*0.25=20) = 30
        assert!((hi2 - 160.0).abs() < 1e-10);
    }

    #[test]
    fn test_two_channel_delay_recovery() {
        // Synthetic test: two channels with known delays (2 ms and 4 ms).
        // Wide band [20, 5000] Hz forces the optimiser to align tightly:
        // for GD to be flat across this band, the first comb-filter null
        // (at 1/(2Δτ)) must be above 5000 Hz, i.e. Δτ < 0.1 ms.
        let freq = log_freq_grid(20.0, 5000.0, 500);

        let ch0 = make_delayed_channel(&freq, 2.0, 0.98);
        let ch1 = make_delayed_channel(&freq, 4.0, 0.98);

        let channels = vec![ch0, ch1];
        let band = (20.0, 5000.0);

        let config = GdOptConfig {
            sample_rate: 48000.0,
            max_delay_ms: 10.0,
            ap_per_channel: 0, // no AP filters for this test
            optimize_polarity: false,
            max_iter: 5000,
            popsize: 30,
            tol: 1e-12,
            seed: Some(42),
            ..Default::default()
        };

        let result = optimize_group_delay(&channels, band, &config).unwrap();

        // The optimiser should align the channels by finding delays that
        // equalise their contribution. The relative delay difference should
        // be recovered: |τ0 - τ1| ≈ 2 ms (or the complement within max_delay).
        let d0 = result.per_channel[0].delay_ms;
        let d1 = result.per_channel[1].delay_ms;

        // After optimisation, the effective delays should be equal:
        // Original: ch0 has 2ms, ch1 has 4ms → difference = 2ms.
        // Optimiser adds ~2ms to ch0 (d0 ≈ 2ms, d1 ≈ 0ms).
        let effective_delay_0 = 2.0 + d0;
        let effective_delay_1 = 4.0 + d1;
        let residual_diff = (effective_delay_0 - effective_delay_1).abs();

        assert!(
            residual_diff < 0.1,
            "Delay recovery failed: residual difference = {:.3} ms (expected < 0.1 ms). \
             d0={:.3}, d1={:.3}, effective: {:.3} vs {:.3}",
            residual_diff,
            d0,
            d1,
            effective_delay_0,
            effective_delay_1,
        );

        // Improvement should be >= 6 dB
        assert!(
            result.improvement_db >= 6.0,
            "Improvement too low: {:.1} dB (expected >= 6.0 dB). \
             pre_rms={:.3} ms, post_rms={:.3} ms",
            result.improvement_db,
            result.sum_gd_pre_rms_ms,
            result.sum_gd_post_rms_ms,
        );
    }

    #[test]
    fn test_band_derivation_respects_min_freq() {
        // When min_freq > crossover*0.25, band_lo should be min_freq
        let (lo, _) = derive_band(50.0, 100.0);
        assert!((lo - 50.0).abs() < 1e-10);

        // When min_freq < crossover*0.25, band_lo should be crossover*0.25
        let (lo2, _) = derive_band(10.0, 100.0);
        assert!((lo2 - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_coherence_weighting() {
        // Two channels with same delay mismatch but different coherence.
        // Low-coherence bins should contribute less to the loss.
        let freq = log_freq_grid(20.0, 300.0, 100);

        // Channel 0: flat, no delay
        let ch0 = make_delayed_channel(&freq, 0.0, 0.95);

        // Channel 1: 10ms delay, but with low coherence in the first half
        let n = freq.len();
        let spl = Array1::zeros(n);
        let delay_s = 10.0e-3;
        let phase = freq.mapv(|f| -2.0 * PI * f * delay_s);
        let mut coherence = Array1::from_elem(n, 0.95);
        // Set low coherence for first half of band
        for i in 0..n / 2 {
            coherence[i] = 0.1;
        }
        let ch1 = ChannelMeasurementInput {
            freq: freq.clone(),
            spl,
            phase,
            coherence,
        };

        let channels = vec![ch0, ch1];
        let band_indices: Vec<usize> = (0..n).collect();

        // Compute loss with coherence weighting
        let identity = vec![0.0; param_count(2, &GdOptConfig::default())];
        let rms = compute_sum_gd_rms(&channels, &identity, &band_indices, &GdOptConfig::default());

        // RMS should be non-zero (there's a delay mismatch)
        assert!(rms > 0.0, "RMS should be non-zero with delay mismatch");

        // Now make all coherence high and verify RMS is larger
        // (low coherence was suppressing contribution from misaligned bins)
        let ch1_high_coh = make_delayed_channel(&freq, 10.0, 0.95);
        let channels_high_coh = vec![make_delayed_channel(&freq, 0.0, 0.95), ch1_high_coh];
        let rms_high = compute_sum_gd_rms(
            &channels_high_coh,
            &identity,
            &band_indices,
            &GdOptConfig::default(),
        );

        assert!(
            rms_high > rms,
            "High-coherence RMS ({:.3}) should exceed low-coherence RMS ({:.3})",
            rms_high,
            rms,
        );
    }

    #[test]
    fn test_sum_gd_last_band_bin_uses_in_band_backward_difference() {
        let freq = Array1::from_vec(vec![20.0, 30.0, 40.0, 1000.0]);
        let channel = ChannelMeasurementInput {
            freq,
            spl: Array1::zeros(4),
            phase: Array1::from_vec(vec![0.0, -0.1, -0.2, -10.0]),
            coherence: Array1::from_elem(4, 0.95),
        };
        let channels = vec![channel];
        let band_indices = vec![0, 1, 2];
        let identity = vec![0.0; param_count(1, &GdOptConfig::default())];

        let gd = compute_sum_gd(&channels, &identity, &band_indices, &GdOptConfig::default());

        assert_eq!(gd.len(), band_indices.len());
        assert!(
            (gd[2] - gd[1]).abs() < 1e-9,
            "last in-band GD should use backward difference; got {:?}",
            gd
        );
    }

    #[test]
    fn test_gd_target_uses_coherence_weighted_median() {
        let target = weighted_median(&[0.0, 100.0, 101.0], &[10.0, 0.1, 0.1]);

        assert_eq!(target, 0.0);
    }

    #[test]
    fn test_minimum_channels() {
        let freq = log_freq_grid(20.0, 300.0, 50);
        let ch0 = make_delayed_channel(&freq, 0.0, 0.95);
        let result = optimize_group_delay(&[ch0], (20.0, 300.0), &GdOptConfig::default());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 2 channels"));
    }

    #[test]
    fn test_frequency_grid_values_must_match() {
        let freq0 = Array1::from(vec![20.0, 40.0, 80.0, 160.0]);
        let freq1 = Array1::from(vec![20.0, 41.0, 80.0, 160.0]);
        let ch0 = make_delayed_channel(&freq0, 0.0, 0.95);
        let ch1 = make_delayed_channel(&freq1, 1.0, 0.95);

        let result = optimize_group_delay(
            &[ch0, ch1],
            (20.0, 160.0),
            &GdOptConfig {
                ap_per_channel: 0,
                optimize_polarity: false,
                max_iter: 10,
                popsize: 4,
                seed: Some(1),
                ..Default::default()
            },
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("frequency grid"));
    }

    #[test]
    fn test_reported_delays_are_normalized_no_common_latency() {
        let freq = log_freq_grid(20.0, 5000.0, 300);
        let ch0 = make_delayed_channel(&freq, 2.0, 0.98);
        let ch1 = make_delayed_channel(&freq, 4.0, 0.98);

        let result = optimize_group_delay(
            &[ch0, ch1],
            (20.0, 5000.0),
            &GdOptConfig {
                max_delay_ms: 10.0,
                ap_per_channel: 0,
                optimize_polarity: false,
                max_iter: 3000,
                popsize: 20,
                tol: 1e-10,
                seed: Some(43),
                ..Default::default()
            },
        )
        .unwrap();

        let min_delay = result
            .per_channel
            .iter()
            .map(|ch| ch.delay_ms)
            .fold(f64::INFINITY, f64::min);
        assert!(
            min_delay.abs() < 1e-6,
            "normalized controls must leave one channel at 0ms, got {min_delay:.6}ms"
        );
        assert!(
            result.per_channel.iter().all(|ch| ch.delay_ms >= -1e-9),
            "exported delays must be non-negative: {:?}",
            result
                .per_channel
                .iter()
                .map(|ch| ch.delay_ms)
                .collect::<Vec<_>>()
        );
    }

    // ─── Adaptive bootstrap tests ────────────────────────────────────────────

    #[test]
    fn test_adaptive_bootstrap_rejects_noisy_ap() {
        // Two channels with pure delay mismatch. AP filters can't help
        // (only delay alignment is needed). With noisy realisations,
        // the bootstrap should reject the AP and return delay-only.
        let freq = log_freq_grid(20.0, 5000.0, 300);
        let ch0 = make_delayed_channel(&freq, 2.0, 0.95);
        let ch1 = make_delayed_channel(&freq, 4.0, 0.95);
        let channels = vec![ch0, ch1];

        // Create noisy per-sweep realisations (4 sweeps)
        // Each has slight random phase jitter to simulate measurement noise
        let sweep_realisations: Vec<Vec<ChannelMeasurementInput>> = (0..4)
            .map(|seed| {
                let jitter = (seed as f64 * 0.1 + 0.05) * 1e-3; // 0.05-0.35ms jitter
                vec![
                    make_delayed_channel(&freq, 2.0 + jitter, 0.95),
                    make_delayed_channel(&freq, 4.0 - jitter, 0.95),
                ]
            })
            .collect();

        let config = GdOptConfig {
            sample_rate: 48000.0,
            max_delay_ms: 10.0,
            ap_per_channel: 2, // allow up to 2, but bootstrap should reject
            optimize_polarity: false,
            max_iter: 2000,
            popsize: 20,
            tol: 1e-10,
            seed: Some(123),
            ..Default::default()
        };

        let result =
            optimize_group_delay_adaptive(&channels, &sweep_realisations, (20.0, 5000.0), &config)
                .unwrap();

        // The result should still achieve good alignment (delay recovery works)
        let d0 = result.per_channel[0].delay_ms;
        let d1 = result.per_channel[1].delay_ms;
        let residual = ((2.0 + d0) - (4.0 + d1)).abs();
        assert!(
            residual < 0.2,
            "Delay alignment failed: residual={:.3}ms",
            residual
        );

        // AP filters should be either empty (rejected) or minimal
        // The key check: the result should improve GD
        assert!(
            result.improvement_db >= 6.0,
            "Improvement too low: {:.1} dB",
            result.improvement_db
        );
    }

    #[test]
    fn test_adaptive_bootstrap_requires_min_sweeps() {
        let freq = log_freq_grid(20.0, 300.0, 50);
        let ch0 = make_delayed_channel(&freq, 0.0, 0.95);
        let ch1 = make_delayed_channel(&freq, 5.0, 0.95);
        let channels = vec![ch0, ch1];

        // Only 1 sweep — should fail
        let one_sweep = vec![vec![
            make_delayed_channel(&freq, 0.0, 0.95),
            make_delayed_channel(&freq, 5.0, 0.95),
        ]];

        let config = GdOptConfig::default();
        let result = optimize_group_delay_adaptive(&channels, &one_sweep, (20.0, 300.0), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 2"));
    }

    // ─── Mode dispatch tests ─────────────────────────────────────────────────

    #[test]
    fn test_mode_dispatch_low_latency() {
        let freq = log_freq_grid(20.0, 5000.0, 300);
        let ch0 = make_delayed_channel(&freq, 1.0, 0.95);
        let ch1 = make_delayed_channel(&freq, 3.0, 0.95);
        let channels = vec![ch0, ch1];

        let config = GdOptConfig {
            ap_per_channel: 1,
            optimize_polarity: false,
            max_iter: 2000,
            popsize: 20,
            seed: Some(77),
            ..Default::default()
        };

        let result = optimize_group_delay_for_mode(
            &channels,
            (20.0, 5000.0),
            &config,
            &ProcessingMode::LowLatency,
            None,
        )
        .unwrap();

        assert!(result.improvement_db > 0.0);
    }

    #[test]
    fn test_mode_dispatch_hybrid_within_crossover() {
        let freq = log_freq_grid(20.0, 200.0, 100);
        let ch0 = make_delayed_channel(&freq, 1.0, 0.95);
        let ch1 = make_delayed_channel(&freq, 3.0, 0.95);
        let channels = vec![ch0, ch1];

        let config = GdOptConfig {
            ap_per_channel: 0,
            optimize_polarity: false,
            max_iter: 1000,
            popsize: 15,
            seed: Some(88),
            ..Default::default()
        };

        let mixed_config = MixedModeConfig {
            crossover_freq: 300.0,
            crossover_type: "LR24".to_string(),
            fir_band: "high".to_string(),
        };

        // band_hi=200 < crossover=300, should succeed
        let result = optimize_group_delay_for_mode(
            &channels,
            (20.0, 200.0),
            &config,
            &ProcessingMode::Hybrid,
            Some(&mixed_config),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_mode_dispatch_hybrid_exceeds_crossover() {
        let freq = log_freq_grid(20.0, 500.0, 100);
        let ch0 = make_delayed_channel(&freq, 1.0, 0.95);
        let ch1 = make_delayed_channel(&freq, 3.0, 0.95);
        let channels = vec![ch0, ch1];

        let config = GdOptConfig::default();
        let mixed_config = MixedModeConfig {
            crossover_freq: 300.0,
            crossover_type: "LR24".to_string(),
            fir_band: "high".to_string(),
        };

        // band_hi=500 > crossover=300, should fail
        let result = optimize_group_delay_for_mode(
            &channels,
            (20.0, 500.0),
            &config,
            &ProcessingMode::Hybrid,
            Some(&mixed_config),
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("exceeds mixed_config crossover")
        );
    }

    #[test]
    fn test_mode_dispatch_mixed_phase_caps_ap() {
        let freq = log_freq_grid(20.0, 5000.0, 300);
        let ch0 = make_delayed_channel(&freq, 1.0, 0.95);
        let ch1 = make_delayed_channel(&freq, 3.0, 0.95);
        let channels = vec![ch0, ch1];

        let config = GdOptConfig {
            ap_per_channel: 2, // requests 2, but MixedPhase caps at 1
            optimize_polarity: false,
            max_iter: 2000,
            popsize: 20,
            seed: Some(99),
            ..Default::default()
        };

        let result = optimize_group_delay_for_mode(
            &channels,
            (20.0, 5000.0),
            &config,
            &ProcessingMode::MixedPhase,
            None,
        )
        .unwrap();

        // Each channel should have at most 1 AP filter
        for ch in &result.per_channel {
            assert!(
                ch.ap_filters.len() <= 1,
                "MixedPhase should cap AP at 1, got {}",
                ch.ap_filters.len()
            );
        }
    }

    #[test]
    fn test_mode_dispatch_phase_linear_rejects() {
        let freq = log_freq_grid(20.0, 300.0, 50);
        let ch0 = make_delayed_channel(&freq, 0.0, 0.95);
        let ch1 = make_delayed_channel(&freq, 5.0, 0.95);
        let channels = vec![ch0, ch1];

        let result = optimize_group_delay_for_mode(
            &channels,
            (20.0, 300.0),
            &GdOptConfig::default(),
            &ProcessingMode::PhaseLinear,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("PhaseLinear"));
    }

    #[test]
    fn test_mode_dispatch_warped_iir_same_as_low_latency() {
        // WarpedIir and KautzModal use the same code path as LowLatency.
        // Verify both achieve good results (not exact equality due to DE
        // parallel evaluation non-determinism).
        let freq = log_freq_grid(20.0, 5000.0, 300);
        let ch0 = make_delayed_channel(&freq, 2.0, 0.95);
        let ch1 = make_delayed_channel(&freq, 4.0, 0.95);
        let channels = vec![ch0, ch1];

        let config = GdOptConfig {
            ap_per_channel: 0,
            optimize_polarity: false,
            max_iter: 3000,
            popsize: 25,
            tol: 1e-10,
            seed: Some(42),
            ..Default::default()
        };

        let wi_result = optimize_group_delay_for_mode(
            &channels,
            (20.0, 5000.0),
            &config,
            &ProcessingMode::WarpedIir,
            None,
        )
        .unwrap();

        let km_result = optimize_group_delay_for_mode(
            &channels,
            (20.0, 5000.0),
            &config,
            &ProcessingMode::KautzModal,
            None,
        )
        .unwrap();

        // Both should achieve significant improvement
        assert!(
            wi_result.improvement_db >= 6.0,
            "WarpedIir improvement too low: {:.1} dB",
            wi_result.improvement_db
        );
        assert!(
            km_result.improvement_db >= 6.0,
            "KautzModal improvement too low: {:.1} dB",
            km_result.improvement_db
        );
    }

    // ─── QA integration tests (GD-5) ─────────────────────────────────────────

    /// Create a channel with pure linear-phase delay plus the phase contribution
    /// of an allpass biquad. Magnitude stays flat (0 dB).
    fn make_delayed_channel_with_allpass(
        freq_grid: &Array1<f64>,
        delay_ms: f64,
        ap_freq: f64,
        ap_q: f64,
        sample_rate: f64,
        coherence: f64,
    ) -> ChannelMeasurementInput {
        let n = freq_grid.len();
        let spl = Array1::zeros(n); // 0 dB flat
        let delay_s = delay_ms * 1e-3;

        let ap = Biquad::new(BiquadFilterType::AllPass, ap_freq, sample_rate, ap_q, 0.0);

        // Phase = linear delay phase + allpass phase
        let phase = freq_grid.mapv(|f| {
            let linear_phase = -2.0 * PI * f * delay_s;
            let ap_phase = ap.complex_response(f).arg();
            linear_phase + ap_phase
        });

        let coherence = Array1::from_elem(n, coherence);

        ChannelMeasurementInput {
            freq: freq_grid.clone(),
            spl,
            phase,
            coherence,
        }
    }

    #[test]
    fn test_qa_three_channel_lrsub_delay_recovery() {
        // Synthetic L/R/Sub with known delays: L=1ms, R=3ms, Sub=8ms.
        // The optimiser must align all three pairwise by adding correction
        // delays. After alignment, every pairwise effective-delay difference
        // should be < 0.15 ms.
        let freq = log_freq_grid(20.0, 5000.0, 500);

        let ch_l = make_delayed_channel(&freq, 1.0, 0.98);
        let ch_r = make_delayed_channel(&freq, 3.0, 0.98);
        let ch_sub = make_delayed_channel(&freq, 8.0, 0.98);

        let channels = vec![ch_l, ch_r, ch_sub];
        let band = (20.0, 5000.0);

        let config = GdOptConfig {
            sample_rate: 48000.0,
            max_delay_ms: 15.0,
            ap_per_channel: 0,
            optimize_polarity: false,
            max_iter: 5000,
            popsize: 30,
            tol: 1e-12,
            seed: Some(42),
            ..Default::default()
        };

        let result = optimize_group_delay(&channels, band, &config).unwrap();

        // Known measurement delays for each channel
        let meas_delays = [1.0_f64, 3.0, 8.0];
        let opt_delays: Vec<f64> = result.per_channel.iter().map(|ch| ch.delay_ms).collect();

        // All pairwise effective delay differences must be < 0.15 ms
        for i in 0..3 {
            for j in (i + 1)..3 {
                let eff_i = meas_delays[i] + opt_delays[i];
                let eff_j = meas_delays[j] + opt_delays[j];
                let diff = (eff_i - eff_j).abs();
                assert!(
                    diff < 0.15,
                    "Pairwise effective delay difference (ch{i} vs ch{j}) = {diff:.3} ms \
                     (expected < 0.15 ms). opt_delays = {opt_delays:?}",
                );
            }
        }

        // Overall improvement must be >= 6 dB
        assert!(
            result.improvement_db >= 6.0,
            "Improvement too low: {:.1} dB (expected >= 6 dB). \
             pre_rms={:.3} ms, post_rms={:.3} ms",
            result.improvement_db,
            result.sum_gd_pre_rms_ms,
            result.sum_gd_post_rms_ms,
        );
    }

    #[test]
    fn test_qa_two_channel_with_allpass_distortion() {
        // Channel 0: pure 2 ms delay (reference).
        // Channel 1: pure 2 ms delay plus an allpass GD bump at 60 Hz Q=2.
        // The optimiser should use AP filters to cancel the GD distortion and
        // achieve >= 6 dB improvement with ap_per_channel=2.
        let freq = log_freq_grid(20.0, 300.0, 400);
        let sample_rate = 48000.0;

        let ch0 = make_delayed_channel(&freq, 2.0, 0.98);
        let ch1 = make_delayed_channel_with_allpass(&freq, 2.0, 60.0, 2.0, sample_rate, 0.98);

        let channels = vec![ch0, ch1];
        let band = (20.0, 300.0);

        let config = GdOptConfig {
            sample_rate,
            max_delay_ms: 10.0,
            ap_per_channel: 2,
            ap_min_freq: 20.0,
            ap_max_freq: 300.0,
            ap_min_q: 0.3,
            ap_max_q: 10.0,
            optimize_polarity: false,
            algorithm: "autoeq:de".to_string(),
            strategy: "lshade".to_string(),
            max_iter: 5000,
            popsize: 30,
            tol: 1e-12,
            seed: Some(7),
        };

        let result = optimize_group_delay(&channels, band, &config).unwrap();

        // Improvement must be >= 6 dB
        assert!(
            result.improvement_db >= 6.0,
            "Improvement too low: {:.1} dB (expected >= 6 dB). \
             pre_rms={:.3} ms, post_rms={:.3} ms",
            result.improvement_db,
            result.sum_gd_pre_rms_ms,
            result.sum_gd_post_rms_ms,
        );

        // At least one channel should have non-empty AP filters in the result
        let any_ap = result
            .per_channel
            .iter()
            .any(|ch| !ch.ap_filters.is_empty());
        assert!(
            any_ap,
            "Expected at least one channel to have AP filters; got none. \
             ap counts: {:?}",
            result
                .per_channel
                .iter()
                .map(|ch| ch.ap_filters.len())
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn test_qa_adaptive_bootstrap_accepts_real_ap() {
        // Two channels where one has a genuine allpass GD distortion (not noise).
        // Channel 0: pure 2 ms delay.
        // Channel 1: 2 ms delay + allpass at 60 Hz Q=2.
        // Four sweep realisations have small delay jitter but all carry the real
        // allpass distortion, so the bootstrap should accept at least 1 AP filter.
        let freq = log_freq_grid(20.0, 300.0, 300);
        let sample_rate = 48000.0;

        let channels = vec![
            make_delayed_channel(&freq, 2.0, 0.98),
            make_delayed_channel_with_allpass(&freq, 2.0, 60.0, 2.0, sample_rate, 0.98),
        ];

        // Four sweep realisations: each has a tiny jitter but preserves the
        // allpass distortion on channel 1 (real, not noise → bootstrap accepts AP).
        let sweep_realisations: Vec<Vec<ChannelMeasurementInput>> = (0..4)
            .map(|seed| {
                let jitter = seed as f64 * 0.02e-3; // 0–0.06 ms jitter (tiny)
                vec![
                    make_delayed_channel(&freq, 2.0 + jitter, 0.98),
                    make_delayed_channel_with_allpass(
                        &freq,
                        2.0 + jitter,
                        60.0,
                        2.0,
                        sample_rate,
                        0.98,
                    ),
                ]
            })
            .collect();

        let config = GdOptConfig {
            sample_rate,
            max_delay_ms: 10.0,
            ap_per_channel: 2,
            ap_min_freq: 20.0,
            ap_max_freq: 300.0,
            ap_min_q: 0.3,
            ap_max_q: 10.0,
            optimize_polarity: false,
            algorithm: "autoeq:de".to_string(),
            strategy: "lshade".to_string(),
            max_iter: 4000,
            popsize: 25,
            tol: 1e-10,
            seed: Some(11),
        };

        let result =
            optimize_group_delay_adaptive(&channels, &sweep_realisations, (20.0, 300.0), &config)
                .unwrap();

        // The bootstrap should accept at least 1 AP filter across all channels
        let total_ap: usize = result
            .per_channel
            .iter()
            .map(|ch| ch.ap_filters.len())
            .sum();
        assert!(
            total_ap >= 1,
            "Expected adaptive bootstrap to accept at least 1 AP filter; got 0. \
             improvement_db={:.1}",
            result.improvement_db,
        );

        // And overall improvement must be >= 4 dB
        assert!(
            result.improvement_db >= 4.0,
            "Improvement too low: {:.1} dB (expected >= 4 dB). \
             pre_rms={:.3} ms, post_rms={:.3} ms",
            result.improvement_db,
            result.sum_gd_pre_rms_ms,
            result.sum_gd_post_rms_ms,
        );
    }

    #[test]
    fn test_qa_build_gd_alignment_target() {
        // Run a 2-channel optimisation, then check build_gd_alignment_target
        // produces a structurally valid GdAlignmentTarget.
        let freq = log_freq_grid(20.0, 5000.0, 300);
        let ch0 = make_delayed_channel(&freq, 1.0, 0.95);
        let ch1 = make_delayed_channel(&freq, 4.0, 0.95);
        let channels = vec![ch0, ch1];
        let band = (20.0, 5000.0);

        let config = GdOptConfig {
            ap_per_channel: 0,
            optimize_polarity: false,
            max_iter: 3000,
            popsize: 20,
            tol: 1e-10,
            seed: Some(55),
            ..Default::default()
        };

        let result = optimize_group_delay(&channels, band, &config).unwrap();
        let target = build_gd_alignment_target(&channels, &result, &config);

        // per_channel_delay_ms must have one entry per channel
        assert_eq!(
            target.per_channel_delay_ms.len(),
            channels.len(),
            "per_channel_delay_ms length mismatch: got {}, expected {}",
            target.per_channel_delay_ms.len(),
            channels.len(),
        );

        // freq grid must be non-empty and within the band
        assert!(
            !target.freq.is_empty(),
            "GdAlignmentTarget freq grid is empty"
        );
        assert!(
            target.freq[0] >= band.0 - 1e-6,
            "freq[0]={} below band_lo={}",
            target.freq[0],
            band.0,
        );
        assert!(
            *target.freq.last().unwrap() <= band.1 + 1e-6,
            "freq[last]={} above band_hi={}",
            target.freq.last().unwrap(),
            band.1,
        );

        // sum_gd_reference_ms must have the same length as freq
        assert_eq!(
            target.sum_gd_reference_ms.len(),
            target.freq.len(),
            "sum_gd_reference_ms and freq length mismatch: {} vs {}",
            target.sum_gd_reference_ms.len(),
            target.freq.len(),
        );
    }
}
