//! Main optimization entry points for room EQ.
//!
//! This module provides the primary public API for room optimization.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use log::{debug, info, warn};
use math_audio_dsp::analysis::compute_average_response;
use math_audio_iir_fir::Biquad;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::config::validate_room_config;
use super::fir;
use super::group_delay;
use super::output;
use super::phase_alignment;
use super::types::{
    ChannelDspChain, DspChainOutput, MeasurementSource, OptimizationMetadata, OptimizerConfig,
    ProcessingMode, RoomConfig, SpeakerConfig, SystemModel, TargetCurveConfig,
};

// Private module imports for extracted functions
use super::speaker_eq::process_single_speaker;

use super::crossover_utils::check_group_consistency;
use super::group_processing::{
    process_cardioid, process_dba, process_multisub_group, process_speaker_group,
};

// ============================================================================
// Type Aliases
// ============================================================================

/// Internal result type for speaker processing to reduce type complexity
/// Returns: (channel_name, chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms, fir_coeffs)
type SpeakerProcessResult = std::result::Result<
    (
        String,
        ChannelDspChain,
        f64,
        f64,
        crate::Curve,
        crate::Curve,
        Vec<crate::iir::Biquad>,
        f64,
        Option<f64>,
        Option<Vec<f64>>,
    ),
    AutoeqError,
>;

/// Result type for mixed mode processing
/// Returns: (chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms, fir_coeffs)
type MixedModeResult = (
    ChannelDspChain,
    f64,
    f64,
    Curve,
    Curve,
    Vec<Biquad>,
    f64,
    Option<f64>,
    Option<Vec<f64>>,
);

/// Detect passband and compute mean SPL for normalization.
///
/// Smooths the measurement at 1 octave to suppress room modes and comb
/// filtering, then uses a log-frequency weighted average (not the raw
/// median) as the reference level. The passband is the full span between
/// the first and last samples whose smoothed SPL is within 10 dB of that
/// reference.
///
/// Using a log-frequency weighted reference is essential: a raw median on
/// linearly sampled curves biases toward the high-frequency region, and a
/// single strong bass mode can inflate the median enough that the true
/// passband falls below the -10 dB threshold, collapsing detection to a
/// tiny bass window. Similarly, searching only for the first crossing from
/// each end misreports the high edge when the curve does not roll off
/// within the measurement range and is tricked into returning a deep
/// mid-band null. First-above / last-above indices are robust to both
/// failure modes.
/// B3 — warn when the optimizer frequency range is outside the measurement data.
///
/// Filters can only be placed within the frequency span the measurement
/// actually covers. If `min_freq` falls below the lowest measured frequency,
/// or `max_freq` above the highest, the optimizer will either find no data
/// there (and skip the region) or interpolate through a roll-off, producing
/// a silently degraded result. Surface the divergence as a `log::warn!` so
/// it appears in the `roomeq` binary's stderr and in any consumer that
/// attaches a log subscriber.
///
/// The tolerance is `10^0.05 ≈ 1.122` on the frequency axis — about
/// 1/6 octave. Small numerical mismatches (19.99 Hz vs 20 Hz) are
/// ignored, but a user who asks the optimizer to work down to 10 Hz
/// with a measurement that only starts at 100 Hz gets the warning
/// (100 → 10 is exactly one decade, well past 1/6 octave).
pub(super) fn warn_if_optimizer_bounds_exceed_data(
    channel_name: &str,
    curve: &Curve,
    opt: &super::types::OptimizerConfig,
) {
    if curve.freq.is_empty() {
        return;
    }
    let data_min = curve.freq[0];
    let data_max = curve.freq[curve.freq.len() - 1];
    // 10^0.05 ≈ 1.122 on the frequency axis — about 1/6 octave. Wide
    // enough to ignore floating-point mismatches at the edges (19.99 vs
    // 20 Hz) yet still flag genuinely out-of-range optimizer settings
    // (e.g. asking for 10 Hz when data starts at 100 Hz).
    let log_margin = 0.05;
    let min_tol = data_min * 10_f64.powf(-log_margin);
    let max_tol = data_max * 10_f64.powf(log_margin);
    if opt.min_freq < min_tol {
        warn!(
            "Channel '{}': optimizer.min_freq={:.1} Hz is below measurement minimum {:.1} Hz. \
             Filters in [{:.1} .. {:.1}] Hz will have no data to correct and will be ignored.",
            channel_name, opt.min_freq, data_min, opt.min_freq, data_min,
        );
    }
    if opt.max_freq > max_tol {
        warn!(
            "Channel '{}': optimizer.max_freq={:.1} Hz is above measurement maximum {:.1} Hz. \
             Filters in [{:.1} .. {:.1}] Hz will have no data to correct and will be ignored.",
            channel_name, opt.max_freq, data_max, data_max, opt.max_freq,
        );
    }
}

pub(super) fn detect_passband_and_mean(curve: &Curve) -> (Option<(f64, f64)>, f64) {
    let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();

    if spl_f32.len() < 2 {
        return (None, 0.0);
    }

    // 1-octave smoothing suppresses narrow room modes while preserving
    // the broadband shape of the speaker response.
    let smoothed = crate::read::smooth_one_over_n_octave(curve, 1);
    let smoothed_spl: Vec<f32> = smoothed.spl.iter().map(|&s| s as f32).collect();

    // Log-frequency weighted average of the smoothed curve: robust to
    // narrow peaks and to non-uniform sample spacing.
    let ref_level = compute_average_response(&freqs_f32, &smoothed_spl, None);
    if !ref_level.is_finite() || ref_level < -100.0 {
        return (None, 0.0);
    }

    let threshold = ref_level - 10.0;

    // Outermost indices at or above the threshold. Interior dips (room
    // nulls) are intentionally ignored -- they do not change the speaker's
    // reproducible range, only its in-room smoothness.
    let first_above = smoothed_spl.iter().position(|&v| v >= threshold);
    let last_above = smoothed_spl.iter().rposition(|&v| v >= threshold);

    let (start_idx, end_idx) = match (first_above, last_above) {
        (Some(s), Some(e)) if e > s => (s, e),
        _ => return (None, 0.0),
    };

    let f_low = if start_idx > 0 {
        interp_threshold_crossing(
            freqs_f32[start_idx - 1],
            freqs_f32[start_idx],
            smoothed_spl[start_idx - 1],
            smoothed_spl[start_idx],
            threshold,
        )
    } else {
        freqs_f32[start_idx]
    };
    let f_high = if end_idx + 1 < smoothed_spl.len() {
        interp_threshold_crossing(
            freqs_f32[end_idx],
            freqs_f32[end_idx + 1],
            smoothed_spl[end_idx],
            smoothed_spl[end_idx + 1],
            threshold,
        )
    } else {
        freqs_f32[end_idx]
    };

    // Compute mean on the original (unsmoothed) curve within the detected passband
    let norm_range_f32 = Some((f_low, f_high));
    let mean = compute_average_response(&freqs_f32, &spl_f32, norm_range_f32) as f64;

    (Some((f_low as f64, f_high as f64)), mean)
}

/// Linear interpolation of the frequency at which a segment crosses the
/// given magnitude threshold. The result is clamped to the segment so a
/// non-monotonic pair cannot return a frequency outside `[f0, f1]`.
fn interp_threshold_crossing(f0: f32, f1: f32, m0: f32, m1: f32, threshold: f32) -> f32 {
    let denom = m1 - m0;
    if denom.abs() < 1e-9 {
        return f0;
    }
    let t = ((threshold - m0) / denom).clamp(0.0, 1.0);
    f0 + t * (f1 - f0)
}

/// Post-generate FIR coefficients for a channel that only has IIR results.
///
/// For Hybrid mode, uses the IIR-corrected curve as FIR input;
/// for PhaseLinear (FIR-only) mode, uses the raw measurement.
fn post_generate_fir(
    name: &str,
    initial_curve: &Curve,
    final_curve: &Curve,
    config: &super::types::OptimizerConfig,
    target_curve: Option<&super::types::TargetCurveConfig>,
    sample_rate: f64,
    output_dir: Option<&Path>,
) -> Option<Vec<f64>> {
    let fir_input = match config.processing_mode {
        ProcessingMode::Hybrid => final_curve,
        _ => initial_curve,
    };
    match fir::generate_fir_correction(fir_input, config, target_curve, sample_rate) {
        Ok(coeffs) => {
            if let Some(out_dir) = output_dir {
                let filename = format!("{}_fir.wav", name);
                let wav_path = out_dir.join(&filename);
                if let Err(e) = crate::fir::save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path)
                {
                    warn!("Failed to save FIR WAV for {}: {}", name, e);
                } else {
                    info!("  Saved FIR filter to {}", wav_path.display());
                }
            }
            Some(coeffs)
        }
        Err(e) => {
            warn!("FIR generation failed for {}: {}", name, e);
            None
        }
    }
}

