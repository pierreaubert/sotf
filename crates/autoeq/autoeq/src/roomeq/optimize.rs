//! Main optimization entry points for room EQ.
//!
//! This module provides the primary public API for room optimization.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use log::{debug, info, warn};
use math_audio_dsp::analysis::{compute_average_response, find_db_point};
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
    ChannelDspChain, DspChainOutput, MeasurementSource,
    OptimizationMetadata, OptimizerConfig, ProcessingMode, RoomConfig, SpeakerConfig,
    SystemModel, TargetCurveConfig,
};

// Private module imports for extracted functions
use super::speaker_eq::process_single_speaker;
pub(super) use super::speaker_eq::optimize_eq_with_optional_schroeder; // Re-export for workflows

use super::group_processing::{
    process_speaker_group, process_multisub_group, process_dba, process_cardioid,
};
use super::crossover_utils::check_group_consistency;

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

/// Detect passband and compute mean SPL for normalization
///
/// Smooths the measurement at 1 octave to eliminate room mode dips, then
/// finds the -10 dB points relative to the median SPL. Validates the result
/// against 2-octave smoothing to ensure stability.
pub(super) fn detect_passband_and_mean(curve: &Curve) -> (Option<(f64, f64)>, f64) {
    let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();

    if spl_f32.is_empty() {
        return (None, 0.0);
    }

    // Smooth at 1 octave to filter out room modes and comb filtering
    let smoothed_1oct = crate::read::smooth_one_over_n_octave(curve, 1);
    let spl_1oct: Vec<f32> = smoothed_1oct.spl.iter().map(|&s| s as f32).collect();

    // Use median of smoothed SPL as reference (robust to residual peaks/nulls)
    let mut sorted_spl: Vec<f32> = spl_1oct.clone();
    sorted_spl.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_spl = sorted_spl[sorted_spl.len() / 2];

    if median_spl < -100.0 {
        return (None, 0.0);
    }

    let threshold = median_spl - 10.0;
    let f_low_1 =
        find_db_point(&freqs_f32, &spl_1oct, threshold, true).unwrap_or(freqs_f32[0]);
    let f_high_1 = find_db_point(&freqs_f32, &spl_1oct, threshold, false)
        .unwrap_or(freqs_f32[freqs_f32.len() - 1]);

    // Validate against double-smoothed (1 oct applied twice ≈ 2 oct effective).
    // If the passband differs significantly, the 1-oct result is unstable;
    // use the wider estimate.
    let spl_2oct: Vec<f32> = {
        let double_smooth = crate::read::smooth_one_over_n_octave(&smoothed_1oct, 1);
        double_smooth.spl.iter().map(|&s| s as f32).collect()
    };

    let f_low_2 =
        find_db_point(&freqs_f32, &spl_2oct, threshold, true).unwrap_or(freqs_f32[0]);
    let f_high_2 = find_db_point(&freqs_f32, &spl_2oct, threshold, false)
        .unwrap_or(freqs_f32[freqs_f32.len() - 1]);

    // Use the wider (more conservative) passband from the two estimates
    let f_low = f_low_1.min(f_low_2);
    let f_high = f_high_1.max(f_high_2);

    // Compute mean on the original (unsmoothed) curve within the detected passband
    let norm_range_f32 = Some((f_low, f_high));
    let mean = compute_average_response(&freqs_f32, &spl_f32, norm_range_f32) as f64;

    (Some((f_low as f64, f_high as f64)), mean)
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
///
/// # Returns
/// * `RoomOptimizationResult` containing DSP chains and optimization results
pub fn optimize_room(
    config: &RoomConfig,
    sample_rate: f64,
    mut callback: Option<RoomOptimizationCallback>,
    output_dir: Option<&Path>,
) -> Result<RoomOptimizationResult> {
    // Ensure legacy target_tilt/broadband fields are migrated.
    // load_config() does this already, but callers building RoomConfig in memory
    // (tests, GPUI) may not have called it.
    let mut config = config.clone();
    config.optimizer.migrate_target_config();
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

        // The Stereo 2.0 workflow (no subwoofer) doesn't implement per-channel
        // features like excursion protection, target tilt, or broadband matching.
        // These are only in process_single_speaker (the generic path). For simple
        // stereo configs, fall through to the generic path when these features are
        // active. Multi-channel workflows (2.1, 5.1) have subwoofer/crossover logic
        // that the generic path cannot replicate, so keep them on their workflows.
        let use_generic_for_stereo = sys.model == SystemModel::Stereo
            && sys.subwoofers.is_none()
            && (config
                .optimizer
                .excursion_protection
                .as_ref()
                .is_some_and(|e| e.enabled)
                || config.optimizer.target_response.is_some()
                || config.optimizer.target_tilt.is_some()
                || config
                    .optimizer
                    .broadband_target_matching
                    .as_ref()
                    .is_some_and(|b| b.enabled));

        if use_generic_for_stereo {
            info!(
                "Stereo 2.0 with excursion/tilt/broadband features, using generic path"
            );
        }

        if !has_group && !use_generic_for_stereo {
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
                    },
                );
                // Workflows only do IIR. If FIR/mixed mode is requested, post-generate
                // FIR coefficients for each channel from its initial measurement curve.
                // MixedPhase handles its own FIR generation internally.
                if !matches!(config.optimizer.processing_mode, ProcessingMode::LowLatency | ProcessingMode::MixedPhase) {
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
                // Compute IR waveforms for the workflow result
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
    let desired_pop = config.optimizer.population.max(1).min(config.optimizer.max_iter.max(1));
    let pop_multiplier = desired_pop.div_ceil(n_free).max(4);
    let population_size = pop_multiplier * n_free;
    let max_iterations = (config.optimizer.max_iter.saturating_sub(population_size) / population_size).max(5000);
    info!(
        "DE budget: {} params, population_size={}, max_generations={} (from max_iter={}, floor=5000)",
        n_params, population_size, max_iterations, config.optimizer.max_iter
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
            Some(Box::new(move |iter: usize, loss: f64| {
                let base_progress = si as f64 / ts as f64;
                let speaker_progress = if mi > 0 {
                    iter as f64 / mi as f64
                } else {
                    0.0
                };
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
            && !matches!(config.optimizer.processing_mode, ProcessingMode::LowLatency | ProcessingMode::MixedPhase)
        {
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
                        && let Some(chain) = channel_chains.get_mut(channel_name) {
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

    // Compute IR waveforms (pre- and post-correction) for each channel
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

    let metadata = OptimizationMetadata {
        pre_score: avg_pre_score,
        post_score: avg_post_score,
        algorithm: config.optimizer.algorithm.clone(),
        iterations: config.optimizer.max_iter,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre_score,
        combined_post_score: avg_post_score,
        metadata,
    })
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
    // Create a minimal RoomConfig for internal processing
    let room_config = RoomConfig {
        version: super::types::default_config_version(),
        system: None,
        speakers: HashMap::new(),
        crossovers: None,
        target_curve: target_curve.cloned(),
        optimizer: optimizer_config.clone(),
        recording_config: None,
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
fn process_speaker_internal(
    channel_name: &str,
    speaker_config: &SpeakerConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    output_dir: Option<&Path>,
    callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<MixedModeResult> {
    let output_dir = output_dir.unwrap_or(Path::new("."));

    match speaker_config {
        SpeakerConfig::Single(source) => {
            process_single_speaker(
                channel_name,
                source,
                room_config,
                sample_rate,
                output_dir,
                callback,
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