/// Post-generate a short excess-phase FIR for MixedPhase mode.
///
/// The workflow path only runs IIR optimisation.  For MixedPhase we still need
/// the short FIR that corrects residual excess phase.  This mirrors the logic
/// in `optimize_speaker_eq` MixedPhase branch but runs after the workflow.
fn post_generate_mixed_phase_fir(
    name: &str,
    initial_curve: &Curve,
    config: &super::types::OptimizerConfig,
    sample_rate: f64,
    output_dir: Option<&Path>,
) -> Option<Vec<f64>> {
    let phase = initial_curve.phase.as_ref()?;
    if phase.is_empty() {
        return None;
    }

    let mp_config = match &config.mixed_phase {
        Some(sc) => super::mixed_phase::MixedPhaseConfig {
            max_fir_length_ms: sc.max_fir_length_ms,
            pre_ringing_threshold_db: sc.pre_ringing_threshold_db,
            min_spatial_depth: sc.min_spatial_depth,
            phase_smoothing_octaves: sc.phase_smoothing_octaves,
        },
        None => super::mixed_phase::MixedPhaseConfig::default(),
    };

    match super::mixed_phase::decompose_phase(initial_curve, &mp_config) {
        Ok((_min_phase, _excess, delay_ms, residual)) => {
            info!(
                "  Mixed-phase (post-workflow) '{}': delay={:.2} ms",
                name, delay_ms
            );
            let coeffs = super::mixed_phase::generate_excess_phase_fir(
                &initial_curve.freq,
                &residual,
                &mp_config,
                sample_rate,
            );

            if let Some(out_dir) = output_dir {
                let filename = format!("{}_excess_phase_fir.wav", name);
                let wav_path = out_dir.join(&filename);
                if let Err(e) = crate::fir::save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path)
                {
                    warn!("Failed to save excess phase FIR for {}: {}", name, e);
                } else {
                    info!("  Saved excess phase FIR to {}", wav_path.display());
                }
            }

            Some(coeffs)
        }
        Err(e) => {
            warn!(
                "  Mixed-phase decomposition failed for '{}': {}. Using IIR only.",
                name, e
            );
            None
        }
    }
}

/// Apply standalone phase correction to a channel (rePhase-style).
///
/// Generates a phase-only FIR from the measurement's excess phase and appends it
/// to the channel's DSP chain. If the channel already has a magnitude FIR, the
/// two are convolved together so `fir_coeffs` remains a single filter for IR
/// computation.
fn apply_phase_correction(
    name: &str,
    ch: &mut ChannelOptimizationResult,
    chain: &mut super::types::ChannelDspChain,
    config: &super::types::MixedPhaseSerdeConfig,
    sample_rate: f64,
    output_dir: Option<&Path>,
) {
    let phase = match ch.initial_curve.phase.as_ref() {
        Some(p) if !p.is_empty() => p,
        _ => return,
    };
    let _ = phase; // used via initial_curve below

    let mp_config = super::mixed_phase::MixedPhaseConfig {
        max_fir_length_ms: config.max_fir_length_ms,
        pre_ringing_threshold_db: config.pre_ringing_threshold_db,
        min_spatial_depth: config.min_spatial_depth,
        phase_smoothing_octaves: config.phase_smoothing_octaves,
    };

    let phase_fir = match super::mixed_phase::decompose_phase(&ch.initial_curve, &mp_config) {
        Ok((_min, _excess, delay_ms, residual)) => {
            info!(
                "  Phase correction '{}': delay={:.2} ms, generating phase-only FIR",
                name, delay_ms
            );
            super::mixed_phase::generate_excess_phase_fir(
                &ch.initial_curve.freq,
                &residual,
                &mp_config,
                sample_rate,
            )
        }
        Err(e) => {
            warn!("  Phase correction failed for '{}': {}", name, e);
            return;
        }
    };

    // Save phase FIR WAV and add convolution plugin
    let filename = format!("{}_phase_correction.wav", name);
    if let Some(out_dir) = output_dir {
        let wav_path = out_dir.join(&filename);
        if let Err(e) = crate::fir::save_fir_to_wav(&phase_fir, sample_rate as u32, &wav_path) {
            warn!("Failed to save phase correction FIR for {}: {}", name, e);
        } else {
            info!("  Saved phase correction FIR to {}", wav_path.display());
        }
    }
    chain
        .plugins
        .push(super::output::create_convolution_plugin(&filename));

    // Combine with existing FIR for IR computation (convolve the two)
    if let Some(ref existing) = ch.fir_coeffs {
        ch.fir_coeffs = Some(convolve(existing, &phase_fir));
    } else {
        ch.fir_coeffs = Some(phase_fir);
    }
}

/// Linear convolution of two FIR filters.
fn convolve(a: &[f64], b: &[f64]) -> Vec<f64> {
    let len = a.len() + b.len() - 1;
    let mut out = vec![0.0; len];
    for (i, &av) in a.iter().enumerate() {
        for (j, &bv) in b.iter().enumerate() {
            out[i + j] += av * bv;
        }
    }
    out
}

/// Threshold in dB above which to warn about channel level differences
const LEVEL_DIFFERENCE_WARNING_THRESHOLD: f64 = 6.0;

/// Threshold in ms above which to warn about arrival time differences
const ARRIVAL_TIME_WARNING_THRESHOLD_MS: f64 = 50.0;

// ============================================================================
// Sub-Main Pairing Logic
// ============================================================================

/// Find subwoofer-to-main-speaker pairings using system config or heuristic fallback.
///
/// Returns `(sub_name, main_name)` pairs where names are keys into the curves/chains maps.
/// Used by both phase alignment and GD-Opt v2.
fn find_sub_main_pairings(
    config: &RoomConfig,
    curves: &HashMap<String, crate::Curve>,
) -> Vec<(String, String)> {
    let mut pairings = Vec::new();

    if let Some(sys) = &config.system {
        // Use explicit system configuration
        if let Some(subs) = &sys.subwoofers {
            // Invert speakers map to find roles from measurement keys
            // measurement_key -> role
            let meas_to_role: HashMap<&String, &String> =
                sys.speakers.iter().map(|(r, m)| (m, r)).collect();

            for (sub_meas_key, main_role) in &subs.mapping {
                if let Some(sub_role) = meas_to_role.get(sub_meas_key) {
                    pairings.push((sub_role.to_string(), main_role.clone()));
                } else {
                    warn!(
                        "Subwoofer measurement '{}' not mapped to any output channel",
                        sub_meas_key
                    );
                }
            }
        }
    } else {
        // Legacy heuristic: find "lfe" or "sub*" channel, pair with all non-sub channels
        let sub_channel = curves
            .keys()
            .find(|name| *name == "lfe" || name.starts_with("sub"))
            .cloned();
        if let Some(sub_name) = sub_channel {
            let main_channels: Vec<String> = curves
                .keys()
                .filter(|name| *name != &sub_name && !name.starts_with("sub"))
                .cloned()
                .collect();
            for main in main_channels {
                pairings.push((sub_name.clone(), main));
            }
        }
    }

    pairings
}

// ============================================================================
// Progress and Callback Types
// ============================================================================

/// Action to take after progress callback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackAction {
    /// Continue optimization
    Continue,
    /// Stop optimization early
    Stop,
}

/// Progress update for room optimization
#[derive(Debug, Clone)]
pub struct RoomOptimizationProgress {
    /// Current speaker being optimized
    pub current_speaker: String,
    /// Speaker index (0-based)
    pub speaker_index: usize,
    /// Total number of speakers
    pub total_speakers: usize,
    /// Current iteration within this speaker
    pub iteration: usize,
    /// Maximum iterations for this speaker
    pub max_iterations: usize,
    /// Current loss value
    pub loss: f64,
    /// Overall progress (0.0 - 1.0)
    pub overall_progress: f64,
    /// Optional log message for display
    pub message: Option<String>,
    /// EPA preference score (higher = better), computed every N iterations
    pub epa_preference: Option<f64>,
}

/// Callback type for room optimization progress
pub type RoomOptimizationCallback =
    Box<dyn FnMut(&RoomOptimizationProgress) -> CallbackAction + Send>;

/// Callback type for single speaker optimization progress
pub type SpeakerOptimizationCallback =
    Box<dyn FnMut(&RoomOptimizationProgress) -> CallbackAction + Send>;

// ============================================================================
// Result Types
// ============================================================================

/// Result for a single channel optimization
#[derive(Debug, Clone)]
pub struct ChannelOptimizationResult {
    /// Channel name
    pub name: String,
    /// Pre-optimization score
    pub pre_score: f64,
    /// Post-optimization score
    pub post_score: f64,
    /// Initial frequency response curve
    pub initial_curve: Curve,
    /// Final corrected frequency response curve
    pub final_curve: Curve,
    /// Biquad filters (for IIR mode)
    pub biquads: Vec<Biquad>,
    /// FIR coefficients (for FIR/mixed mode)
    pub fir_coeffs: Option<Vec<f64>>,
}

/// Result of room optimization
#[derive(Debug, Clone)]
pub struct RoomOptimizationResult {
    /// Per-channel DSP chains
    pub channels: HashMap<String, ChannelDspChain>,
    /// Per-channel optimization results (initial/final curves, scores)
    pub channel_results: HashMap<String, ChannelOptimizationResult>,
    /// Combined pre-optimization score (average)
    pub combined_pre_score: f64,
    /// Combined post-optimization score (average)
    pub combined_post_score: f64,
    /// Optimization metadata
    pub metadata: OptimizationMetadata,
}

impl RoomOptimizationResult {
    /// Convert to DspChainOutput for serialization
    pub fn to_dsp_chain_output(&self) -> DspChainOutput {
        output::create_dsp_chain_output(self.channels.clone(), Some(self.metadata.clone()))
    }
}

/// Result for single speaker optimization
#[derive(Debug, Clone)]
pub struct SpeakerOptimizationResult {
    /// DSP chain for this speaker
    pub chain: ChannelDspChain,
    /// Pre-optimization score
    pub pre_score: f64,
    /// Post-optimization score
    pub post_score: f64,
    /// Initial curve
    pub initial_curve: Curve,
    /// Final curve
    pub final_curve: Curve,
    /// Biquad filters
    pub biquads: Vec<Biquad>,
    /// FIR coefficients (if applicable)
    pub fir_coeffs: Option<Vec<f64>>,
}

// ============================================================================
// Main Entry Points
// ============================================================================

/// Optimize a complete room configuration
///
/// Processes all speakers in parallel and returns DSP chains for each channel.
///
/// # Arguments
/// * `config` - Complete room configuration
/// * `sample_rate` - Sample rate for filter design (e.g., 48000.0)
/// * `callback` - Optional progress callback
/// * `output_dir` - Optional directory for writing intermediate artifacts
///
/// # Returns
/// * `RoomOptimizationResult` containing DSP chains and optimization results
pub fn optimize_room(
    config: &RoomConfig,
    sample_rate: f64,
    callback: Option<RoomOptimizationCallback>,
    output_dir: Option<&Path>,
) -> Result<RoomOptimizationResult> {
    optimize_room_impl(config, sample_rate, callback, output_dir, None)
}

/// Same as [`optimize_room`] but accepts per-channel probe-based arrival times.
///
/// When the delay-detection UI step has measured arrival times with a tone
/// burst probe, pass them here so `process_single_speaker` uses the measured
/// values instead of falling back to WAV-onset detection. Channels absent from
/// `probe_arrival_ms` still use the WAV-onset fallback.
pub fn optimize_room_with_probe_arrivals(
    config: &RoomConfig,
    sample_rate: f64,
    callback: Option<RoomOptimizationCallback>,
    output_dir: Option<&Path>,
    probe_arrival_ms: &HashMap<String, f64>,
) -> Result<RoomOptimizationResult> {
    optimize_room_impl(
        config,
        sample_rate,
        callback,
        output_dir,
        Some(probe_arrival_ms),
    )
}

/// I6 — sanity invariants on the final `RoomOptimizationResult`.
///
/// Catches silent corruption bugs that would otherwise produce garbage DSP
/// chains (misaligned indexing, NaN fallout from the optimiser). A full
/// chain resynthesis would need to simulate every plugin type (gain /
/// delay / biquad / FIR) and reproduce each workflow's intermediate curve
/// derivation — that invariant is deferred. A magnitude-delta
/// envelope was considered but had to be removed: in 2.1 / home-cinema
/// workflows the Sub channel's `final_curve` legitimately reaches
/// −300 dB where the LP crossover attenuates far-above-passband content,
/// which is not a bug.
///
/// The function runs in both debug AND release:
///   - In debug builds, panics on violation so tests surface the exact
///     failed invariant.
///   - In release builds, returns an `Err` instead so release QA / fuzz
///     runs don't silently ship a corrupted result when the optimiser
///     diverges — they get a clean error that downstream callers can
///     handle.
///
/// Invariants:
///   1. Every channel's `freq` and `spl` lengths match (on both the
///      initial and final curves).
///   2. No NaN or infinite SPL values in the final curve.
fn sanity_check_result(result: &RoomOptimizationResult) -> Result<()> {
    for (name, ch) in &result.channel_results {
        if ch.initial_curve.freq.len() != ch.initial_curve.spl.len() {
            let msg = format!(
                "channel '{}': initial_curve freq/spl length mismatch ({} vs {})",
                name,
                ch.initial_curve.freq.len(),
                ch.initial_curve.spl.len()
            );
            debug_assert!(false, "{}", msg);
            return Err(AutoeqError::OptimizationFailed { message: msg });
        }
        if ch.final_curve.freq.len() != ch.final_curve.spl.len() {
            let msg = format!(
                "channel '{}': final_curve freq/spl length mismatch ({} vs {})",
                name,
                ch.final_curve.freq.len(),
                ch.final_curve.spl.len()
            );
            debug_assert!(false, "{}", msg);
            return Err(AutoeqError::OptimizationFailed { message: msg });
        }
        if let Some((i, v)) = ch
            .final_curve
            .spl
            .iter()
            .enumerate()
            .find(|(_, v)| !v.is_finite())
        {
            let msg = format!(
                "channel '{}': final_curve.spl[{}]={} is non-finite (optimiser diverged)",
                name, i, v
            );
            debug_assert!(false, "{}", msg);
            return Err(AutoeqError::OptimizationFailed { message: msg });
        }
    }
    Ok(())
}

fn optimize_room_impl(
    config: &RoomConfig,
    sample_rate: f64,
    mut callback: Option<RoomOptimizationCallback>,
    output_dir: Option<&Path>,
    probe_arrival_overrides: Option<&HashMap<String, f64>>,
) -> Result<RoomOptimizationResult> {
    // Ensure legacy target_tilt/broadband fields are migrated.
    // load_config() does this already, but callers building RoomConfig in memory
    // (tests, GPUI) may not have called it.
    let mut config = config.clone();
    config.optimizer.migrate_target_config();

    // Pre-fetch CEA2034 data for all speakers when speaker pre-correction is enabled
    if config
        .optimizer
        .cea2034_correction
        .as_ref()
        .is_some_and(|c| c.enabled)
    {
        let cache = super::cea2034_correction::pre_fetch_all_cea2034(&config);
        if !cache.is_empty() {
            info!(
                "  CEA2034 cache: loaded data for {} speaker(s)",
                cache.len()
            );
            config.cea2034_cache = Some(cache);
        }
    }

    let config = &config;

    // Validate configuration
    let validation = validate_room_config(config);
    validation.print_results();
    if !validation.is_valid {
        return Err(AutoeqError::OptimizationFailed {
            message: format!(
                "Configuration validation failed with {} errors",
                validation.errors.len()
            ),
        });
    }

    /// Helper to invoke the callback if present, returning true if Stop was requested.
    fn send_progress(
        cb: &mut Option<RoomOptimizationCallback>,
        progress: &RoomOptimizationProgress,
    ) -> bool {
        if let Some(f) = cb {
            f(progress) == CallbackAction::Stop
        } else {
            false
        }
    }

    // Dispatch to specific workflows based on topology
    if let Some(sys) = &config.system {
        // If any channel uses SpeakerConfig::Group, fall through to the generic path
        // which handles Groups via process_speaker_group.
        let has_group = sys
            .speakers
            .values()
            .any(|key| matches!(config.speakers.get(key), Some(SpeakerConfig::Group(_))));

        // Phase 3: workflows route per-channel EQ through
        // `process_single_speaker` (via `run_channel_via_generic_path`)
        // so excursion / target_response / broadband / CEA2034 apply
        // uniformly across Stereo 2.0, Stereo 2.1, HomeCinema-no-sub, and
        // HomeCinema-with-sub. The previous `use_generic_for_stereo`
        // fallback and Phase 3a's "features not honoured" warning are
        // both retired — the workflows are now feature-complete.

        if !has_group {
            let workflow_name = match sys.model {
                SystemModel::Stereo => {
                    if sys.subwoofers.is_some() {
                        "Stereo 2.1"
                    } else {
                        "Stereo 2.0"
                    }
                }
                SystemModel::HomeCinema => "Home Cinema",
                SystemModel::Custom => "Custom",
            };

            // Send pre-workflow progress message
            if sys.model != SystemModel::Custom {
                send_progress(
                    &mut callback,
                    &RoomOptimizationProgress {
                        current_speaker: String::new(),
                        speaker_index: 0,
                        total_speakers: sys.speakers.len(),
                        iteration: 0,
                        max_iterations: 0,
                        loss: 0.0,
                        overall_progress: 0.0,
                        message: Some(format!(
                            "Starting {} workflow ({} channels)",
                            workflow_name,
                            sys.speakers.len()
                        )),
                        epa_preference: None,
                    },
                );
            }

            let workflow_result = match sys.model {
                SystemModel::Stereo => {
                    if sys.subwoofers.is_some() {
                        Some(super::workflows::optimize_stereo_2_1(
                            config,
                            sys,
                            sample_rate,
                            output_dir.unwrap_or(Path::new(".")),
                        ))
                    } else {
                        Some(super::workflows::optimize_stereo_2_0(
                            config,
                            sys,
                            sample_rate,
                            output_dir.unwrap_or(Path::new(".")),
                        ))
                    }
                }
                SystemModel::HomeCinema => Some(super::workflows::optimize_home_cinema(
                    config,
                    sys,
                    sample_rate,
                    output_dir.unwrap_or(Path::new(".")),
                )),
                SystemModel::Custom => None, // Fall through to generic path
            };

            if let Some(result) = workflow_result {
                let mut result = result?;

                // Send post-workflow summary
                let summary: Vec<String> = result
                    .channel_results
                    .iter()
                    .map(|(name, ch)| {
                        format!("  {}: {:.4} -> {:.4}", name, ch.pre_score, ch.post_score)
                    })
                    .collect();
                send_progress(
                    &mut callback,
                    &RoomOptimizationProgress {
                        current_speaker: String::new(),
                        speaker_index: result.channel_results.len(),
                        total_speakers: result.channel_results.len(),
                        iteration: 0,
                        max_iterations: 0,
                        loss: result.combined_post_score,
                        overall_progress: 1.0,
                        message: Some(format!(
                            "{} workflow complete:\n{}",
                            workflow_name,
                            summary.join("\n")
                        )),
                        epa_preference: None,
                    },
                );
                // Workflows only do IIR. If FIR/Hybrid mode is requested, post-generate
                // full FIR coefficients for each channel.
                if matches!(
                    config.optimizer.processing_mode,
                    ProcessingMode::PhaseLinear | ProcessingMode::Hybrid
                ) {
                    send_progress(
                        &mut callback,
                        &RoomOptimizationProgress {
                            current_speaker: "FIR generation".to_string(),
                            speaker_index: 0,
                            total_speakers: result.channel_results.len(),
                            iteration: 0,
                            max_iterations: 0,
                            loss: 0.0,
                            overall_progress: 0.95,
                            message: Some("Generating FIR coefficients...".to_string()),
                            epa_preference: None,
                        },
                    );
                    let out_dir = output_dir.unwrap_or(Path::new("."));
                    for (name, ch) in result.channel_results.iter_mut() {
                        if ch.fir_coeffs.is_some() {
                            continue;
                        }
                        ch.fir_coeffs = post_generate_fir(
                            name,
                            &ch.initial_curve,
                            &ch.final_curve,
                            &config.optimizer,
                            config.target_curve.as_ref(),
                            sample_rate,
                            Some(out_dir),
                        );
                    }
                }
                // MixedPhase: post-generate short excess-phase FIR for each channel
                // and add convolution plugin to the DSP chain.
                if config.optimizer.processing_mode == ProcessingMode::MixedPhase {
                    send_progress(
                        &mut callback,
                        &RoomOptimizationProgress {
                            current_speaker: "Mixed-phase FIR".to_string(),
                            speaker_index: 0,
                            total_speakers: result.channel_results.len(),
                            iteration: 0,
                            max_iterations: 0,
                            loss: 0.0,
                            overall_progress: 0.95,
                            message: Some("Generating mixed-phase FIR...".to_string()),
                            epa_preference: None,
                        },
                    );
                    let out_dir = output_dir.unwrap_or(Path::new("."));
                    for (name, ch) in result.channel_results.iter_mut() {
                        if ch.fir_coeffs.is_some() {
                            continue;
                        }
                        ch.fir_coeffs = post_generate_mixed_phase_fir(
                            name,
                            &ch.initial_curve,
                            &config.optimizer,
                            sample_rate,
                            Some(out_dir),
                        );
                        if ch.fir_coeffs.is_some()
                            && let Some(chain) = result.channels.get_mut(name)
                        {
                            let filename = format!("{}_excess_phase_fir.wav", name);
                            chain
                                .plugins
                                .push(super::output::create_convolution_plugin(&filename));
                        }
                    }
                }
                // Standalone phase correction (rePhase-style)
                if config.optimizer.phase_correction.is_some() {
                    send_progress(
                        &mut callback,
                        &RoomOptimizationProgress {
                            current_speaker: "Phase correction".to_string(),
                            speaker_index: 0,
                            total_speakers: result.channel_results.len(),
                            iteration: 0,
                            max_iterations: 0,
                            loss: 0.0,
                            overall_progress: 0.96,
                            message: Some("Phase correction...".to_string()),
                            epa_preference: None,
                        },
                    );
                }
                if let Some(ref pc_config) = config.optimizer.phase_correction {
                    let out_dir = output_dir.unwrap_or(Path::new("."));
                    let names: Vec<String> = result.channel_results.keys().cloned().collect();
                    for name in &names {
                        if let Some(ch) = result.channel_results.get_mut(name)
                            && let Some(chain) = result.channels.get_mut(name)
                        {
                            apply_phase_correction(
                                name,
                                ch,
                                chain,
                                pc_config,
                                sample_rate,
                                Some(out_dir),
                            );
                        }
                    }
                }

                // Compute IR waveforms for the workflow result
                send_progress(
                    &mut callback,
                    &RoomOptimizationProgress {
                        current_speaker: "IR computation".to_string(),
                        speaker_index: 0,
                        total_speakers: result.channel_results.len(),
                        iteration: 0,
                        max_iterations: 0,
                        loss: 0.0,
                        overall_progress: 0.97,
                        message: Some("Computing impulse responses...".to_string()),
                        epa_preference: None,
                    },
                );
                for (channel_name, ch_result) in &result.channel_results {
                    let delay_ms = result
                        .channels
                        .get(channel_name)
                        .and_then(|chain| chain.plugins.iter().find(|p| p.plugin_type == "delay"))
                        .and_then(|p| p.parameters.get("delay_ms").and_then(|v| v.as_f64()))
                        .unwrap_or(0.0);
                    if let Some((pre_ir, post_ir)) =
                        super::ir_waveform::compute_channel_ir_waveforms(
                            &ch_result.initial_curve,
                            &ch_result.biquads,
                            ch_result.fir_coeffs.as_deref(),
                            delay_ms,
                            sample_rate,
                        )
                        && let Some(chain) = result.channels.get_mut(channel_name)
                    {
                        chain.pre_ir = Some(pre_ir);
                        chain.post_ir = Some(post_ir);
                    }
                }

                // Compute inter-channel deviation and optionally correct it
                if result.channel_results.len() > 1 {
                    send_progress(
                        &mut callback,
                        &RoomOptimizationProgress {
                            current_speaker: "Channel matching".to_string(),
                            speaker_index: 0,
                            total_speakers: result.channel_results.len(),
                            iteration: 0,
                            max_iterations: 0,
                            loss: 0.0,
                            overall_progress: 0.98,
                            message: Some("Channel matching analysis...".to_string()),
                            epa_preference: None,
                        },
                    );
                    compute_and_correct_icd(&mut result, config, sample_rate);
                }

                sanity_check_result(&result)?;
                return Ok(result);
            }
        }
    }

    // Determine channels to process based on system config or legacy config
    // Returns list of (output_channel_name, speaker_config)
    let channels_to_process: Vec<(String, SpeakerConfig)> = if let Some(sys) = &config.system {
        info!("Using SystemConfig for channel mapping");
        sys.speakers
            .iter()
            .filter_map(|(role, key)| match config.speakers.get(key) {
                Some(cfg) => Some((role.clone(), cfg.clone())),
                None => {
                    warn!(
                        "System config references missing speaker key '{}' for role '{}'",
                        key, role
                    );
                    None
                }
            })
            .collect()
    } else {
        config
            .speakers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };

    let total_speakers = channels_to_process.len();
    info!("Processing {} channels", total_speakers);

    // ========================================================================
    // Pre-pass: compute shared average response level across all channels
    // ========================================================================
    // When multiple channels are present, load each Single-channel measurement
    // and compute its mean SPL. The cross-channel average becomes a shared
    // target reference level so every channel optimizes toward the SAME level,
    // naturally reducing inter-channel deviation at the source.
    let shared_mean_spl: Option<f64> = if total_speakers > 1 {
        let min_freq = config.optimizer.min_freq;
        let max_freq = config.optimizer.max_freq;
        let mut channel_means: Vec<f64> = Vec::new();
        let mut excluded_group_count = 0_usize;

        for (_name, speaker_config) in &channels_to_process {
            if let SpeakerConfig::Single(source) = speaker_config
                && let Ok(curve) = crate::read::load_source(source)
            {
                let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
                let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();
                let mean = compute_average_response(
                    &freqs_f32,
                    &spl_f32,
                    Some((min_freq as f32, max_freq as f32)),
                ) as f64;
                channel_means.push(mean);
            } else if !matches!(speaker_config, SpeakerConfig::Single(_)) {
                excluded_group_count += 1;
            }
        }

        if excluded_group_count > 0 {
            info!(
                "Shared mean pre-pass: {} non-Single speaker(s) excluded (Group/MultiSub/DBA/Cardioid)",
                excluded_group_count
            );
        }

        if channel_means.len() > 1 {
            let avg = channel_means.iter().sum::<f64>() / channel_means.len() as f64;
            info!(
                "Shared target level: {:.1} dB (average of {} channels)",
                avg,
                channel_means.len()
            );
            Some(avg)
        } else {
            None
        }
    } else {
        None
    };

    send_progress(
        &mut callback,
        &RoomOptimizationProgress {
            current_speaker: String::new(),
            speaker_index: 0,
            total_speakers,
            iteration: 0,
            max_iterations: 0,
            loss: 0.0,
            overall_progress: 0.0,
            message: Some(format!(
                "Starting optimization for {} channels",
                total_speakers
            )),
            epa_preference: None,
        },
    );

    // Process each speaker sequentially so we can report progress.
    // Wrap callback in Arc<Mutex> so we can create per-speaker OptimProgressCallbacks.
    //
    // Compute actual DE generation budget for accurate progress display.
    // The DE callback reports generation numbers, not function evals.
    // Formula mirrors optim_de::derive_de_budget.
    let params_per_filter = match config.optimizer.peq_model.as_str() {
        "free" | "ls-pk-hs" => 4,
        _ => 3,
    };
    let n_params = config.optimizer.num_filters * params_per_filter;
    let n_free = n_params.max(1); // all params are free in standard EQ
    let desired_pop = config
        .optimizer
        .population
        .max(1)
        .min(config.optimizer.max_iter.max(1));
    let pop_multiplier = desired_pop.div_ceil(n_free).max(4);
    let population_size = pop_multiplier * n_free;
    // B6 — only apply the 5000-generation floor when the user's budget
    // actually supports it; otherwise honour the requested max_iter so
    // QA / benchmark runs don't silently exceed their evaluation budget.
    // Mirrors `derive_de_budget` in `optim_de.rs`.
    const DE_GENERATIONS_FLOOR: usize = 5000;
    let computed_generations =
        config.optimizer.max_iter.saturating_sub(population_size) / population_size;
    let budget_supports_floor =
        config.optimizer.max_iter >= DE_GENERATIONS_FLOOR.saturating_mul(population_size);
    let max_iterations = if budget_supports_floor {
        computed_generations.max(DE_GENERATIONS_FLOOR)
    } else {
        let capped = computed_generations.max(1);
        if config.optimizer.max_iter > 0 && capped < DE_GENERATIONS_FLOOR {
            warn!(
                "DE budget: max_iter={} with population_size={} is below the {} generation floor × pop. \
                 Running {} generations — expect degraded convergence. Raise max_iter to {} to regain the floor.",
                config.optimizer.max_iter,
                population_size,
                DE_GENERATIONS_FLOOR,
                capped,
                DE_GENERATIONS_FLOOR.saturating_mul(population_size),
            );
        }
        capped
    };
    info!(
        "DE budget: {} params, population_size={}, max_generations={} (from max_iter={}, floor={} when budget allows)",
        n_params,
        population_size,
        max_iterations,
        config.optimizer.max_iter,
        DE_GENERATIONS_FLOOR,
    );
    let callback_shared: Arc<Mutex<Option<RoomOptimizationCallback>>> =
        Arc::new(Mutex::new(callback));

    let mut results: Vec<SpeakerProcessResult> = Vec::with_capacity(total_speakers);
    for (speaker_idx, (channel_name, speaker_config)) in channels_to_process.into_iter().enumerate()
    {
        info!("Processing channel: {}", channel_name);

        {
            let mut guard = callback_shared.lock().unwrap();
            let stop = send_progress(
                &mut guard,
                &RoomOptimizationProgress {
                    current_speaker: channel_name.clone(),
                    speaker_index: speaker_idx,
                    total_speakers,
                    iteration: 0,
                    max_iterations: 0,
                    loss: 0.0,
                    overall_progress: speaker_idx as f64 / total_speakers as f64,
                    message: Some(format!("Processing channel: {}", channel_name)),
                    epa_preference: None,
                },
            );
            if stop {
                break;
            }
        }

        // Create a per-speaker OptimProgressCallback that forwards to the room callback
        let eq_callback: Option<crate::optim::OptimProgressCallback> = {
            let cb = Arc::clone(&callback_shared);
            let name = channel_name.clone();
            let si = speaker_idx;
            let ts = total_speakers;
            let mi = max_iterations;
            Some(Box::new(move |iter: usize, loss: f64, epa: Option<f64>| {
                let base_progress = si as f64 / ts as f64;
                let speaker_progress = if mi > 0 { iter as f64 / mi as f64 } else { 0.0 };
                let overall = (base_progress + speaker_progress / ts as f64).min(1.0);

                if let Ok(mut guard) = cb.lock()
                    && let Some(room_cb) = guard.as_mut()
                {
                    let action = room_cb(&RoomOptimizationProgress {
                        current_speaker: name.clone(),
                        speaker_index: si,
                        total_speakers: ts,
                        iteration: iter,
                        max_iterations: mi,
                        loss,
                        overall_progress: overall,
                        message: None,
                        epa_preference: epa,
                    });
                    return match action {
                        CallbackAction::Continue => crate::de::CallbackAction::Continue,
                        CallbackAction::Stop => crate::de::CallbackAction::Stop,
                    };
                }
                crate::de::CallbackAction::Continue
            }))
        };

        let result = process_speaker_internal(
            &channel_name,
            &speaker_config,
            config,
            sample_rate,
            output_dir,
            eq_callback,
            shared_mean_spl,
            probe_arrival_overrides,
        );

        match result {
            Ok((
                chain,
                pre_score,
                post_score,
                initial_curve,
                final_curve,
                biquads,
                mean_spl,
                arrival_time_ms,
                fir_coeffs,
            )) => {
                {
                    let mut guard = callback_shared.lock().unwrap();
                    let stop = send_progress(
                        &mut guard,
                        &RoomOptimizationProgress {
                            current_speaker: channel_name.clone(),
                            speaker_index: speaker_idx,
                            total_speakers,
                            iteration: 0,
                            max_iterations: 0,
                            loss: post_score,
                            overall_progress: (speaker_idx + 1) as f64 / total_speakers as f64,
                            message: Some(format!(
                                "Channel {}: {:.4} -> {:.4}",
                                channel_name, pre_score, post_score
                            )),
                            epa_preference: None,
                        },
                    );
                    // Note: can't break here since we're inside a match arm.
                    // The stop signal is handled by the per-iteration callback.
                    let _ = stop;
                }

                results.push(Ok((
                    channel_name,
                    chain,
                    pre_score,
                    post_score,
                    initial_curve,
                    final_curve,
                    biquads,
                    mean_spl,
                    arrival_time_ms,
                    fir_coeffs,
                )));
            }
            Err(e) => {
                results.push(Err(e));
            }
        }
    }

    // Collect results
    let mut channel_chains: HashMap<String, ChannelDspChain> = HashMap::new();
    let mut channel_results: HashMap<String, ChannelOptimizationResult> = HashMap::new();
    let mut pre_scores: Vec<f64> = Vec::new();
    let mut post_scores: Vec<f64> = Vec::new();
    let mut curves: HashMap<String, crate::Curve> = HashMap::new();
    let mut channel_means: HashMap<String, f64> = HashMap::new();
    let mut channel_arrivals: HashMap<String, f64> = HashMap::new();

    for res in results {
        let (
            channel_name,
            chain,
            pre_score,
            post_score,
            initial_curve,
            final_curve,
            biquads,
            mean_spl,
            arrival_time_ms,
            fir_coeffs,
        ) = res?;

        channel_chains.insert(channel_name.clone(), chain);
        curves.insert(channel_name.clone(), final_curve.clone());
        pre_scores.push(pre_score);
        post_scores.push(post_score);
        channel_means.insert(channel_name.clone(), mean_spl);
        if let Some(arrival_ms) = arrival_time_ms {
            channel_arrivals.insert(channel_name.clone(), arrival_ms);
        }

        // Post-generate FIR coefficients for channels that need them but don't have them
        // (e.g., speaker groups that only support IIR internally)
        let fir_coeffs = if fir_coeffs.is_none()
            && !matches!(
                config.optimizer.processing_mode,
                ProcessingMode::LowLatency | ProcessingMode::MixedPhase
            ) {
            send_progress(
                &mut callback_shared.lock().unwrap(),
                &RoomOptimizationProgress {
                    current_speaker: format!("FIR: {}", channel_name),
                    speaker_index: 0,
                    total_speakers,
                    iteration: 0,
                    max_iterations: 0,
                    loss: 0.0,
                    overall_progress: 0.95,
                    message: Some(format!(
                        "Generating FIR coefficients for {}...",
                        channel_name
                    )),
                    epa_preference: None,
                },
            );
            post_generate_fir(
                &channel_name,
                &initial_curve,
                &final_curve,
                &config.optimizer,
                config.target_curve.as_ref(),
                sample_rate,
                output_dir,
            )
        } else {
            fir_coeffs
        };

        channel_results.insert(
            channel_name.clone(),
            ChannelOptimizationResult {
                name: channel_name,
                pre_score,
                post_score,
                initial_curve,
                final_curve,
                biquads,
                fir_coeffs,
            },
        );
    }

    // Auto IR sync: if no WAV-based arrivals were collected, estimate from phase data.
    // Runs unconditionally (does not require allow_delay = true).
    let phase_ir_sync = channel_arrivals.is_empty() && channel_results.len() > 1;
    if phase_ir_sync {
        for (channel_name, result) in &channel_results {
            if let Some(arrival_ms) =
                super::time_align::estimate_arrival_from_phase(&result.initial_curve, 200.0, 2000.0)
            {
                channel_arrivals.insert(channel_name.clone(), arrival_ms);
            }
        }
        if channel_arrivals.len() > 1 {
            info!(
                "Auto IR sync: phase-estimated arrival times for {} channels",
                channel_arrivals.len()
            );
            for (name, arrival) in &channel_arrivals {
                info!(
                    "  Channel '{}': phase-estimated arrival = {:.2} ms",
                    name, arrival
                );
            }
        } else {
            // Clear partial arrivals — not enough channels have phase data
            channel_arrivals.clear();
        }
    }

    // Time alignment: add delay plugins to align all channels to the slowest one
    // This is done PRE-EQ by inserting at the beginning of the plugin chain
    if (config.optimizer.allow_delay() || phase_ir_sync) && channel_arrivals.len() > 1 {
        let arrivals: Vec<f64> = channel_arrivals.values().copied().collect();
        let min_arrival = arrivals.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_arrival = arrivals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let arrival_spread = max_arrival - min_arrival;

        // Warn if arrival time differences are significant (might indicate measurement issues)
        if arrival_spread > ARRIVAL_TIME_WARNING_THRESHOLD_MS {
            warn!(
                "Channel arrival times differ by {:.1} ms (threshold: {:.1} ms). \
                This may indicate measurement issues or very different speaker distances.",
                arrival_spread, ARRIVAL_TIME_WARNING_THRESHOLD_MS
            );
            for (name, arrival) in &channel_arrivals {
                info!("  Channel '{}': arrival time = {:.2} ms", name, arrival);
            }
        }

        // Calculate alignment delays (slowest channel = reference, others get delays)
        let alignment_delays = super::time_align::calculate_alignment_delays(&channel_arrivals);

        // Add delay plugins at the BEGINNING of the chain (pre-EQ)
        for (channel_name, delay_ms) in &alignment_delays {
            // Only add delay plugin if the adjustment is significant (> 0.01 ms = ~0.5 samples at 48kHz)
            if *delay_ms > 0.01
                && let Some(chain) = channel_chains.get_mut(channel_name)
            {
                // Insert delay plugin at the beginning (before EQ)
                chain
                    .plugins
                    .insert(0, output::create_delay_plugin(*delay_ms));
                info!(
                    "  Channel '{}': added {:.3} ms delay for time alignment",
                    channel_name, delay_ms
                );
            }
        }
    } else if channel_arrivals.is_empty() && config.speakers.len() > 1 {
        info!("No arrival time data (WAV or phase) available for time alignment. Skipping.");
    }

    // Spectral channel alignment: fit low-shelf + high-shelf + flat gain to each
    // channel's deviation from the average post-EQ curve. This corrects both broadband
    // level differences and frequency-dependent tilt between channels.
    if curves.len() > 1 {
        send_progress(
            &mut callback_shared.lock().unwrap(),
            &RoomOptimizationProgress {
                current_speaker: "Spectral alignment".to_string(),
                speaker_index: 0,
                total_speakers,
                iteration: 0,
                max_iterations: 0,
                loss: 0.0,
                overall_progress: 0.92,
                message: Some("Spectral channel alignment...".to_string()),
                epa_preference: None,
            },
        );
        let min_freq = config.optimizer.min_freq;
        let max_freq = config.optimizer.max_freq;
        let sample_rate = config
            .recording_config
            .as_ref()
            .and_then(|rc| rc.playback_sample_rate)
            .unwrap_or(48000) as f64;

        // Compute post-EQ mean SPL per channel for the level spread warning
        let mut post_eq_means: HashMap<String, f64> = HashMap::new();
        for (channel_name, final_curve) in &curves {
            let freqs_f32: Vec<f32> = final_curve.freq.iter().map(|&f| f as f32).collect();
            let spl_f32: Vec<f32> = final_curve.spl.iter().map(|&s| s as f32).collect();
            let post_mean = compute_average_response(
                &freqs_f32,
                &spl_f32,
                Some((min_freq as f32, max_freq as f32)),
            ) as f64;
            post_eq_means.insert(channel_name.clone(), post_mean);
        }

        let means: Vec<f64> = post_eq_means.values().copied().collect();
        let min_mean = means.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_mean = means.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let level_spread = max_mean - min_mean;

        info!(
            "Post-EQ spectral alignment: level spread = {:.2} dB across {} channels",
            level_spread,
            post_eq_means.len()
        );
        for (name, mean) in &post_eq_means {
            info!("  Channel '{}': post-EQ mean SPL = {:.1} dB", name, mean);
        }

        // Warn if level differences are significant (might indicate measurement issues)
        if level_spread > LEVEL_DIFFERENCE_WARNING_THRESHOLD {
            warn!(
                "Channel levels differ by {:.1} dB (threshold: {:.1} dB). \
                This may indicate measurement issues (mic placement, cable problems, etc.).",
                level_spread, LEVEL_DIFFERENCE_WARNING_THRESHOLD
            );
        }

        // Compute spectral alignment (shelf + gain) for each channel
        let alignment_results = super::spectral_align::compute_spectral_alignment(
            &curves,
            sample_rate,
            min_freq,
            max_freq,
        );
        super::spectral_align::log_spectral_alignment(&alignment_results);

        // Insert alignment plugins after the per-channel PEQ
        for (channel_name, result) in &alignment_results {
            if let Some(chain) = channel_chains.get_mut(channel_name) {
                let (eq_plugin, gain_plugin) =
                    super::spectral_align::create_alignment_plugins(result, sample_rate);
                if let Some(eq) = eq_plugin {
                    chain.plugins.push(eq);
                }
                if let Some(gain) = gain_plugin {
                    chain.plugins.push(gain);
                }
            }
        }
    }

    // ========================================================================
    // Voice of God (VoG) — Timbre-match satellites to a reference channel
    // ========================================================================
    if let Some(vog_config) = &config.optimizer.vog
        && vog_config.enabled
    {
        send_progress(
            &mut callback_shared.lock().unwrap(),
            &RoomOptimizationProgress {
                current_speaker: "Voice of God".to_string(),
                speaker_index: 0,
                total_speakers,
                iteration: 0,
                max_iterations: 0,
                loss: 0.0,
                overall_progress: 0.93,
                message: Some(format!(
                    "Voice of God alignment (ref: '{}')...",
                    vog_config.reference_channel
                )),
                epa_preference: None,
            },
        );
        info!(
            "Running Voice of God alignment (reference: '{}')...",
            vog_config.reference_channel
        );

        // Build corrected curves from the current channel results
        let corrected_curves: HashMap<String, Curve> = channel_results
            .iter()
            .map(|(name, result)| (name.clone(), result.final_curve.clone()))
            .collect();

        match super::voice_of_god::compute_voice_of_god(
            &corrected_curves,
            &vog_config.reference_channel,
            sample_rate,
            config.optimizer.min_freq,
            config.optimizer.max_freq,
        ) {
            Ok(vog_results) => {
                for (channel_name, vog_result) in &vog_results {
                    let plugins = super::voice_of_god::create_vog_plugins(vog_result, sample_rate);
                    if !plugins.is_empty()
                        && let Some(chain) = channel_chains.get_mut(channel_name)
                    {
                        for plugin in plugins {
                            chain.plugins.push(plugin);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Voice of God optimization failed: {}", e);
            }
        }
    }

    // ========================================================================
    // Phase Alignment Optimization (Scenario A: WITH Subwoofers)
    // ========================================================================
    // Phase alignment maximizes energy sum in the crossover region by optimizing
    // delay and polarity. This runs BEFORE group delay optimization.
    // Uses the same sub-main pairing logic as GD-Opt v2 (system config or heuristic).
    let mut phase_alignment_results: HashMap<String, (f64, bool)> = HashMap::new();

    if config.optimizer.allow_delay()
        && let Some(phase_config) = &config.optimizer.phase_alignment
        && phase_config.enabled
    {
        let pairings = find_sub_main_pairings(config, &curves);

        if pairings.is_empty() {
            warn!("Phase alignment enabled but no valid sub-main pairings found.");
        } else {
            info!("Running phase alignment optimization...");
            send_progress(
                &mut callback_shared.lock().unwrap(),
                &RoomOptimizationProgress {
                    current_speaker: String::new(),
                    speaker_index: 0,
                    total_speakers: pairings.len(),
                    iteration: 0,
                    max_iterations: 0,
                    loss: 0.0,
                    overall_progress: 0.0,
                    message: Some("Running phase alignment...".to_string()),
                    epa_preference: None,
                },
            );

            for (sub_name, main_name) in &pairings {
                let sub_curve = match curves.get(sub_name) {
                    Some(c) => c,
                    None => {
                        warn!(
                            "Subwoofer channel '{}' not found for phase alignment",
                            sub_name
                        );
                        continue;
                    }
                };

                if let Some(speaker_curve) = curves.get(main_name) {
                    // Phase alignment requires phase data
                    if sub_curve.phase.is_some() && speaker_curve.phase.is_some() {
                        match phase_alignment::optimize_phase_alignment(
                            sub_curve,
                            speaker_curve,
                            phase_config,
                        ) {
                            Ok(result) => {
                                info!(
                                    "  Phase alignment '{}' with '{}': delay={:.2}ms, invert={}, improvement={:.2}dB",
                                    main_name,
                                    sub_name,
                                    result.delay_ms,
                                    result.invert_polarity,
                                    result.improvement_db
                                );
                                phase_alignment_results.insert(
                                    main_name.clone(),
                                    (result.delay_ms, result.invert_polarity),
                                );
                            }
                            Err(e) => {
                                warn!("  Phase alignment failed for '{}': {}", main_name, e);
                            }
                        }
                    } else {
                        debug!(
                            "  Skipping phase alignment for '{}': no phase data available",
                            main_name
                        );
                    }
                }
            }
        }
    }

    // Apply phase alignment results (polarity inversion + delay)
    for (speaker_name, (delay_ms, invert)) in &phase_alignment_results {
        if let Some(chain) = channel_chains.get_mut(speaker_name) {
            if *invert {
                // Insert polarity inversion at the beginning of the chain
                let invert_plugin = output::create_gain_plugin_with_invert(0.0, true);
                chain.plugins.insert(0, invert_plugin);
                info!("  Applied polarity inversion to '{}'", speaker_name);
            }
            if *delay_ms > 0.01 {
                // Apply phase alignment delay (additive with any existing time-alignment delay)
                output::add_delay_plugin(chain, *delay_ms);
                info!(
                    "  Applied {:.3} ms phase alignment delay to '{}'",
                    delay_ms, speaker_name
                );
            }
        }
    }

    // ========================================================================
    // Group Delay Optimization (v2) - IIR Mode
    // ========================================================================
    // Align Main speakers to Subwoofer using All-Pass filters to match phase slope
    if let Some(gd_opt) = &config.optimizer.gd_opt
        && gd_opt.enabled
        && config.optimizer.processing_mode == ProcessingMode::LowLatency
    {
        info!("Running Group Delay Optimization (IIR Mode)...");

        let pairings = find_sub_main_pairings(config, &curves);

        if pairings.is_empty() {
            warn!("GD-Opt enabled but no valid sub-main pairings found.");
        } else {
            send_progress(
                &mut callback_shared.lock().unwrap(),
                &RoomOptimizationProgress {
                    current_speaker: String::new(),
                    speaker_index: 0,
                    total_speakers: pairings.len(),
                    iteration: 0,
                    max_iterations: 0,
                    loss: 0.0,
                    overall_progress: 0.0,
                    message: Some("Running group delay optimization...".to_string()),
                    epa_preference: None,
                },
            );
        }

        let min_freq = config.optimizer.min_freq;
        let max_freq = 200.0;

        for (sub_name, main_name) in pairings {
            if let (Some(sub_curve), Some(main_curve)) =
                (curves.get(&sub_name), curves.get(&main_name))
            {
                info!("  Optimizing GD for '{}' vs '{}'", main_name, sub_name);
                send_progress(
                    &mut callback_shared.lock().unwrap(),
                    &RoomOptimizationProgress {
                        current_speaker: format!("GD {}", main_name),
                        speaker_index: 0,
                        total_speakers: 1,
                        iteration: 0,
                        max_iterations: 0,
                        loss: 0.0,
                        overall_progress: 0.0,
                        message: Some(format!("Optimizing GD for '{}'", main_name)),
                        epa_preference: None,
                    },
                );

                match group_delay::optimize_gd_iir(
                    sub_curve,
                    main_curve,
                    min_freq,
                    max_freq,
                    sample_rate,
                ) {
                    Ok(filters) => {
                        if !filters.is_empty() {
                            info!(
                                "    Generated {} All-Pass filters for GD alignment",
                                filters.len()
                            );
                            if let Some(chain) = channel_chains.get_mut(&main_name) {
                                let plugin = output::create_eq_plugin(&filters);
                                chain.plugins.push(plugin);
                            }
                        } else {
                            info!("    No AP filters needed");
                        }
                    }
                    Err(e) => {
                        warn!("    GD optimization failed for '{}': {}", main_name, e);
                    }
                }
            } else {
                warn!(
                    "GD-Opt: Channel '{}' or '{}' not found in results",
                    sub_name, main_name
                );
            }
        }
    }

    // Standalone phase correction (rePhase-style)
    if config.optimizer.phase_correction.is_some() {
        send_progress(
            &mut callback_shared.lock().unwrap(),
            &RoomOptimizationProgress {
                current_speaker: "Phase correction".to_string(),
                speaker_index: 0,
                total_speakers,
                iteration: 0,
                max_iterations: 0,
                loss: 0.0,
                overall_progress: 0.96,
                message: Some("Phase correction...".to_string()),
                epa_preference: None,
            },
        );
    }
    if let Some(ref pc_config) = config.optimizer.phase_correction {
        let names: Vec<String> = channel_results.keys().cloned().collect();
        for name in &names {
            if let Some(ch) = channel_results.get_mut(name.as_str())
                && let Some(chain) = channel_chains.get_mut(name.as_str())
            {
                apply_phase_correction(name, ch, chain, pc_config, sample_rate, output_dir);
            }
        }
    }

    // Compute IR waveforms (pre- and post-correction) for each channel
    send_progress(
        &mut callback_shared.lock().unwrap(),
        &RoomOptimizationProgress {
            current_speaker: "IR computation".to_string(),
            speaker_index: 0,
            total_speakers,
            iteration: 0,
            max_iterations: 0,
            loss: 0.0,
            overall_progress: 0.97,
            message: Some("Computing impulse responses...".to_string()),
            epa_preference: None,
        },
    );
    for (channel_name, result) in &channel_results {
        let delay_ms = channel_chains
            .get(channel_name)
            .and_then(|chain| chain.plugins.iter().find(|p| p.plugin_type == "delay"))
            .and_then(|p| p.parameters.get("delay_ms").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);

        if let Some((pre_ir, post_ir)) = super::ir_waveform::compute_channel_ir_waveforms(
            &result.initial_curve,
            &result.biquads,
            result.fir_coeffs.as_deref(),
            delay_ms,
            sample_rate,
        ) && let Some(chain) = channel_chains.get_mut(channel_name)
        {
            chain.pre_ir = Some(pre_ir);
            chain.post_ir = Some(post_ir);
        }
    }

    // Aggregate scores
    let avg_pre_score = if !pre_scores.is_empty() {
        pre_scores.iter().sum::<f64>() / pre_scores.len() as f64
    } else {
        0.0
    };
    let avg_post_score = if !post_scores.is_empty() {
        post_scores.iter().sum::<f64>() / post_scores.len() as f64
    } else {
        0.0
    };

    info!(
        "Average pre-score: {:.4}, post-score: {:.4}",
        avg_pre_score, avg_post_score
    );

    // Identify acoustic groups for consistency checks
    let acoustic_groups = identify_acoustic_groups(config);
    for (group_name, group_channels) in &acoustic_groups {
        if group_channels.len() > 1 {
            debug!("Acoustic Group '{}': {:?}", group_name, group_channels);

            // Perform consistency checks between speakers in the same group
            check_group_consistency(group_name, group_channels, &channel_means, &curves);
        }
    }

    let epa_cfg = config.optimizer.epa_config.clone().unwrap_or_default();
    let epa_per_channel = crate::roomeq::output::compute_epa_per_channel(&channel_chains, &epa_cfg);

    let metadata = OptimizationMetadata {
        pre_score: avg_pre_score,
        post_score: avg_post_score,
        algorithm: config.optimizer.algorithm.clone(),
        loss_type: Some(config.optimizer.loss_type.clone()),
        iterations: config.optimizer.max_iter,
        timestamp: chrono::Utc::now().to_rfc3339(),
        inter_channel_deviation: None,
        epa_per_channel,
    };

    let mut result = RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre_score,
        combined_post_score: avg_post_score,
        metadata,
    };

    // Compute inter-channel deviation and optionally correct it
    if curves.len() > 1 {
        send_progress(
            &mut callback_shared.lock().unwrap(),
            &RoomOptimizationProgress {
                current_speaker: "Channel matching".to_string(),
                speaker_index: 0,
                total_speakers,
                iteration: 0,
                max_iterations: 0,
                loss: 0.0,
                overall_progress: 0.98,
                message: Some("Channel matching analysis...".to_string()),
                epa_preference: None,
            },
        );
        compute_and_correct_icd(&mut result, config, sample_rate);
    }

    sanity_check_result(&result)?;
    Ok(result)
}

/// Compute inter-channel deviation and optionally apply correction filters.
///
/// Called after per-channel optimization on any result with >1 channel.
/// If `channel_matching` config is enabled and ICD exceeds threshold, adds
/// targeted PEQ filters to bring channels closer together.
fn compute_and_correct_icd(
    result: &mut RoomOptimizationResult,
    config: &RoomConfig,
    sample_rate: f64,
) {
    let final_curves: HashMap<String, crate::Curve> = result
        .channel_results
        .iter()
        .map(|(name, ch)| (name.clone(), ch.final_curve.clone()))
        .collect();

    let f3 = final_curves
        .values()
        .filter_map(|c| super::excursion::detect_f3(c, None).ok().map(|r| r.f3_hz))
        .reduce(f64::min)
        .unwrap_or(50.0);

    let icd = super::spectral_align::compute_inter_channel_deviation(&final_curves, f3);
    info!(
        "Inter-channel deviation: midrange_rms={:.2}dB, peak={:.1}dB @{:.0}Hz, passband_rms={:.2}dB",
        icd.midrange_rms_db, icd.midrange_peak_db, icd.midrange_peak_freq, icd.passband_rms_db,
    );

    // Check if correction is enabled and needed.
    // When the user omits `channel_matching`, fall back to `ChannelMatchingConfig::default()`
    // so the two paths (None vs Some(default)) produce identical behaviour.
    let matching_cfg = config
        .optimizer
        .channel_matching
        .clone()
        .unwrap_or_default();
    let enabled = matching_cfg.enabled;
    let threshold = matching_cfg.threshold_db;
    let max_filters = matching_cfg.max_filters;

    if enabled && icd.midrange_rms_db > threshold {
        info!(
            "ICD midrange_rms={:.2}dB > threshold={:.1}dB — applying channel matching correction (max {} filters/ch)",
            icd.midrange_rms_db, threshold, max_filters,
        );

        let corrections = super::spectral_align::correct_inter_channel_deviation(
            &final_curves,
            f3,
            max_filters,
            sample_rate,
        );

        for correction in &corrections {
            if let Some(plugin) = &correction.plugin {
                info!(
                    "  Channel '{}': {} matching filters",
                    correction.channel_name,
                    correction.filters.len(),
                );
                for f in &correction.filters {
                    info!(
                        "    PK @ {:.0} Hz, Q={:.2}, gain={:+.1} dB",
                        f.freq, f.q, f.db_gain,
                    );
                }

                // Add plugin to DSP chain
                if let Some(chain) = result.channels.get_mut(&correction.channel_name) {
                    chain.plugins.push(plugin.clone());
                }

                // Update the final curve with the correction applied
                if let Some(ch_result) = result.channel_results.get_mut(&correction.channel_name) {
                    let resp = crate::response::compute_peq_complex_response(
                        &correction.filters,
                        &ch_result.final_curve.freq,
                        sample_rate,
                    );
                    ch_result.final_curve =
                        crate::response::apply_complex_response(&ch_result.final_curve, &resp);

                    // Also update the display final_curve in the chain
                    if let Some(chain) = result.channels.get_mut(&correction.channel_name)
                        && let Some(ref display_final) = chain.final_curve
                    {
                        let display_curve: crate::Curve = display_final.clone().into();
                        let display_resp = crate::response::compute_peq_complex_response(
                            &correction.filters,
                            &display_curve.freq,
                            sample_rate,
                        );
                        let corrected =
                            crate::response::apply_complex_response(&display_curve, &display_resp);
                        chain.final_curve = Some((&corrected).into());
                    }
                }
            }
        }

        // Re-compute ICD after correction
        let corrected_curves: HashMap<String, crate::Curve> = result
            .channel_results
            .iter()
            .map(|(name, ch)| (name.clone(), ch.final_curve.clone()))
            .collect();
        let icd_after =
            super::spectral_align::compute_inter_channel_deviation(&corrected_curves, f3);
        info!(
            "ICD after correction: midrange_rms={:.2}dB (was {:.2}dB), peak={:.1}dB @{:.0}Hz",
            icd_after.midrange_rms_db,
            icd.midrange_rms_db,
            icd_after.midrange_peak_db,
            icd_after.midrange_peak_freq,
        );
        result.metadata.inter_channel_deviation = Some(icd_after);
    } else {
        if enabled {
            info!(
                "ICD midrange_rms={:.2}dB <= threshold={:.1}dB — no correction needed",
                icd.midrange_rms_db, threshold,
            );
        }
        result.metadata.inter_channel_deviation = Some(icd);
    }
}

/// Identify Acoustic Groups from RoomConfig
///
/// Acoustic Groups are speakers expected to be acoustically similar (e.g., L/R pair).
/// Uses explicit speaker_name metadata if available, otherwise falls back to
/// positional heuristics (L/R, SL/SR, etc.).
fn identify_acoustic_groups(config: &RoomConfig) -> HashMap<String, Vec<String>> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    let mut positioned_channels: HashMap<String, String> = HashMap::new();

    // 1. Group by explicit speaker_name
    for (channel_name, speaker_cfg) in &config.speakers {
        if let Some(speaker_name) = speaker_cfg.speaker_name() {
            groups
                .entry(speaker_name.to_string())
                .or_default()
                .push(channel_name.clone());
        } else {
            positioned_channels.insert(channel_name.clone(), channel_name.clone());
        }
    }

    // 2. Positional heuristics for remaining channels
    let pairs = [
        ("L", "R"),
        ("SL", "SR"),
        ("SBL", "SBR"),
        ("TFL", "TFR"),
        ("TRL", "TRR"),
        ("FWL", "FWR"),
    ];

    for (p1, p2) in pairs {
        if positioned_channels.contains_key(p1) && positioned_channels.contains_key(p2) {
            let group_name = format!("{}-{}", p1, p2);
            let mut group = Vec::new();
            if let Some(c1) = positioned_channels.remove(p1) {
                group.push(c1);
            }
            if let Some(c2) = positioned_channels.remove(p2) {
                group.push(c2);
            }
            groups.insert(group_name, group);
        }
    }

    groups
}

/// Optimize a single speaker (simple or group)
///
/// # Arguments
/// * `channel_name` - Name of the channel
/// * `speaker_config` - Speaker configuration
/// * `optimizer_config` - Optimizer parameters
/// * `target_curve` - Optional target curve configuration
/// * `sample_rate` - Sample rate for filter design
/// * `callback` - Optional progress callback
///
/// # Returns
/// * `SpeakerOptimizationResult` containing DSP chain and optimization results
pub fn optimize_speaker(
    channel_name: &str,
    speaker_config: &SpeakerConfig,
    optimizer_config: &OptimizerConfig,
    target_curve: Option<&TargetCurveConfig>,
    sample_rate: f64,
    _callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult> {
    // Migrate legacy `target_tilt` + `broadband_target_matching` into `target_response`
    // to match the behaviour of `optimize_room_impl`. Without this, in-memory callers
    // that go through `optimize_speaker` with a legacy config silently take a divergent
    // path in `speaker_eq.rs`. Idempotent — safe to call even on already-migrated configs.
    let mut optimizer_config = optimizer_config.clone();
    optimizer_config.migrate_target_config();

    // Create a minimal RoomConfig for internal processing
    let room_config = RoomConfig {
        version: super::types::default_config_version(),
        system: None,
        speakers: HashMap::new(),
        crossovers: None,
        target_curve: target_curve.cloned(),
        optimizer: optimizer_config,
        recording_config: None,
        cea2034_cache: None,
    };

    let (
        chain,
        pre_score,
        post_score,
        initial_curve,
        final_curve,
        biquads,
        _mean_spl,
        _arrival_time_ms,
        fir_coeffs,
    ) = process_speaker_internal(
        channel_name,
        speaker_config,
        &room_config,
        sample_rate,
        None,
        None,
        None, // no shared mean for standalone single-channel optimization
        None, // no probe_arrival_overrides on the standalone path
    )?;

    Ok(SpeakerOptimizationResult {
        chain,
        pre_score,
        post_score,
        initial_curve,
        final_curve,
        biquads,
        fir_coeffs,
    })
}

// ============================================================================
// Internal Processing Functions
// ============================================================================

/// Process a single speaker (simple or group)
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
///
/// `probe_arrival_overrides` — optional per-channel probe-based arrival times
/// (from the delay-detection UI step). When present and the channel name has a
/// matching entry, this overrides the WAV-onset fallback inside
/// `process_single_speaker`.
fn process_speaker_internal(
    channel_name: &str,
    speaker_config: &SpeakerConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    output_dir: Option<&Path>,
    callback: Option<crate::optim::OptimProgressCallback>,
    shared_mean_spl: Option<f64>,
    probe_arrival_overrides: Option<&HashMap<String, f64>>,
) -> Result<MixedModeResult> {
    let output_dir = output_dir.unwrap_or(Path::new("."));

    match speaker_config {
        SpeakerConfig::Single(source) => {
            let probe_arrival_ms =
                probe_arrival_overrides.and_then(|m| m.get(channel_name).copied());
            process_single_speaker(
                channel_name,
                source,
                room_config,
                sample_rate,
                output_dir,
                callback,
                probe_arrival_ms,
                shared_mean_spl,
            )
        }
        SpeakerConfig::Group(group) => {
            process_speaker_group(channel_name, group, room_config, sample_rate, output_dir)
        }
        SpeakerConfig::MultiSub(group) => {
            process_multisub_group(channel_name, group, room_config, sample_rate, output_dir)
        }
        SpeakerConfig::Dba(config) => {
            process_dba(channel_name, config, room_config, sample_rate, output_dir)
        }
        SpeakerConfig::Cardioid(config) => {
            process_cardioid(channel_name, config, room_config, sample_rate, output_dir)
        }
    }
}

/// Extract wav_path from a MeasurementSource if available
pub(super) fn extract_wav_path(source: &MeasurementSource) -> Option<String> {
    match source {
        MeasurementSource::Single(s) => {
            if let crate::MeasurementRef::Inline(inline) = &s.measurement {
                inline.wav_path.clone()
            } else {
                None
            }
        }
        MeasurementSource::Multiple(m) => {
            // Use the first measurement's wav_path if available
            m.measurements.first().and_then(|r| {
                if let crate::MeasurementRef::Inline(inline) = r {
                    inline.wav_path.clone()
                } else {
                    None
                }
            })
        }
        MeasurementSource::InMemory(_) | MeasurementSource::InMemoryMultiple(_) => None,
    }
}
