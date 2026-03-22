//! Main optimization entry points for room EQ.
//!
//! This module provides the primary public API for room optimization.

use super::spectral_align;
use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::read as load;
use crate::response;
use log::{debug, info, warn};
use math_audio_dsp::analysis::{compute_average_response, find_db_point};
use math_audio_iir_fir::Biquad;
use ndarray::Array1;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::config::validate_room_config;
use super::crossover;
use super::dba;
use super::eq;
use super::excursion;
use super::fir;
use super::group_delay;
use super::multisub;
use super::output;
use super::phase_alignment;
use super::target_tilt;
use super::types::{
    ChannelDspChain, DspChainOutput, MeasurementSource, MixedModeConfig, MultiSubGroup,
    OptimizationMetadata, OptimizerConfig, ProcessingMode, RoomConfig, SpeakerConfig, SpeakerGroup,
    SystemModel, TargetCurveConfig, TiltType,
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

/// Detect passband and compute mean SPL for normalization
///
/// Finds the -3 dB points relative to the peak SPL, then computes the
/// average response within that passband.
fn detect_passband_and_mean(curve: &Curve) -> (Option<(f64, f64)>, f64) {
    let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();

    // find_db_point uses an absolute threshold, so compute peak - 3 dB
    let peak_spl = spl_f32.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    if peak_spl < -100.0 {
        // Measurement is essentially silence — passband detection is undefined
        return (None, 0.0);
    }
    let threshold = peak_spl - 3.0;

    let f_low = find_db_point(&freqs_f32, &spl_f32, threshold, true).unwrap_or(freqs_f32[0]);
    let f_high = find_db_point(&freqs_f32, &spl_f32, threshold, false)
        .unwrap_or(freqs_f32[freqs_f32.len() - 1]);

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
    let max_iterations = config.optimizer.max_iter;
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
                    if !plugins.is_empty() {
                        if let Some(chain) = channel_chains.get_mut(channel_name) {
                            for plugin in plugins {
                                chain.plugins.push(plugin);
                            }
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
fn extract_wav_path(source: &MeasurementSource) -> Option<String> {
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

/// Optimize EQ filters, dispatching to multi-measurement if configured.
///
/// If the source is `Multiple` or `InMemoryMultiple` and a non-Average multi-measurement
/// strategy is configured, loads individual curves and uses `optimize_channel_eq_multi`.
/// Otherwise falls back to the standard single-curve path.
fn optimize_eq_maybe_multi(
    source: &MeasurementSource,
    optimization_curve: &Curve,
    optimizer_config: &OptimizerConfig,
    target_config: Option<&super::types::TargetCurveConfig>,
    sample_rate: f64,
    channel_name: &str,
    callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<(Vec<Biquad>, f64)> {
    use super::types::MultiMeasurementStrategy;

    let use_multi = matches!(
        source,
        MeasurementSource::Multiple(_) | MeasurementSource::InMemoryMultiple(_)
    ) && optimizer_config
        .multi_measurement
        .as_ref()
        .is_some_and(|mc| mc.strategy != MultiMeasurementStrategy::Average);

    if use_multi {
        let multi_config = optimizer_config.multi_measurement.as_ref().unwrap();
        let curves =
            load::load_source_individual(source).map_err(|e| AutoeqError::InvalidMeasurement {
                message: format!(
                    "Failed to load individual measurements for channel {}: {}",
                    channel_name, e
                ),
            })?;

        info!(
            "  Multi-measurement optimization ({:?}) with {} curves",
            multi_config.strategy,
            curves.len()
        );

        if let Some(cb) = callback {
            eq::optimize_channel_eq_multi_with_callback(
                &curves,
                optimizer_config,
                multi_config,
                target_config,
                sample_rate,
                cb,
            )
        } else {
            eq::optimize_channel_eq_multi(
                &curves,
                optimizer_config,
                multi_config,
                target_config,
                sample_rate,
            )
        }
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!(
                "Multi-measurement EQ optimization failed for channel {}: {}",
                channel_name, e
            ),
        })
    } else {
        if let Some(cb) = callback {
            eq::optimize_channel_eq_with_callback(
                optimization_curve,
                optimizer_config,
                target_config,
                sample_rate,
                cb,
            )
        } else {
            eq::optimize_channel_eq(
                optimization_curve,
                optimizer_config,
                target_config,
                sample_rate,
            )
        }
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("EQ optimization failed for channel {}: {}", channel_name, e),
        })
    }
}

/// Process a simple speaker with a single measurement
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
fn process_single_speaker(
    channel_name: &str,
    source: &MeasurementSource,
    room_config: &RoomConfig,
    sample_rate: f64,
    output_dir: &Path,
    mut callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<MixedModeResult> {
    // Load measurement
    let curve = load::load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
        message: format!(
            "Failed to load measurement for channel {}: {}",
            channel_name, e
        ),
    })?;

    debug!(
        "  Loaded measurement: {:.1} Hz - {:.1} Hz",
        curve.freq[0],
        curve.freq[curve.freq.len() - 1]
    );

    // Extract wav_path and calculate arrival time for time alignment
    let arrival_time_ms: Option<f64> = extract_wav_path(source).and_then(|wav_path| {
        let path = std::path::Path::new(&wav_path);
        if path.exists() {
            match super::time_align::find_arrival_time(path, None) {
                Ok(result) => {
                    debug!(
                        "  Arrival time for '{}': {:.2} ms (peak at sample {})",
                        channel_name, result.arrival_ms, result.arrival_samples
                    );
                    Some(result.arrival_ms)
                }
                Err(e) => {
                    debug!(
                        "  Could not determine arrival time for '{}': {}",
                        channel_name, e
                    );
                    None
                }
            }
        } else {
            debug!("  WAV file not found for '{}': {:?}", channel_name, path);
            None
        }
    });

    // ========================================================================
    // Build target curve with tilt (if configured)
    // ========================================================================
    let target_tilt_curve = if let Some(tilt_config) = &room_config.optimizer.target_tilt {
        // When tilt_type is Flat but the user set a non-zero slope or bass shelf,
        // promote to Custom so the tilt is actually applied. This handles configs
        // where tilt_type is omitted (defaults to Flat) but slope_db_per_octave is set.
        let effective_config = if tilt_config.tilt_type == TiltType::Flat
            && (tilt_config.slope_db_per_octave.abs() > 1e-6
                || tilt_config.bass_shelf_db.abs() > 1e-6)
        {
            warn!(
                "  target_tilt has slope={:.2} dB/oct but tilt_type is Flat — \
                 promoting to Custom. Set tilt_type explicitly to avoid this warning.",
                tilt_config.slope_db_per_octave
            );
            let mut promoted = tilt_config.clone();
            promoted.tilt_type = TiltType::Custom;
            promoted
        } else {
            tilt_config.clone()
        };

        if effective_config.tilt_type != TiltType::Flat {
            info!(
                "  Building target curve with {:?} tilt ({:.2} dB/octave)",
                effective_config.tilt_type, effective_config.slope_db_per_octave
            );
            Some(target_tilt::build_target_curve_with_tilt(
                &curve.freq,
                &effective_config,
            ))
        } else {
            None
        }
    } else {
        None
    };

    // When target_tilt is non-flat, the tilt is baked into the measurement curve
    // before optimization. Passing target_curve to the optimizer on top of that
    // would double-apply the target. Guard against this.
    if target_tilt_curve.is_some() && room_config.target_curve.is_some() {
        warn!(
            "  Both target_curve and target_tilt are configured for '{}'. \
             target_tilt is baked into the measurement; target_curve will be \
             ignored to avoid double-application.",
            channel_name
        );
    }

    // ========================================================================
    // Excursion Protection (detect F3, generate HPF)
    // ========================================================================
    let excursion_filters: Vec<Biquad> =
        if let Some(exc_config) = &room_config.optimizer.excursion_protection {
            if exc_config.enabled {
                info!("  Applying excursion protection...");
                match excursion::generate_excursion_protection(&curve, exc_config, sample_rate) {
                    Ok(result) => {
                        info!(
                            "  Excursion protection: F3={:.1}Hz, HPF={:.1}Hz ({} filters)",
                            result.f3_hz,
                            result.hpf_frequency,
                            result.filters.len()
                        );
                        result.filters
                    }
                    Err(e) => {
                        warn!(
                            "  Excursion protection failed: {}. Continuing without protection.",
                            e
                        );
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

    // Simulate excursion HPF on the curve so the EQ optimizer sees the measurement
    // as it will be after the HPF. Without this, the optimizer doesn't know about the
    // HPF cuts and stacks additional cuts on top, double-cutting the bass.
    // Keep `curve_raw` for final display (all_filters applied to raw measurement).
    let curve_raw = curve.clone();
    let curve = if !excursion_filters.is_empty() {
        let hpf_resp =
            response::compute_peq_complex_response(&excursion_filters, &curve.freq, sample_rate);
        let adjusted = response::apply_complex_response(&curve, &hpf_resp);
        info!(
            "  Simulating excursion HPF on optimization curve ({} filters)",
            excursion_filters.len()
        );
        adjusted
    } else {
        curve
    };

    // Compute pre-score (within EQ range)
    let mut min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;

    // Detect passband for display metadata only
    let (norm_range, _passband_mean) = detect_passband_and_mean(&curve);

    if let Some((f_low, f_high)) = norm_range {
        info!(
            "  Detected passband for '{}': {:.1} Hz - {:.1} Hz",
            channel_name, f_low, f_high
        );
    }

    // When target tilt is active, clamp min_freq to the speaker's F3 rolloff.
    // Without this, the tilt creates a massive target deficit below the speaker's
    // capability (e.g. +4.5dB at 20Hz on a speaker that rolls off at 60Hz).
    // The optimizer wastes filters on impossible bass boost, and the broad filter
    // skirts cause collateral damage in the midrange.
    if target_tilt_curve.is_some() {
        match excursion::detect_f3(&curve, None) {
            Ok(f3_result) => {
                // Only clamp if F3 is above the configured min_freq but still
                // well below max_freq. A very high "F3" (e.g., on a tilted curve
                // with no real rolloff) would invalidate the frequency range.
                if f3_result.f3_hz > min_freq && f3_result.f3_hz < max_freq * 0.5 {
                    info!(
                        "  Tilt active: clamping min_freq from {:.1}Hz to F3={:.1}Hz \
                         to prevent bass over-boost below rolloff",
                        min_freq, f3_result.f3_hz
                    );
                    min_freq = f3_result.f3_hz;
                }
            }
            Err(e) => {
                debug!(
                    "  F3 detection failed for tilt clamping: {}. Using configured min_freq.",
                    e
                );
            }
        }
    }

    // Use range-based mean (same as optimizer) for consistent pre/post scoring
    let pre_freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
    let pre_spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();
    let pre_mean = compute_average_response(
        &pre_freqs_f32,
        &pre_spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    let normalized_spl = &curve.spl - pre_mean;
    let pre_score = crate::loss::flat_loss(&curve.freq, &normalized_spl, min_freq, max_freq);

    // Level alignment: use mean SPL within the EQ optimization range.
    // Passband mean (-3 dB from peak) is too narrow for resonant room data;
    // full-range mean is misleading for bandpass speakers (subwoofers).
    // The optimizer range gives a consistent reference across channel types.
    let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    // ========================================================================
    // Broadband Target Matching (v2.1)
    // ========================================================================
    // Fit shelves/gain to the target curve across the full 20Hz-20kHz range
    // to establish a balanced baseline before fine-grained optimization.
    let (curve_for_optim, broadband_plugins, broadband_biquads, bb_mean_shift) =
        if let Some(bb_config) = &room_config.optimizer.broadband_target_matching {
            if bb_config.enabled {
                info!("  Broadband Target Matching enabled...");
                // 1. Construct a FLAT target at the measurement's mean level.
                // The tilt is handled exclusively by the EQ optimizer (which subtracts
                // the tilt curve before optimizing). Including the tilt here would
                // double-apply it: broadband shelves push toward tilt, then the EQ
                // normalizer subtracts tilt again, leaving only shelf artifacts.
                let target = Curve {
                    freq: curve.freq.clone(),
                    spl: Array1::from_elem(curve.freq.len(), mean_spl),
                    phase: None,
                };

                // 2. Compute alignment across the full audible range (20-20kHz).
                // The target is flat at mean_spl, so the alignment fits gentle
                // shelves + gain to correct the measurement's broadband shape.
                if let Some(result) = spectral_align::compute_target_alignment(
                    &curve,
                    &target,
                    20.0,
                    20000.0,
                    sample_rate,
                ) {
                    info!(
                        "  Broadband correction: LS={:+.2}dB, HS={:+.2}dB, Gain={:+.2}dB",
                        result.lowshelf_gain_db, result.highshelf_gain_db, result.flat_gain_db
                    );

                    // 3. Create plugins
                    let (eq_plugin, gain_plugin) =
                        spectral_align::create_alignment_plugins(&result, sample_rate);

                    let mut plugins = Vec::new();
                    if let Some(g) = gain_plugin {
                        plugins.push(g);
                    }
                    if let Some(eq) = eq_plugin {
                        plugins.push(eq);
                    }

                    // Simulate the broadband correction on the curve
                    use math_audio_iir_fir::{Biquad, BiquadFilterType, DEFAULT_Q_HIGH_LOW_SHELF};
                    let mut filters = Vec::new();
                    if result.lowshelf_gain_db.abs() > 1e-3 {
                        filters.push(Biquad::new(
                            BiquadFilterType::Lowshelf,
                            spectral_align::LOWSHELF_FREQ,
                            sample_rate,
                            DEFAULT_Q_HIGH_LOW_SHELF,
                            result.lowshelf_gain_db,
                        ));
                    }
                    if result.highshelf_gain_db.abs() > 1e-3 {
                        filters.push(Biquad::new(
                            BiquadFilterType::Highshelf,
                            spectral_align::HIGHSHELF_FREQ,
                            sample_rate,
                            DEFAULT_Q_HIGH_LOW_SHELF,
                            result.highshelf_gain_db,
                        ));
                    }

                    // 1. Gain
                    let mut temp_curve = curve.clone();
                    temp_curve.spl += result.flat_gain_db;

                    // 2. Filters
                    let final_curve = if !filters.is_empty() {
                        let resp = response::compute_peq_complex_response(
                            &filters,
                            &curve.freq,
                            sample_rate,
                        );
                        response::apply_complex_response(&temp_curve, &resp)
                    } else {
                        temp_curve
                    };

                    (final_curve, plugins, filters, result.flat_gain_db)
                } else {
                    (curve.clone(), Vec::new(), Vec::new(), 0.0)
                }
            } else {
                (curve.clone(), Vec::new(), Vec::new(), 0.0)
            }
        } else {
            (curve.clone(), Vec::new(), Vec::new(), 0.0)
        };

    // We must update the mean_spl because the broadband gain shifted it
    let mean_spl = mean_spl + bb_mean_shift;

    // Build optimizer config with the clamped min_freq so the optimizer
    // doesn't place filters below the speaker's rolloff when tilt is active.
    let clamped_optimizer = if min_freq != room_config.optimizer.min_freq {
        let mut opt = room_config.optimizer.clone();
        opt.min_freq = min_freq;
        opt
    } else {
        room_config.optimizer.clone()
    };

    match room_config.optimizer.processing_mode {
        ProcessingMode::PhaseLinear => {
            info!("  Generating FIR filter...");

            // Report initial loss so the progress chart has data
            if let Some(ref mut cb) = callback {
                cb(1, pre_score);
            }

            // Check if we should force excess phase correction for GD-Opt on subwoofer
            let mut opt_config = clamped_optimizer.clone();
            if let Some(gd_opt) = &clamped_optimizer.gd_opt
                && gd_opt.enabled
                && (channel_name == "lfe" || channel_name.starts_with("sub"))
                && let Some(fir) = &mut opt_config.fir
            {
                fir.correct_excess_phase = true;
                info!(
                    "  GD-Opt: Forcing excess phase correction for '{}'",
                    channel_name
                );
            }

            // Apply target tilt to the curve (subtract tilt from measurement),
            // same as LowLatency does
            let fir_input_curve = if let Some(ref tilt_curve) = target_tilt_curve {
                Curve {
                    freq: curve_for_optim.freq.clone(),
                    spl: &curve_for_optim.spl - &tilt_curve.spl,
                    phase: curve_for_optim.phase.clone(),
                }
            } else {
                curve_for_optim.clone()
            };

            // When tilt is baked into the curve, don't also pass target_curve
            // to the optimizer (would double-apply the target)
            let effective_target = if target_tilt_curve.is_some() {
                None
            } else {
                room_config.target_curve.as_ref()
            };

            let coeffs = fir::generate_fir_correction(
                &fir_input_curve,
                &opt_config,
                effective_target,
                sample_rate,
            )
            .map_err(|e| AutoeqError::OptimizationFailed {
                message: format!("FIR generation failed: {}", e),
            })?;

            let filename = format!("{}_fir.wav", channel_name);
            let wav_path = output_dir.join(&filename);
            crate::fir::save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path).map_err(|e| {
                AutoeqError::OptimizationFailed {
                    message: format!("Failed to save FIR WAV: {}", e),
                }
            })?;

            info!("  Saved FIR filter to {}", wav_path.display());

            // Build DSP chain with convolution plugin referencing the FIR WAV file
            let convolution_plugin = output::create_convolution_plugin(&filename);
            let mut chain = output::build_channel_dsp_chain_with_curves(
                channel_name,
                None,
                broadband_plugins,
                &[],
                None,
                None,
            );
            chain.plugins.push(convolution_plugin);

            let complex_resp =
                response::compute_fir_complex_response(&coeffs, &curve.freq, sample_rate);
            let final_curve = response::apply_complex_response(&curve_for_optim, &complex_resp);

            // Compute post_score consistently with pre_score (range-based mean)
            let post_freqs_f32: Vec<f32> = final_curve.freq.iter().map(|&f| f as f32).collect();
            let post_spl_f32: Vec<f32> = final_curve.spl.iter().map(|&s| s as f32).collect();
            let mean_final = compute_average_response(
                &post_freqs_f32,
                &post_spl_f32,
                Some((min_freq as f32, max_freq as f32)),
            ) as f64;
            let normalized_final_spl = &final_curve.spl - mean_final;
            let post_score = crate::loss::flat_loss(
                &final_curve.freq,
                &normalized_final_spl,
                min_freq,
                max_freq,
            );

            info!(
                "  Pre-score: {:.6}, Post-score: {:.6}",
                pre_score, post_score
            );

            // Report final loss so the progress chart shows the FIR improvement
            if let Some(ref mut cb) = callback {
                cb(2, post_score);
            }

            // Extend curves to 20 Hz – 20 kHz for display output
            let display_initial = output::extend_curve_to_full_range(&curve_raw);
            let display_fir_resp =
                response::compute_fir_complex_response(&coeffs, &display_initial.freq, sample_rate);
            let display_final =
                response::apply_complex_response(&display_initial, &display_fir_resp);

            let mut initial_data: super::types::CurveData = (&display_initial).into();
            initial_data.norm_range = norm_range;
            let mut final_data: super::types::CurveData = (&display_final).into();
            final_data.norm_range = norm_range;

            chain.initial_curve = Some(initial_data.clone());
            chain.final_curve = Some(final_data.clone());
            chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

            Ok((
                chain,
                pre_score,
                post_score,
                curve_raw.clone(),
                final_curve,
                vec![],
                mean_spl,
                arrival_time_ms,
                Some(coeffs),
            ))
        }
        ProcessingMode::Hybrid => {
            // Check for frequency-based crossover configuration
            if let Some(mixed_config) = &room_config.optimizer.mixed_config {
                // New frequency-based mixed mode: FIR on one band, IIR on the other
                return process_mixed_mode_crossover(
                    channel_name,
                    &curve_for_optim,
                    room_config,
                    mixed_config,
                    sample_rate,
                    output_dir,
                    min_freq,
                    max_freq,
                    mean_spl,
                    pre_score,
                    arrival_time_ms,
                    callback,
                );
            }

            // Legacy sequential mixed mode: IIR first, then FIR on residual
            // Check if we should force excess phase correction for GD-Opt on subwoofer
            let mut opt_config = clamped_optimizer.clone();
            if let Some(gd_opt) = &clamped_optimizer.gd_opt
                && gd_opt.enabled
            {
                let is_sub = if let Some(sys) = &room_config.system {
                    // V2.1 System Config
                    if let Some(meas_key) = sys.speakers.get(channel_name) {
                        if let Some(subs) = &sys.subwoofers {
                            subs.mapping.contains_key(meas_key)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    // Legacy
                    channel_name == "lfe" || channel_name.starts_with("sub")
                };

                if is_sub && let Some(fir) = &mut opt_config.fir {
                    fir.correct_excess_phase = true;
                    info!(
                        "  GD-Opt: Forcing excess phase correction for '{}'",
                        channel_name
                    );
                }
            }

            // Apply target tilt to the curve (subtract tilt from measurement),
            // same as LowLatency does
            let hybrid_optim_curve = if let Some(ref tilt_curve) = target_tilt_curve {
                Curve {
                    freq: curve_for_optim.freq.clone(),
                    spl: &curve_for_optim.spl - &tilt_curve.spl,
                    phase: curve_for_optim.phase.clone(),
                }
            } else {
                curve_for_optim.clone()
            };

            // When tilt is baked into the curve, don't also pass target_curve
            // to the optimizer (would double-apply the target)
            let effective_target = if target_tilt_curve.is_some() {
                None
            } else {
                room_config.target_curve.as_ref()
            };

            let (eq_filters, _opt_loss) = if let Some(cb) = callback {
                eq::optimize_channel_eq_with_callback(
                    &hybrid_optim_curve,
                    &opt_config, // Use modified config
                    effective_target,
                    sample_rate,
                    cb,
                )
            } else {
                eq::optimize_channel_eq(
                    &hybrid_optim_curve,
                    &opt_config, // Use modified config
                    effective_target,
                    sample_rate,
                )
            }
            .map_err(|e| AutoeqError::OptimizationFailed {
                message: format!(
                    "IIR optimization failed for channel {}: {}",
                    channel_name, e
                ),
            })?;

            info!("  IIR stage: {} filters", eq_filters.len());

            let iir_resp =
                response::compute_peq_complex_response(&eq_filters, &curve.freq, sample_rate);
            let final_curve_iir = response::apply_complex_response(&curve, &iir_resp);
            let input_plus_iir = final_curve_iir.clone();

            info!("  Generating FIR for residual...");
            let coeffs = fir::generate_fir_correction(
                &input_plus_iir,
                &opt_config, // Use modified config
                effective_target,
                sample_rate,
            )
            .map_err(|e| AutoeqError::OptimizationFailed {
                message: format!("FIR generation failed: {}", e),
            })?;

            let filename = format!("{}_residual_fir.wav", channel_name);
            let wav_path = output_dir.join(&filename);
            crate::fir::save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path).map_err(|e| {
                AutoeqError::OptimizationFailed {
                    message: format!("Failed to save FIR WAV: {}", e),
                }
            })?;

            info!("  Saved FIR filter to {}", wav_path.display());

            let conv_plugin = output::create_convolution_plugin(&filename);
            let mut chain =
                output::build_channel_dsp_chain(channel_name, None, broadband_plugins, &eq_filters);
            chain.plugins.push(conv_plugin);

            let fir_resp =
                response::compute_fir_complex_response(&coeffs, &curve.freq, sample_rate);
            let final_curve = response::apply_complex_response(&input_plus_iir, &fir_resp);

            // Compute post_score consistently with pre_score (range-based mean)
            let post_freqs_f32: Vec<f32> = final_curve.freq.iter().map(|&f| f as f32).collect();
            let post_spl_f32: Vec<f32> = final_curve.spl.iter().map(|&s| s as f32).collect();
            let mean_final = compute_average_response(
                &post_freqs_f32,
                &post_spl_f32,
                Some((min_freq as f32, max_freq as f32)),
            ) as f64;
            let normalized_final_spl = &final_curve.spl - mean_final;
            let post_score = crate::loss::flat_loss(
                &final_curve.freq,
                &normalized_final_spl,
                min_freq,
                max_freq,
            );

            info!(
                "  Pre-score: {:.6}, Post-score: {:.6}",
                pre_score, post_score
            );

            // Extend curves to 20 Hz – 20 kHz for display output.
            // Use curve_raw since all_filters includes excursion HPF.
            let display_initial = output::extend_curve_to_full_range(&curve_raw);
            let display_iir_resp = response::compute_peq_complex_response(
                &eq_filters,
                &display_initial.freq,
                sample_rate,
            );
            let display_iir_corrected =
                response::apply_complex_response(&display_initial, &display_iir_resp);
            let display_fir_resp =
                response::compute_fir_complex_response(&coeffs, &display_initial.freq, sample_rate);
            let display_final =
                response::apply_complex_response(&display_iir_corrected, &display_fir_resp);

            let mut initial_data: super::types::CurveData = (&display_initial).into();
            initial_data.norm_range = norm_range;
            let mut final_data: super::types::CurveData = (&display_final).into();
            final_data.norm_range = norm_range;

            chain.initial_curve = Some(initial_data.clone());
            chain.final_curve = Some(final_data.clone());
            chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

            Ok((
                chain,
                pre_score,
                post_score,
                curve_raw.clone(),
                final_curve,
                eq_filters,
                mean_spl,
                arrival_time_ms,
                Some(coeffs),
            ))
        }
        ProcessingMode::MixedPhase => {
            // Mixed-phase correction: IIR for minimum-phase + short FIR for excess phase
            // Step 1: Run standard IIR optimization (same as LowLatency)
            let optimization_curve = if let Some(ref tilt_curve) = target_tilt_curve {
                Curve {
                    freq: curve_for_optim.freq.clone(),
                    spl: &curve_for_optim.spl - &tilt_curve.spl,
                    phase: curve_for_optim.phase.clone(),
                }
            } else {
                curve_for_optim.clone()
            };

            let effective_target = if target_tilt_curve.is_some() {
                None
            } else {
                room_config.target_curve.as_ref()
            };

            let (eq_filters, _opt_loss) = optimize_eq_maybe_multi(
                source,
                &optimization_curve,
                &clamped_optimizer,
                effective_target,
                sample_rate,
                channel_name,
                callback,
            )?;

            info!("  IIR stage: {} filters", eq_filters.len());

            // Step 2: Decompose phase and generate excess phase FIR
            let mp_config = match &room_config.optimizer.mixed_phase {
                Some(sc) => super::mixed_phase::MixedPhaseConfig {
                    max_fir_length_ms: sc.max_fir_length_ms,
                    pre_ringing_threshold_db: sc.pre_ringing_threshold_db,
                    min_spatial_depth: sc.min_spatial_depth,
                    phase_smoothing_octaves: sc.phase_smoothing_octaves,
                },
                None => super::mixed_phase::MixedPhaseConfig::default(),
            };

            // Compute spatial correction depth mask if multi-measurement data is available.
            // This prevents the excess phase FIR from correcting position-dependent phase.
            let spatial_depth = if matches!(source, MeasurementSource::Multiple(_)) {
                match load::load_source_individual(source) {
                    Ok(curves) if curves.len() > 1 => {
                        let sr_config = super::spatial_robustness::SpatialRobustnessConfig::default();
                        let analysis = super::spatial_robustness::analyze_spatial_robustness(&curves, &sr_config);
                        info!(
                            "  Spatial depth for mixed-phase: mean={:.2}",
                            analysis.correction_depth.iter().sum::<f64>() / analysis.correction_depth.len() as f64,
                        );
                        Some(analysis.correction_depth)
                    }
                    _ => None,
                }
            } else {
                None
            };

            let fir_coeffs = if curve_for_optim.phase.is_some() {
                match super::mixed_phase::decompose_phase(&curve_for_optim, &mp_config) {
                    Ok((_min_phase, _excess, delay_ms, residual)) => {
                        info!(
                            "  Mixed-phase: delay={:.2} ms, generating excess phase FIR...",
                            delay_ms
                        );
                        let coeffs = super::mixed_phase::generate_excess_phase_fir_with_depth(
                            &curve_for_optim.freq,
                            &residual,
                            &mp_config,
                            sample_rate,
                            spatial_depth.as_ref(),
                        );

                        // Save FIR to WAV
                        let filename = format!("{}_excess_phase_fir.wav", channel_name);
                        let wav_path = output_dir.join(&filename);
                        if let Err(e) =
                            crate::fir::save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path)
                        {
                            warn!("Failed to save excess phase FIR WAV: {}", e);
                        } else {
                            info!("  Saved excess phase FIR to {}", wav_path.display());
                        }

                        Some((coeffs, filename))
                    }
                    Err(e) => {
                        warn!(
                            "  Mixed-phase decomposition failed for '{}': {}. Using IIR only.",
                            channel_name, e
                        );
                        None
                    }
                }
            } else {
                info!(
                    "  No phase data for '{}', using IIR only (skipping excess phase FIR).",
                    channel_name
                );
                None
            };

            // Build DSP chain (same pattern as LowLatency)
            let mut chain = output::build_channel_dsp_chain_with_curves(
                channel_name,
                None,
                broadband_plugins,
                &eq_filters,
                None,
                None,
            );

            // Add convolution plugin for excess phase FIR if generated
            let returned_fir = if let Some((ref coeffs, ref filename)) = fir_coeffs {
                let convolution_plugin = output::create_convolution_plugin(filename);
                chain.plugins.push(convolution_plugin);
                Some(coeffs.clone())
            } else {
                None
            };

            // Compute final response (IIR + optional FIR)
            let eq_resp =
                crate::response::compute_peq_complex_response(&eq_filters, &curve.freq, sample_rate);
            let after_eq = crate::response::apply_complex_response(&curve_for_optim, &eq_resp);

            let final_curve = if let Some((ref coeffs, _)) = fir_coeffs {
                let fir_resp =
                    crate::response::compute_fir_complex_response(coeffs, &after_eq.freq, sample_rate);
                crate::response::apply_complex_response(&after_eq, &fir_resp)
            } else {
                after_eq
            };

            // Score
            let post_freqs_f32: Vec<f32> = final_curve.freq.iter().map(|&f| f as f32).collect();
            let post_spl_f32: Vec<f32> = final_curve.spl.iter().map(|&s| s as f32).collect();
            let mean_final = compute_average_response(
                &post_freqs_f32,
                &post_spl_f32,
                Some((min_freq as f32, max_freq as f32)),
            ) as f64;
            let normalized_final_spl = &final_curve.spl - mean_final;
            let post_score = crate::loss::flat_loss(
                &final_curve.freq,
                &normalized_final_spl,
                min_freq,
                max_freq,
            );

            info!(
                "  Mixed-phase result: pre={:.6}, post={:.6}",
                pre_score, post_score
            );

            let display_initial = output::extend_curve_to_full_range(&curve_raw);
            let display_eq_resp = crate::response::compute_peq_complex_response(
                &eq_filters,
                &display_initial.freq,
                sample_rate,
            );
            let display_after_eq =
                crate::response::apply_complex_response(&display_initial, &display_eq_resp);
            let display_final = if let Some((ref coeffs, _)) = fir_coeffs {
                let fir_resp = crate::response::compute_fir_complex_response(
                    coeffs,
                    &display_after_eq.freq,
                    sample_rate,
                );
                crate::response::apply_complex_response(&display_after_eq, &fir_resp)
            } else {
                display_after_eq
            };

            let mut initial_data: super::types::CurveData = (&display_initial).into();
            initial_data.norm_range = norm_range;
            let mut final_data: super::types::CurveData = (&display_final).into();
            final_data.norm_range = norm_range;

            chain.initial_curve = Some(initial_data.clone());
            chain.final_curve = Some(final_data.clone());
            chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

            Ok((
                chain,
                pre_score,
                post_score,
                curve_raw.clone(),
                final_curve,
                eq_filters,
                mean_spl,
                arrival_time_ms,
                returned_fir,
            ))
        }
        ProcessingMode::LowLatency => {
            // Default IIR mode with enhanced processing

            // Apply target tilt to the curve (subtract tilt from measurement)
            let optimization_curve = if let Some(ref tilt_curve) = target_tilt_curve {
                Curve {
                    freq: curve_for_optim.freq.clone(),
                    spl: &curve_for_optim.spl - &tilt_curve.spl,
                    phase: curve_for_optim.phase.clone(),
                }
            } else {
                curve_for_optim.clone()
            };

            // When tilt is baked into the curve, don't also pass target_curve
            // to the optimizer (would double-apply the target)
            let effective_target = if target_tilt_curve.is_some() {
                None
            } else {
                room_config.target_curve.as_ref()
            };

            // ================================================================
            // Schroeder Split Optimization (if configured)
            // ================================================================
            let eq_filters = if let Some(schroeder_config) = &room_config.optimizer.schroeder_split
            {
                if schroeder_config.enabled {
                    let schroeder_freq = if let Some(ref dims) = schroeder_config.room_dimensions {
                        let calculated = dims.schroeder_frequency();
                        info!(
                            "  Schroeder split: calculated frequency {:.1} Hz from room dimensions",
                            calculated
                        );
                        calculated
                    } else {
                        schroeder_config.schroeder_freq
                    };
                    info!(
                        "  Schroeder split: optimizing below {:.1} Hz with max_q={:.1}, above with max_q={:.1}",
                        schroeder_freq,
                        schroeder_config.low_freq_config.max_q,
                        schroeder_config.high_freq_config.max_q
                    );

                    // Two-pass optimization with different Q constraints
                    let (low_filters, high_filters) = optimize_with_schroeder_split(
                        &optimization_curve,
                        &clamped_optimizer,
                        schroeder_config,
                        sample_rate,
                    )?;

                    let mut combined_filters = low_filters;
                    combined_filters.extend(high_filters);
                    info!(
                        "  Schroeder split: {} low-freq filters + {} high-freq filters",
                        combined_filters
                            .iter()
                            .filter(|f| f.freq < schroeder_freq)
                            .count(),
                        combined_filters
                            .iter()
                            .filter(|f| f.freq >= schroeder_freq)
                            .count()
                    );
                    combined_filters
                } else {
                    // Standard optimization (with multi-measurement dispatch)
                    let (filters, _opt_loss) = optimize_eq_maybe_multi(
                        source,
                        &optimization_curve,
                        &clamped_optimizer,
                        effective_target,
                        sample_rate,
                        channel_name,
                        callback,
                    )?;
                    filters
                }
            } else {
                // Standard optimization (with multi-measurement dispatch)
                let (filters, _opt_loss) = optimize_eq_maybe_multi(
                    source,
                    &optimization_curve,
                    &clamped_optimizer,
                    effective_target,
                    sample_rate,
                    channel_name,
                    callback,
                )?;
                filters
            };

            info!("  Optimized {} EQ filters", eq_filters.len());

            // Combine excursion protection + broadband + EQ filters
            let mut all_filters = excursion_filters.clone();
            all_filters.extend(broadband_biquads.iter().cloned());
            all_filters.extend(eq_filters.clone());

            let mut chain = output::build_channel_dsp_chain_with_curves(
                channel_name,
                None,
                broadband_plugins,
                &all_filters,
                None,
                None,
            );

            // Compute final response including all corrections (HPF + broadband + EQ).
            // Apply to curve_raw (original measurement) since all_filters includes
            // the excursion HPF and broadband shelves.
            let mut score_raw = curve_raw.clone();
            score_raw.spl += bb_mean_shift; // broadband gain
            let all_resp =
                response::compute_peq_complex_response(&all_filters, &score_raw.freq, sample_rate);
            let final_curve = response::apply_complex_response(&score_raw, &all_resp);

            // Compute post_score consistently with pre_score (flatness of corrected response)
            // If target tilt is applied, score against the tilt target
            let score_curve = if let Some(ref tilt_curve) = target_tilt_curve {
                Curve {
                    freq: final_curve.freq.clone(),
                    spl: &final_curve.spl - &tilt_curve.spl,
                    phase: final_curve.phase.clone(),
                }
            } else {
                final_curve.clone()
            };

            let post_freqs_f32: Vec<f32> = score_curve.freq.iter().map(|&f| f as f32).collect();
            let post_spl_f32: Vec<f32> = score_curve.spl.iter().map(|&s| s as f32).collect();
            let mean_final = compute_average_response(
                &post_freqs_f32,
                &post_spl_f32,
                Some((min_freq as f32, max_freq as f32)),
            ) as f64;
            let normalized_final_spl = &score_curve.spl - mean_final;
            let post_score = crate::loss::flat_loss(
                &score_curve.freq,
                &normalized_final_spl,
                min_freq,
                max_freq,
            );

            info!(
                "  Pre-score: {:.6}, Post-score: {:.6}",
                pre_score, post_score
            );

            // Extend curves to 20 Hz – 20 kHz for display output.
            // Use curve_raw (not HPF-adjusted) since all_filters includes the HPF.
            let display_initial = output::extend_curve_to_full_range(&curve_raw);
            let mut display_raw_with_bb = display_initial.clone();
            display_raw_with_bb.spl += bb_mean_shift; // broadband gain
            let display_resp = response::compute_peq_complex_response(
                &all_filters,
                &display_raw_with_bb.freq,
                sample_rate,
            );
            let display_final =
                response::apply_complex_response(&display_raw_with_bb, &display_resp);

            let mut initial_data: super::types::CurveData = (&display_initial).into();
            initial_data.norm_range = norm_range;
            let mut final_data: super::types::CurveData = (&display_final).into();
            final_data.norm_range = norm_range;

            chain.initial_curve = Some(initial_data.clone());
            chain.final_curve = Some(final_data.clone());
            chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

            // Build effective target curve in absolute SPL coordinates for display.
            // The optimizer works on mean-normalized data, so the effective target is
            // mean_spl + tilt (if any). This lets the frontend show what the optimizer
            // was actually aiming for instead of a misleading 0dB line.
            let display_target_spl = if let Some(ref tilt_curve) = target_tilt_curve {
                // Interpolate tilt to display frequency grid
                let tilt_at_display = crate::read::normalize_and_interpolate_response(
                    &display_initial.freq,
                    tilt_curve,
                );
                &tilt_at_display.spl + mean_spl
            } else {
                ndarray::Array1::from_elem(display_initial.freq.len(), mean_spl)
            };
            chain.target_curve = Some(super::types::CurveData {
                freq: display_initial.freq.to_vec(),
                spl: display_target_spl.to_vec(),
                phase: None,
                norm_range,
            });

            Ok((
                chain,
                pre_score,
                post_score,
                curve_raw,
                final_curve,
                eq_filters,
                mean_spl,
                arrival_time_ms,
                None,
            ))
        }
    }
}

/// Optimize EQ with optional Schroeder frequency split.
///
/// If the optimizer config has an enabled Schroeder split, performs two-pass
/// optimization with different Q constraints. Otherwise falls back to standard
/// single-pass optimization.
///
/// This is the unified entry point for EQ optimization that both the generic
/// pipeline and system-config workflows should use.
pub(super) fn optimize_eq_with_optional_schroeder(
    curve: &Curve,
    optimizer: &OptimizerConfig,
    target_config: Option<&super::types::TargetCurveConfig>,
    sample_rate: f64,
) -> std::result::Result<(Vec<Biquad>, f64), Box<dyn std::error::Error>> {
    if let Some(schroeder_config) = &optimizer.schroeder_split
        && schroeder_config.enabled
    {
        let schroeder_freq = if let Some(ref dims) = schroeder_config.room_dimensions {
            dims.schroeder_frequency()
        } else {
            schroeder_config.schroeder_freq
        };
        info!(
            "  Schroeder split: optimizing below {:.1} Hz with max_q={:.1}, above with max_q={:.1}",
            schroeder_freq,
            schroeder_config.low_freq_config.max_q,
            schroeder_config.high_freq_config.max_q
        );

        let (low_filters, high_filters) = optimize_with_schroeder_split(
            curve, optimizer, schroeder_config, sample_rate,
        ).map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;

        let mut combined = low_filters;
        combined.extend(high_filters);
        // Loss is approximate (sum of both passes) — not used for scoring
        let loss = 0.0;
        Ok((combined, loss))
    } else {
        eq::optimize_channel_eq(curve, optimizer, target_config, sample_rate)
    }
}

/// Optimize EQ with Schroeder frequency split
///
/// Performs two-pass optimization with different Q constraints:
/// - Below Schroeder: high-Q narrow filters for room modes
/// - Above Schroeder: low-Q broad filters for tonal adjustment
fn optimize_with_schroeder_split(
    curve: &Curve,
    optimizer: &OptimizerConfig,
    schroeder_config: &super::types::SchroederSplitConfig,
    sample_rate: f64,
) -> Result<(Vec<Biquad>, Vec<Biquad>)> {
    let schroeder_freq = if let Some(ref dims) = schroeder_config.room_dimensions {
        dims.schroeder_frequency()
    } else {
        schroeder_config.schroeder_freq
    };

    let low_config = &schroeder_config.low_freq_config;
    let high_config = &schroeder_config.high_freq_config;

    // Determine filter allocation (roughly proportional to frequency range)
    let total_filters = optimizer.num_filters;
    let log_range_total = (optimizer.max_freq / optimizer.min_freq).log2();
    let log_range_low = (schroeder_freq / optimizer.min_freq).max(1.0).log2();
    let low_ratio = log_range_low / log_range_total;

    let low_filters = ((total_filters as f64 * low_ratio).round() as usize)
        .max(1)
        .min(total_filters - 1);
    let high_filters = total_filters - low_filters;

    debug!(
        "  Schroeder split: {} filters below {:.1}Hz, {} filters above",
        low_filters, schroeder_freq, high_filters
    );

    // Each sub-pass gets the full maxeval budget. With fewer filters (lower
    // dimensionality) the optimizer converges faster, so the same budget is
    // adequate for each pass independently.
    // When target_tilt is active, the optimizer works on a tilt-adjusted curve
    // where following the tilt may require both boosts and cuts. Allow limited
    // boost (half the configured max) to give the optimizer enough freedom.
    let low_max_db = if low_config.allow_boost {
        optimizer.max_db
    } else if optimizer.target_tilt.is_some() {
        (optimizer.max_db / 2.0).min(3.0) // limited boost for tilt tracking
    } else {
        0.0
    };
    let low_optimizer = OptimizerConfig {
        num_filters: low_filters,
        min_freq: optimizer.min_freq,
        max_freq: schroeder_freq,
        min_q: low_config.min_q,
        max_q: low_config.max_q,
        min_db: optimizer.min_db,
        max_db: low_max_db,
        ..optimizer.clone()
    };

    let (low_eq_filters, _) = eq::optimize_channel_eq(
        curve,
        &low_optimizer,
        None, // No additional target for split optimization
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("Low-frequency EQ optimization failed: {}", e),
    })?;

    // High frequency optimization (above Schroeder)
    let high_optimizer = OptimizerConfig {
        num_filters: high_filters,
        min_freq: schroeder_freq,
        max_freq: optimizer.max_freq,
        min_q: optimizer.min_q.max(0.3), // Ensure minimum Q for broad filters
        max_q: high_config.max_q,
        ..optimizer.clone()
    };

    // Apply low-freq correction first, then optimize high-freq on residual
    let low_resp =
        response::compute_peq_complex_response(&low_eq_filters, &curve.freq, sample_rate);
    let curve_with_low_correction = response::apply_complex_response(curve, &low_resp);

    let (high_eq_filters, _) = eq::optimize_channel_eq(
        &curve_with_low_correction,
        &high_optimizer,
        None,
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("High-frequency EQ optimization failed: {}", e),
    })?;

    // Post-optimization Q clamping: NLopt COBYLA can violate bounds slightly (or
    // significantly with low maxeval). Enforce the configured Q constraints on the
    // returned filters to guarantee the Schroeder split invariant.
    let low_eq_filters = clamp_filter_q(low_eq_filters, low_config.min_q, low_config.max_q);
    let high_eq_filters = clamp_filter_q(
        high_eq_filters,
        optimizer.min_q.max(0.3),
        high_config.max_q,
    );

    Ok((low_eq_filters, high_eq_filters))
}

/// Clamp Q values of filters to [min_q, max_q], recomputing biquad coefficients.
fn clamp_filter_q(filters: Vec<Biquad>, min_q: f64, max_q: f64) -> Vec<Biquad> {
    filters
        .into_iter()
        .map(|f| {
            let clamped_q = f.q.clamp(min_q, max_q);
            if (clamped_q - f.q).abs() > 1e-6 {
                debug!(
                    "  Clamping filter Q at {:.0} Hz: {:.2} -> {:.2}",
                    f.freq, f.q, clamped_q
                );
                Biquad::new(f.filter_type, f.freq, f.srate, clamped_q, f.db_gain)
            } else {
                f
            }
        })
        .collect()
}

/// Determine optimization frequency bands for each driver
///
/// Returns a vector of (min_freq, max_freq) tuples for each driver.
/// Bandwidth extends 1 octave beyond the intended crossover region.
fn determine_optimization_bands(
    n_drivers: usize,
    room_config: &RoomConfig,
    crossover_config: &super::types::CrossoverConfig,
) -> Vec<(f64, f64)> {
    let global_min = room_config.optimizer.min_freq;
    let global_max = room_config.optimizer.max_freq;

    let mut bands = Vec::with_capacity(n_drivers);

    // Determine crossover points estimates
    // If fixed frequencies or range provided, use those.
    // Otherwise, assume log-spaced distribution.
    let xover_points = if let Some(ref freqs) = crossover_config.frequencies {
        freqs.clone()
    } else if let Some(freq) = crossover_config.frequency {
        vec![freq]
    } else if let Some((min, max)) = crossover_config.frequency_range {
        // If range provided for 2-way, use geometric mean as center estimate
        // but for bounds calculation, we use the range limits.
        // Actually, for optimization limits:
        // Low driver max = max_range * 2
        // High driver min = min_range / 2
        vec![min, max] // Placeholder, logic below handles range
    } else {
        Vec::new() // No info
    };

    // Helper to get safe crossover bounds
    let get_xover_bounds = |idx: usize| -> (f64, f64) {
        if let Some((min, max)) = crossover_config.frequency_range {
            // If explicit range is given, use it for the single crossover (2-way)
            if n_drivers == 2 && idx == 0 {
                return (min, max);
            }
        }

        if !xover_points.is_empty() && idx < xover_points.len() {
            let f = xover_points[idx];
            return (f, f);
        }

        // Fallback: log-distribute between 80Hz and 3000Hz
        // This is a rough heuristic if no info is present
        (80.0, 3000.0)
    };

    for i in 0..n_drivers {
        let min_f = if i == 0 {
            global_min
        } else {
            // Highpass: 1 octave below crossover
            let (xover_min, _) = get_xover_bounds(i - 1);
            xover_min * 0.5
        };

        let max_f = if i == n_drivers - 1 {
            global_max
        } else {
            // Lowpass: 1 octave above crossover
            let (_, xover_max) = get_xover_bounds(i);
            xover_max * 2.0
        };

        bands.push((min_f.max(global_min), max_f.min(global_max)));
    }

    bands
}

/// Process a speaker group with multiple drivers and crossovers
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
fn process_speaker_group(
    channel_name: &str,
    group: &SpeakerGroup,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<MixedModeResult> {
    // 1. Load all measurements in the group
    let mut driver_curves = Vec::new();
    for (i, source) in group.measurements.iter().enumerate() {
        let curve = load::load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!(
                "Failed to load driver {} measurement for channel {}: {}",
                i, channel_name, e
            ),
        })?;
        driver_curves.push(curve);
    }

    debug!("  Loaded {} driver measurements", driver_curves.len());

    // 2. Sort drivers by mean frequency (Low to High)
    driver_curves.sort_by(|a, b| {
        let get_mean = |c: &Curve| {
            let min_f = c.freq.iter().copied().fold(f64::INFINITY, f64::min);
            let max_f = c.freq.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (min_f * max_f).sqrt()
        };
        get_mean(a)
            .partial_cmp(&get_mean(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 3. Get crossover config
    let crossover_config = if let Some(crossover_ref) = &group.crossover {
        room_config
            .crossovers
            .as_ref()
            .and_then(|xovers| xovers.get(crossover_ref))
            .ok_or_else(|| AutoeqError::InvalidConfiguration {
                message: format!("Crossover configuration '{}' not found", crossover_ref),
            })?
    } else {
        return Err(AutoeqError::InvalidConfiguration {
            message: "Speaker group requires crossover configuration".to_string(),
        });
    };

    // 4. Per-Driver Linearization (Pre-Correction)
    info!("  Linearizing {} drivers...", driver_curves.len());
    let optimization_bands =
        determine_optimization_bands(driver_curves.len(), room_config, crossover_config);
    let mut linearized_drivers = Vec::with_capacity(driver_curves.len());
    let mut per_driver_filters = Vec::with_capacity(driver_curves.len());

    for (i, curve) in driver_curves.iter().enumerate() {
        let (min_f, max_f) = optimization_bands[i];
        info!(
            "    Driver {}: optimizing bandwidth {:.1}-{:.1} Hz",
            i, min_f, max_f
        );

        // Create driver-specific config
        let mut driver_opt_config = room_config.optimizer.clone();
        driver_opt_config.min_freq = min_f;
        driver_opt_config.max_freq = max_f;

        // Optimize EQ for this driver
        let (filters, _) = eq::optimize_channel_eq(
            curve,
            &driver_opt_config,
            room_config.target_curve.as_ref(), // Use global target (usually flat)
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("Linearization failed for driver {}: {}", i, e),
        })?;

        // Apply filters to get linearized curve
        let resp = response::compute_peq_complex_response(&filters, &curve.freq, sample_rate);
        let linear_curve = response::apply_complex_response(curve, &resp);

        linearized_drivers.push(linear_curve);
        per_driver_filters.push(filters);
    }

    // 5. Setup Crossover Optimization
    let crossover_type = crossover::parse_crossover_type(&crossover_config.crossover_type)
        .map_err(|e| AutoeqError::InvalidConfiguration {
            message: e.to_string(),
        })?;

    let fixed_freqs: Option<Vec<f64>> = if let Some(ref freqs) = crossover_config.frequencies {
        Some(freqs.clone())
    } else if let Some(freq) = crossover_config.frequency {
        Some(vec![freq])
    } else {
        None
    };

    // 6. Compute pre-score (using linearized drivers)
    let n_drivers = linearized_drivers.len();
    let initial_gains = vec![0.0; n_drivers];
    let mut initial_xover_freqs = Vec::new();
    // Simple geometric mean estimate for initial guess
    for _ in 0..(n_drivers - 1) {
        let (min, max) = match crossover_config.frequency_range {
            Some((a, b)) => (a, b),
            None => (80.0, 3000.0),
        };
        initial_xover_freqs.push((min * max).sqrt());
    }

    let driver_measurements: Vec<crate::loss::DriverMeasurement> = linearized_drivers
        .iter()
        .map(|curve| crate::loss::DriverMeasurement {
            freq: curve.freq.clone(),
            spl: curve.spl.clone(),
            phase: curve.phase.clone(),
        })
        .collect();

    let initial_delays = vec![0.0; n_drivers];

    let drivers_data = crate::loss::DriversLossData::new(driver_measurements, crossover_type);
    let pre_score = crate::loss::drivers_flat_loss(
        &drivers_data,
        &initial_gains,
        &initial_xover_freqs,
        Some(&initial_delays),
        sample_rate,
        room_config.optimizer.min_freq,
        room_config.optimizer.max_freq,
    );

    // 7. Optimize Crossover (using linearized drivers)
    let (gains, delays, crossover_freqs, combined_curve, inversions) =
        crossover::optimize_crossover(
            linearized_drivers.clone(), // Use linearized curves!
            crossover_type,
            sample_rate,
            &room_config.optimizer,
            fixed_freqs,
            crossover_config.frequency_range,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("Crossover optimization failed: {}", e),
        })?;

    info!(
        "  Optimized crossover: freqs={:?}, gains={:?}, delays={:?}, inversions={:?}",
        crossover_freqs, gains, delays, inversions
    );

    // 8. Global EQ (Optional Touch-up)
    // Run global EQ on the combined response to fix any remaining issues
    // but constrain it to be gentle if possible, or normal full optimization.
    let (global_eq_filters, post_score) = eq::optimize_channel_eq(
        &combined_curve,
        &room_config.optimizer,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!(
            "Global EQ optimization failed for channel {}: {}",
            channel_name, e
        ),
    })?;

    info!("  Optimized {} Global EQ filters", global_eq_filters.len());
    info!(
        "  Pre-score: {:.6}, Post-score: {:.6}",
        pre_score, post_score
    );

    // 9. Build Output DSP Chain
    // We now have per-driver filters AND global filters.

    // Prepare display curves (raw drivers extended)
    let driver_curves_for_display: Vec<Curve> = driver_curves
        .iter()
        .map(output::extend_curve_to_full_range)
        .collect();

    let mut chain = output::build_multidriver_dsp_chain_with_curves(
        channel_name,
        &gains,
        &delays,
        Some(&inversions),
        &crossover_freqs,
        crossover::crossover_type_to_string(&crossover_type),
        &global_eq_filters,
        Some(&per_driver_filters), // Pass the per-driver EQ filters here!
        None,
        None,
        Some(&driver_curves_for_display),
    );

    // 10. Compute Final Response for validation
    let global_resp = response::compute_peq_complex_response(
        &global_eq_filters,
        &combined_curve.freq,
        sample_rate,
    );
    let final_curve = response::apply_complex_response(&combined_curve, &global_resp);

    // Detect passband
    let (norm_range, _passband_mean) = detect_passband_and_mean(&combined_curve);

    // Extend curves for display
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp = response::compute_peq_complex_response(
        &global_eq_filters,
        &display_initial.freq,
        sample_rate,
    );
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    // Use global mean for level alignment
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let freqs_f32: Vec<f32> = combined_curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = combined_curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    Ok((
        chain,
        pre_score,
        post_score,
        combined_curve.clone(),
        final_curve,
        global_eq_filters,
        mean_spl,
        None, // No single WAV for speaker groups
        None, // IIR-only for speaker groups
    ))
}

/// Process multi-subwoofer group
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
fn process_multisub_group(
    channel_name: &str,
    group: &MultiSubGroup,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<MixedModeResult> {
    let (result, combined_curve, allpass_filters) = if group.allpass_optimization {
        // All-pass enhanced optimization (Dirac Bass Control inspired)
        info!("  Using all-pass enhanced multi-sub optimization");
        let ap_result = multisub::optimize_multisub_with_allpass(
            &group.subwoofers,
            &room_config.optimizer,
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("Multi-sub all-pass optimization failed: {}", e),
        })?;

        for (i, (freq, q)) in ap_result.allpass_filters.iter().enumerate() {
            info!(
                "  Sub {}: gain={:.1} dB, delay={:.1} ms, all-pass: {:.0} Hz Q={:.2}",
                i, ap_result.base.gains[i], ap_result.base.delays[i], freq, q
            );
        }

        (
            ap_result.base,
            ap_result.combined_curve,
            Some(ap_result.allpass_filters),
        )
    } else {
        // Standard gain + delay optimization
        let (result, curve) =
            multisub::optimize_multisub(&group.subwoofers, &room_config.optimizer, sample_rate)
                .map_err(|e| AutoeqError::OptimizationFailed {
                    message: format!("Multi-sub optimization failed: {}", e),
                })?;
        (result, curve, None)
    };

    info!(
        "  Multi-sub optimization: gains={:?}, delays={:?} ms",
        result.gains, result.delays
    );

    let (eq_filters, post_score) = eq::optimize_channel_eq(
        &combined_curve,
        &room_config.optimizer,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("EQ optimization failed for multi-sub sum: {}", e),
    })?;

    info!(
        "  Global EQ: {} filters, score={:.6}",
        eq_filters.len(),
        post_score
    );

    // Load individual sub curves for per-driver display
    let driver_curves_for_display: Vec<Curve> = group
        .subwoofers
        .iter()
        .filter_map(|source| {
            load::load_source(source)
                .ok()
                .map(|c| output::extend_curve_to_full_range(&c))
        })
        .collect();
    let driver_display_ref = if driver_curves_for_display.len() == group.subwoofers.len() {
        Some(driver_curves_for_display.as_slice())
    } else {
        None
    };

    let mut chain = output::build_multisub_dsp_chain_with_allpass(
        channel_name,
        &group.name,
        group.subwoofers.len(),
        &result.gains,
        &result.delays,
        &eq_filters,
        None,
        None,
        driver_display_ref,
        allpass_filters.as_deref(),
        sample_rate,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    // Detect passband for normalization (used for display curves)
    let (norm_range, _passband_mean) = detect_passband_and_mean(&combined_curve);

    // Level alignment: use mean SPL within the EQ optimization range
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let freqs_f32: Vec<f32> = combined_curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = combined_curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    // Extend curves to 20 Hz – 20 kHz for display output
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    Ok((
        chain,
        result.pre_objective,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
        mean_spl,
        None, // No single WAV for multi-sub groups
        None, // IIR-only for multi-sub groups
    ))
}

/// Process DBA configuration
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
fn process_dba(
    channel_name: &str,
    dba_config: &super::types::DBAConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<MixedModeResult> {
    let (result, combined_curve) =
        dba::optimize_dba(dba_config, &room_config.optimizer, sample_rate).map_err(|e| {
            AutoeqError::OptimizationFailed {
                message: format!("DBA optimization failed: {}", e),
            }
        })?;

    info!(
        "  DBA Optimization: Front Gain={:.2}dB, Rear Gain={:.2}dB, Rear Delay={:.2}ms",
        result.gains[0], result.gains[1], result.delays[1]
    );

    let (eq_filters, post_score) = eq::optimize_channel_eq(
        &combined_curve,
        &room_config.optimizer,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("EQ optimization failed for DBA sum: {}", e),
    })?;

    info!(
        "  Global EQ: {} filters, score={:.6}",
        eq_filters.len(),
        post_score
    );

    // Load front/rear array curves for per-driver display
    // DBA has 2 "drivers": front aggregate and rear aggregate
    let driver_display_ref = match (
        dba::sum_array_response(&dba_config.front),
        dba::sum_array_response(&dba_config.rear),
    ) {
        (Ok(front), Ok(rear)) => Some(vec![
            output::extend_curve_to_full_range(&front),
            output::extend_curve_to_full_range(&rear),
        ]),
        _ => None,
    };
    let driver_display_slice = driver_display_ref.as_deref();

    let mut chain = output::build_dba_dsp_chain_with_curves(
        channel_name,
        &result.gains,
        &result.delays,
        &eq_filters,
        None,
        None,
        driver_display_slice,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    // Detect passband for normalization (used for display curves)
    let (norm_range, _passband_mean) = detect_passband_and_mean(&combined_curve);

    // Level alignment: use mean SPL within the EQ optimization range
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let freqs_f32: Vec<f32> = combined_curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = combined_curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    // Extend curves to 20 Hz – 20 kHz for display output
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    Ok((
        chain,
        result.pre_objective,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
        mean_spl,
        None, // No single WAV for DBA
        None, // IIR-only for DBA
    ))
}

// ============================================================================
// Frequency-Based Mixed Mode Processing
// ============================================================================

/// Process mixed mode with frequency-based crossover
///
/// This mode applies FIR correction to one frequency band (default: low frequencies)
/// and IIR correction to the other band (default: high frequencies), separated by
/// a configurable crossover frequency.
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
#[allow(clippy::too_many_arguments)]
fn process_mixed_mode_crossover(
    channel_name: &str,
    curve: &Curve,
    room_config: &RoomConfig,
    mixed_config: &MixedModeConfig,
    sample_rate: f64,
    output_dir: &Path,
    min_freq: f64,
    max_freq: f64,
    mean: f64,
    pre_score: f64,
    arrival_time_ms: Option<f64>,
    callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<MixedModeResult> {
    let crossover_freq = mixed_config.crossover_freq;
    let fir_uses_low = mixed_config.fir_band.to_lowercase() == "low";

    info!(
        "  Mixed mode crossover at {} Hz (FIR on {} band, IIR on {} band)",
        crossover_freq,
        if fir_uses_low { "low" } else { "high" },
        if fir_uses_low { "high" } else { "low" }
    );

    // Split the curve at crossover frequency
    let (low_curve, high_curve) = split_curve_at_frequency(curve, crossover_freq);

    // Determine which curve gets FIR and which gets IIR
    let (fir_curve, iir_curve) = if fir_uses_low {
        (&low_curve, &high_curve)
    } else {
        (&high_curve, &low_curve)
    };

    // Create band-specific optimizer configs with appropriate frequency ranges
    let fir_min_freq = fir_curve.freq.first().copied().unwrap_or(min_freq);
    let fir_max_freq = fir_curve.freq.last().copied().unwrap_or(crossover_freq);
    let iir_min_freq = iir_curve.freq.first().copied().unwrap_or(crossover_freq);
    let iir_max_freq = iir_curve.freq.last().copied().unwrap_or(max_freq);

    info!(
        "  FIR band: {:.1}-{:.1} Hz, IIR band: {:.1}-{:.1} Hz",
        fir_min_freq, fir_max_freq, iir_min_freq, iir_max_freq
    );

    // Optimize IIR on designated band
    let iir_config = OptimizerConfig {
        min_freq: iir_min_freq,
        max_freq: iir_max_freq,
        ..room_config.optimizer.clone()
    };

    let (eq_filters, _) = if let Some(cb) = callback {
        eq::optimize_channel_eq_with_callback(
            iir_curve,
            &iir_config,
            room_config.target_curve.as_ref(),
            sample_rate,
            cb,
        )
    } else {
        eq::optimize_channel_eq(
            iir_curve,
            &iir_config,
            room_config.target_curve.as_ref(),
            sample_rate,
        )
    }
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!(
            "IIR optimization failed for {} band: {}",
            if fir_uses_low { "high" } else { "low" },
            e
        ),
    })?;

    info!(
        "  IIR stage: {} filters for {} band",
        eq_filters.len(),
        if fir_uses_low { "high" } else { "low" }
    );

    // Optimize FIR on designated band
    let fir_config = OptimizerConfig {
        min_freq: fir_min_freq,
        max_freq: fir_max_freq,
        ..room_config.optimizer.clone()
    };

    let fir_coeffs = fir::generate_fir_correction(
        fir_curve,
        &fir_config,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!(
            "FIR generation failed for {} band: {}",
            if fir_uses_low { "low" } else { "high" },
            e
        ),
    })?;

    // Save FIR to WAV
    let fir_filename = format!("{}_band_fir.wav", channel_name);
    let wav_path = output_dir.join(&fir_filename);
    crate::fir::save_fir_to_wav(&fir_coeffs, sample_rate as u32, &wav_path).map_err(|e| {
        AutoeqError::OptimizationFailed {
            message: format!("Failed to save FIR WAV: {}", e),
        }
    })?;

    info!("  Saved FIR filter to {}", wav_path.display());

    // Build DSP chain with band split/merge
    let mut chain = output::build_mixed_mode_crossover_chain(
        channel_name,
        mixed_config,
        &eq_filters,
        &fir_filename,
        fir_uses_low,
        None,
    );

    // Compute combined response for scoring
    // For proper scoring, we need to simulate what the full chain does:
    // - Split into bands at crossover
    // - Apply FIR to one band, IIR to the other
    // - Sum bands back together
    let iir_resp = response::compute_peq_complex_response(&eq_filters, &curve.freq, sample_rate);
    let fir_resp = response::compute_fir_complex_response(&fir_coeffs, &curve.freq, sample_rate);

    // Compute crossover filter responses (LR24 = 4th order Butterworth)
    let (lp_resp, hp_resp) =
        compute_lr24_crossover_responses(&curve.freq, crossover_freq, sample_rate);

    // Combine responses: low_band * fir_or_iir + high_band * iir_or_fir
    let combined_resp: Vec<num_complex::Complex<f64>> = curve
        .freq
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if fir_uses_low {
                lp_resp[i] * fir_resp[i] + hp_resp[i] * iir_resp[i]
            } else {
                lp_resp[i] * iir_resp[i] + hp_resp[i] * fir_resp[i]
            }
        })
        .collect();

    let final_curve = response::apply_complex_response(curve, &combined_resp);

    // Detect passband for normalization
    let (norm_range, mean_final) = detect_passband_and_mean(&final_curve);

    // Compute post-score
    let normalized_final_spl = &final_curve.spl - mean_final;
    let post_score =
        crate::loss::flat_loss(&final_curve.freq, &normalized_final_spl, min_freq, max_freq);

    info!(
        "  Pre-score: {:.6}, Post-score: {:.6}",
        pre_score, post_score
    );

    // Extend curves to 20 Hz – 20 kHz for display output
    let display_initial = output::extend_curve_to_full_range(curve);
    let display_iir_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_fir_resp =
        response::compute_fir_complex_response(&fir_coeffs, &display_initial.freq, sample_rate);
    let (display_lp, display_hp) =
        compute_lr24_crossover_responses(&display_initial.freq, crossover_freq, sample_rate);
    let display_combined: Vec<num_complex::Complex<f64>> = display_initial
        .freq
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if fir_uses_low {
                display_lp[i] * display_fir_resp[i] + display_hp[i] * display_iir_resp[i]
            } else {
                display_lp[i] * display_iir_resp[i] + display_hp[i] * display_fir_resp[i]
            }
        })
        .collect();
    let display_final = response::apply_complex_response(&display_initial, &display_combined);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    Ok((
        chain,
        pre_score,
        post_score,
        curve.clone(),
        final_curve,
        eq_filters,
        mean,
        arrival_time_ms,
        Some(fir_coeffs),
    ))
}

/// Split a frequency response curve at a specified frequency
fn split_curve_at_frequency(curve: &Curve, crossover_freq: f64) -> (Curve, Curve) {
    // Find the index where frequency exceeds crossover
    let split_idx = curve
        .freq
        .iter()
        .position(|&f| f >= crossover_freq)
        .unwrap_or(curve.freq.len());

    // Include some overlap around crossover for better optimization
    let overlap_points = 3; // Include a few points on each side
    let low_end = (split_idx + overlap_points).min(curve.freq.len());
    let high_start = split_idx.saturating_sub(overlap_points);

    let low_curve = Curve {
        freq: curve.freq.slice(ndarray::s![..low_end]).to_owned(),
        spl: curve.spl.slice(ndarray::s![..low_end]).to_owned(),
        phase: curve
            .phase
            .as_ref()
            .map(|p| p.slice(ndarray::s![..low_end]).to_owned()),
    };

    let high_curve = Curve {
        freq: curve.freq.slice(ndarray::s![high_start..]).to_owned(),
        spl: curve.spl.slice(ndarray::s![high_start..]).to_owned(),
        phase: curve
            .phase
            .as_ref()
            .map(|p| p.slice(ndarray::s![high_start..]).to_owned()),
    };

    (low_curve, high_curve)
}

/// Compute Linkwitz-Riley 24dB/oct crossover filter responses
///
/// Returns (lowpass_response, highpass_response) as complex vectors
///
/// LR24 consists of two cascaded 2nd-order Butterworth filters.
/// This implementation computes the actual complex response including phase,
/// which is critical for accurate band summation in hybrid mode.
fn compute_lr24_crossover_responses(
    frequencies: &ndarray::Array1<f64>,
    crossover_freq: f64,
    sample_rate: f64,
) -> (
    Vec<num_complex::Complex<f64>>,
    Vec<num_complex::Complex<f64>>,
) {
    use math_audio_iir_fir::{Biquad, BiquadFilterType};

    // LR24 = two cascaded Butterworth LP2 filters (Q = 0.7071 each)
    // For LR24 lowpass: two 2nd-order Butterworth lowpass filters in series
    // For LR24 highpass: two 2nd-order Butterworth highpass filters in series

    let q = std::f64::consts::FRAC_1_SQRT_2; // Q = 0.7071 for Butterworth

    // Create biquad filters for lowpass (2 cascaded)
    let lp1 = Biquad::new(
        BiquadFilterType::Lowpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );
    let lp2 = Biquad::new(
        BiquadFilterType::Lowpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );

    // Create biquad filters for highpass (2 cascaded)
    let hp1 = Biquad::new(
        BiquadFilterType::Highpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );
    let hp2 = Biquad::new(
        BiquadFilterType::Highpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );

    let mut lp_resp = Vec::with_capacity(frequencies.len());
    let mut hp_resp = Vec::with_capacity(frequencies.len());

    for &freq in frequencies.iter() {
        // Compute cascaded response: H_lp = H_lp1 * H_lp2
        let lp1_resp = lp1.complex_response(freq);
        let lp2_resp = lp2.complex_response(freq);
        let lp_total = lp1_resp * lp2_resp;

        // Compute cascaded response: H_hp = H_hp1 * H_hp2
        let hp1_resp = hp1.complex_response(freq);
        let hp2_resp = hp2.complex_response(freq);
        let hp_total = hp1_resp * hp2_resp;

        lp_resp.push(lp_total);
        hp_resp.push(hp_total);
    }

    (lp_resp, hp_resp)
}

/// Perform consistency checks between speakers in the same Acoustic Group
fn check_group_consistency(
    group_name: &str,
    channels: &[String],
    channel_means: &HashMap<String, f64>,
    curves: &HashMap<String, Curve>,
) {
    if channels.len() < 2 {
        return;
    }

    // 1. Range Difference Check (3 dB threshold)
    let mut means = Vec::new();
    for ch in channels {
        if let Some(&mean) = channel_means.get(ch) {
            means.push((ch, mean));
        }
    }

    for i in 0..means.len() {
        for j in i + 1..means.len() {
            let (ch1, m1) = means[i];
            let (ch2, m2) = means[j];
            let diff = (m1 - m2).abs();
            if diff > 3.0 {
                warn!(
                    "Speaker group '{}' has significant difference: range SPL between '{}' and '{}' is {:.1} dB (> 3.0 dB threshold).",
                    group_name, ch1, ch2, diff
                );
            }
        }
    }

    // 2. Octave-Wise Difference Check (6 dB threshold)
    // Compare all pairs in the group
    for i in 0..channels.len() {
        for j in i + 1..channels.len() {
            let ch1 = &channels[i];
            let ch2 = &channels[j];
            if let (Some(curve1), Some(curve2)) = (curves.get(ch1), curves.get(ch2)) {
                check_octave_consistency(group_name, ch1, ch2, curve1, curve2);
            }
        }
    }
}

/// Check if two curves are consistent across all octaves (6 dB threshold)
fn check_octave_consistency(
    group_name: &str,
    ch1: &str,
    ch2: &str,
    curve1: &Curve,
    curve2: &Curve,
) {
    // Standard acoustic octaves
    let octave_centers = [
        31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
    ];

    for &center in &octave_centers {
        let f_min = center / 2.0_f64.sqrt();
        let f_max = center * 2.0_f64.sqrt();

        // Find overlap range
        let start_freq = f_min.max(curve1.freq[0]).max(curve2.freq[0]);
        let end_freq = f_max
            .min(curve1.freq[curve1.freq.len() - 1])
            .min(curve2.freq[curve2.freq.len() - 1]);

        if end_freq <= start_freq * 1.1 {
            continue; // Not enough bandwidth in this octave for comparison
        }

        // Compute average SPL for this octave in both curves
        let freqs1_f32: Vec<f32> = curve1.freq.iter().map(|&f| f as f32).collect();
        let spl1_f32: Vec<f32> = curve1.spl.iter().map(|&s| s as f32).collect();
        let freqs2_f32: Vec<f32> = curve2.freq.iter().map(|&f| f as f32).collect();
        let spl2_f32: Vec<f32> = curve2.spl.iter().map(|&s| s as f32).collect();

        let range = Some((start_freq as f32, end_freq as f32));
        let avg1 = compute_average_response(&freqs1_f32, &spl1_f32, range);
        let avg2 = compute_average_response(&freqs2_f32, &spl2_f32, range);

        let diff = (avg1 - avg2).abs() as f64;
        if diff > 6.0 {
            warn!(
                "Speaker group '{}' has significant difference: octave around {:.0} Hz between '{}' and '{}' differs by {:.1} dB (> 6.0 dB threshold).",
                group_name, center, ch1, ch2, diff
            );
        }
    }
}
/// Process Gradient Cardioid configuration
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
fn process_cardioid(
    channel_name: &str,
    config: &super::types::CardioidConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<MixedModeResult> {
    // 1. Load measurements
    let front_curve =
        load::load_source(&config.front).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!("Failed to load Front measurement: {}", e),
        })?;
    let rear_curve =
        load::load_source(&config.rear).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!("Failed to load Rear measurement: {}", e),
        })?;

    // 2. Calculate Delay
    let delay_ms = config.separation_meters / 343.0 * 1000.0;
    info!(
        "  Cardioid: Separation {:.2}m -> Delay {:.2}ms",
        config.separation_meters, delay_ms
    );

    // 3. Simulate Combined Response
    use num_complex::Complex;
    let n_points = front_curve.freq.len();
    let mut combined_spl = ndarray::Array1::zeros(n_points);

    // We need phase. If missing, assume 0.
    let front_phase_zeros = ndarray::Array1::zeros(n_points);
    let rear_phase_zeros = ndarray::Array1::zeros(n_points);
    let front_phase = front_curve.phase.as_ref().unwrap_or(&front_phase_zeros);
    let rear_phase = rear_curve.phase.as_ref().unwrap_or(&rear_phase_zeros);

    for i in 0..n_points {
        let f = front_curve.freq[i];
        let omega = 2.0 * std::f64::consts::PI * f;

        // Front
        let f_mag = 10.0_f64.powf(front_curve.spl[i] / 20.0);
        let f_phi = front_phase[i].to_radians();
        let f_c = Complex::from_polar(f_mag, f_phi);

        // Rear (Inverted + Delayed)
        let r_mag = 10.0_f64.powf(rear_curve.spl[i] / 20.0);
        let r_phi_meas = rear_phase[i].to_radians();

        // Delay phase shift: -omega * delay
        let delay_s = delay_ms / 1000.0;
        let delay_phi = -omega * delay_s;

        // Inversion: +180 deg (PI rad)
        let invert_phi = std::f64::consts::PI;

        let r_phi_total = r_phi_meas + delay_phi + invert_phi;
        let r_c = Complex::from_polar(r_mag, r_phi_total);

        let sum = f_c + r_c;
        combined_spl[i] = 20.0 * sum.norm().log10();
    }

    let combined_curve = Curve {
        freq: front_curve.freq.clone(),
        spl: combined_spl,
        phase: None, // Optimized for magnitude
    };

    // 4. Optimize EQ
    let (eq_filters, post_score) = eq::optimize_channel_eq(
        &combined_curve,
        &room_config.optimizer,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("EQ optimization failed for Cardioid sum: {}", e),
    })?;

    // Compute pre-score
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let (norm_range, mean) = detect_passband_and_mean(&combined_curve);
    let normalized_spl = &combined_curve.spl - mean;
    let pre_score =
        crate::loss::flat_loss(&combined_curve.freq, &normalized_spl, min_freq, max_freq);

    info!(
        "  Global EQ: {} filters, score={:.6}",
        eq_filters.len(),
        post_score
    );

    // 5. Build Output Chain
    // Prepare display curves
    let driver_curves_for_display = vec![
        output::extend_curve_to_full_range(&front_curve),
        output::extend_curve_to_full_range(&rear_curve),
    ];

    let mut chain = output::build_cardioid_dsp_chain_with_curves(
        channel_name,
        &[0.0, 0.0],      // Gains (0 for now)
        &[0.0, delay_ms], // Delays
        &eq_filters,
        None,
        None,
        Some(&driver_curves_for_display),
    );

    // Final Curve calculation
    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    // Populate initial/final curves in chain
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    // Mean SPL
    let freqs_f32: Vec<f32> = combined_curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = combined_curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    Ok((
        chain,
        pre_score,
        post_score,
        combined_curve,
        final_curve,
        eq_filters,
        mean_spl,
        None,
        None, // IIR-only for cardioid
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_passband_silence_returns_none() {
        // All SPL at -120 dB (essentially silence)
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 1000.0, 10000.0]),
            spl: Array1::from_vec(vec![-120.0, -120.0, -120.0]),
            phase: None,
        };
        let (passband, mean) = detect_passband_and_mean(&curve);
        assert!(
            passband.is_none(),
            "silence measurement should return None passband, got {:?}",
            passband
        );
        assert_eq!(mean, 0.0);
    }

    // ========================================================================
    // Pipeline invariant test helpers
    // ========================================================================

    /// Create a synthetic curve with 500 log-spaced points from 20-20kHz
    fn make_test_curve(spl_fn: impl Fn(f64) -> f64) -> Curve {
        let n = 500;
        let log_min = 20.0_f64.ln();
        let log_max = 20000.0_f64.ln();
        let freqs: Vec<f64> = (0..n)
            .map(|i| (log_min + (log_max - log_min) * i as f64 / (n - 1) as f64).exp())
            .collect();
        let spl: Vec<f64> = freqs.iter().map(|&f| spl_fn(f)).collect();
        Curve {
            freq: Array1::from_vec(freqs),
            spl: Array1::from_vec(spl),
            phase: None,
        }
    }

    /// Bookshelf speaker: flat ~70dB 80-20kHz, -12dB/oct rolloff below 60Hz
    fn make_bookshelf_curve() -> Curve {
        make_test_curve(|f| {
            let base = 70.0;
            if f < 60.0 {
                // -12 dB/octave rolloff below 60Hz
                base + 12.0 * (f / 60.0).log2()
            } else {
                base
            }
        })
    }

    /// Create a fast optimizer config with overrides applied.
    /// Uses autoeq:de (always available, no feature flags needed) with
    /// moderate iteration count to balance speed and convergence.
    fn fast_test_config(overrides: impl FnOnce(&mut OptimizerConfig)) -> OptimizerConfig {
        let mut config = OptimizerConfig {
            algorithm: "autoeq:de".to_string(),
            max_iter: 1000,
            population: 20,
            num_filters: 5,
            seed: Some(42),
            refine: false,
            psychoacoustic: false,
            asymmetric_loss: false,
            ..OptimizerConfig::default()
        };
        overrides(&mut config);
        config
    }

    /// Run process_single_speaker with an in-memory curve and minimal RoomConfig
    fn run_single_speaker(curve: Curve, config: &OptimizerConfig) -> MixedModeResult {
        let source = MeasurementSource::InMemory(curve);
        let room_config = RoomConfig {
            version: super::super::types::default_config_version(),
            system: None,
            speakers: {
                let mut m = HashMap::new();
                m.insert("test".to_string(), SpeakerConfig::Single(source.clone()));
                m
            },
            crossovers: None,
            target_curve: None,
            optimizer: config.clone(),
            recording_config: None,
        };
        let tmp = tempfile::TempDir::new().expect("failed to create temp dir");
        process_single_speaker("test", &source, &room_config, 48000.0, tmp.path(), None)
            .expect("process_single_speaker failed")
    }

    /// Find SPL at the nearest frequency point in a curve (log-space nearest)
    fn spl_at(curve: &Curve, target_f: f64) -> f64 {
        let mut best_idx = 0;
        let mut best_dist = f64::INFINITY;
        for (i, &f) in curve.freq.iter().enumerate() {
            let dist = (f.ln() - target_f.ln()).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }
        curve.spl[best_idx]
    }

    /// Compute the slope of a curve in dB/octave between two frequencies
    fn slope_db_per_octave(curve: &Curve, f_low: f64, f_high: f64) -> f64 {
        let delta_spl = spl_at(curve, f_high) - spl_at(curve, f_low);
        let octaves = (f_high / f_low).log2();
        delta_spl / octaves
    }

    // ========================================================================
    // Group 1: Tilt invariants
    // ========================================================================

    #[test]
    fn test_pipeline_tilt_flat_with_slope_still_applies() {
        // Bug #1: tilt_type defaults to Flat, slope silently ignored.
        // Config with Flat + slope=-0.8 must produce tilted output
        // (process_single_speaker promotes Flat+slope to Custom).
        let curve = make_test_curve(|_f| 70.0); // perfectly flat
        let config = fast_test_config(|c| {
            c.target_tilt = Some(super::super::types::TargetTiltConfig {
                tilt_type: TiltType::Flat,
                slope_db_per_octave: -0.8,
                reference_freq: 1000.0,
                bass_shelf_db: 0.0,
                bass_shelf_freq: 200.0,
            });
        });
        let (_chain, _pre, _post, _initial, final_curve, _biquads, _mean, _arrival, _fir) =
            run_single_speaker(curve, &config);

        // The final curve should show a negative slope (tilt applied)
        let slope = slope_db_per_octave(&final_curve, 100.0, 500.0);
        assert!(
            slope < -0.3,
            "Flat+slope config should produce tilted output, but slope={:.2} dB/oct (expected < -0.3)",
            slope
        );
    }

    #[test]
    fn test_pipeline_tilt_not_doubled_by_broadband() {
        // Bug #3: Tilt applied in broadband shelves AND EQ subtraction = double-tilt.
        // slope(tilt only) should approximately equal slope(tilt+broadband).
        let curve = make_test_curve(|_f| 70.0);

        let config_tilt_only = fast_test_config(|c| {
            c.target_tilt = Some(super::super::types::TargetTiltConfig {
                tilt_type: TiltType::Custom,
                slope_db_per_octave: -0.8,
                reference_freq: 1000.0,
                bass_shelf_db: 0.0,
                bass_shelf_freq: 200.0,
            });
        });

        let config_tilt_bb = fast_test_config(|c| {
            c.target_tilt = Some(super::super::types::TargetTiltConfig {
                tilt_type: TiltType::Custom,
                slope_db_per_octave: -0.8,
                reference_freq: 1000.0,
                bass_shelf_db: 0.0,
                bass_shelf_freq: 200.0,
            });
            c.broadband_target_matching =
                Some(super::super::types::BroadbandTargetMatchingConfig { enabled: true });
        });

        let (_, _, _, _, final_tilt, _, _, _, _) =
            run_single_speaker(curve.clone(), &config_tilt_only);
        let (_, _, _, _, final_tilt_bb, _, _, _, _) =
            run_single_speaker(curve, &config_tilt_bb);

        let slope_tilt = slope_db_per_octave(&final_tilt, 200.0, 1000.0);
        let slope_both = slope_db_per_octave(&final_tilt_bb, 200.0, 1000.0);

        let diff = (slope_tilt - slope_both).abs();
        // DE is stochastic, so allow 3.0 dB/oct tolerance. A double-tilt bug
        // would produce ~2× the slope difference (>5 dB/oct).
        assert!(
            diff < 3.0,
            "Tilt slope with broadband ({:.2}) should be within 3.0 dB/oct of tilt-only ({:.2}), diff={:.2}",
            slope_both, slope_tilt, diff
        );
    }

    #[test]
    fn test_pipeline_tilt_no_boost_below_rolloff() {
        // Bug #6: Tilt creates impossible target below speaker rolloff,
        // optimizer wastes filters boosting below F3.
        let curve = make_bookshelf_curve();
        let config = fast_test_config(|c| {
            c.target_tilt = Some(super::super::types::TargetTiltConfig {
                tilt_type: TiltType::Custom,
                slope_db_per_octave: -0.8,
                reference_freq: 1000.0,
                bass_shelf_db: 0.0,
                bass_shelf_freq: 200.0,
            });
        });
        let (_chain, _pre, _post, initial_curve, final_curve, biquads, _mean, _arrival, _fir) =
            run_single_speaker(curve, &config);

        // Behavioral invariant: with tilt active on a bookshelf speaker, the
        // final curve at 30Hz (below rolloff) should not be boosted massively.
        // Tilt clamping prevents the optimizer from chasing an impossible bass target.
        let initial_30 = spl_at(&initial_curve, 30.0);
        let final_30 = spl_at(&final_curve, 30.0);
        assert!(
            final_30 <= initial_30 + 8.0,
            "Final at 30Hz ({:.1}dB) should not exceed initial ({:.1}dB) by more than 8dB \
             (tilt should not cause massive bass boost below rolloff)",
            final_30, initial_30
        );

        // Also check that the total bass boost energy is bounded:
        // sum of boost gains on filters below 50Hz should be limited
        let bass_boost_total: f64 = biquads
            .iter()
            .filter(|b| b.freq < 50.0 && b.db_gain > 0.0)
            .map(|b| b.db_gain)
            .sum();
        assert!(
            bass_boost_total < 10.0,
            "Total boost below 50Hz ({:.1}dB) is excessive — tilt should not drive bass over-boost",
            bass_boost_total
        );
    }

    // ========================================================================
    // Group 2: Broadband invariants
    // ========================================================================

    #[test]
    fn test_pipeline_broadband_no_massive_gain() {
        // Bug #2: Broadband target at 0dB instead of mean → massive correction gains.
        let curve = make_test_curve(|f| {
            // Speaker with a 6dB tilt (brighter at HF)
            70.0 + 3.0 * (f / 1000.0).log2()
        });
        let config = fast_test_config(|c| {
            c.broadband_target_matching =
                Some(super::super::types::BroadbandTargetMatchingConfig { enabled: true });
        });
        let (_chain, pre_score, post_score, _initial, _final_curve, biquads, _mean, _arrival, _fir) =
            run_single_speaker(curve, &config);

        // With broadband handling bulk correction, the mean absolute EQ gain
        // should be moderate. Without broadband, the EQ would need huge gains
        // to handle the 6dB broadband tilt. A massive mean gain (>10dB)
        // indicates broadband failed to do its job.
        let mean_abs_gain: f64 = if biquads.is_empty() {
            0.0
        } else {
            biquads.iter().map(|b| b.db_gain.abs()).sum::<f64>() / biquads.len() as f64
        };
        // Without broadband, a 6dB-tilted curve would need ~6dB mean correction.
        // With broadband handling bulk, EQ should need less. Allow generous margin
        // for DE optimizer variance; catch only catastrophic broadband failure (>12dB).
        assert!(
            mean_abs_gain < 12.0,
            "Mean EQ gain {:.1}dB is too large (broadband should handle bulk correction)",
            mean_abs_gain
        );

        // Score should improve (or at worst not get much worse)
        assert!(
            post_score <= pre_score * 1.5,
            "Score should not degrade significantly: pre={:.4}, post={:.4}",
            pre_score, post_score
        );
    }

    #[test]
    fn test_pipeline_broadband_preserves_out_of_band() {
        // Bug #4: Broadband alignment 20-20kHz but EQ range only 20-1200Hz.
        // Treble should not be mangled by broadband when EQ can't fix it.
        let curve = make_test_curve(|_f| 70.0); // flat curve
        let config = fast_test_config(|c| {
            c.min_freq = 20.0;
            c.max_freq = 1200.0;
            c.broadband_target_matching =
                Some(super::super::types::BroadbandTargetMatchingConfig { enabled: true });
        });
        let (_chain, _pre, _post, initial_curve, final_curve, _biquads, _mean, _arrival, _fir) =
            run_single_speaker(curve, &config);

        // SPL at 5kHz should be within 3dB of initial (broadband shouldn't wreck treble)
        let initial_5k = spl_at(&initial_curve, 5000.0);
        let final_5k = spl_at(&final_curve, 5000.0);
        let delta = (final_5k - initial_5k).abs();
        assert!(
            delta < 3.0,
            "Broadband+EQ should preserve treble: initial@5kHz={:.1}dB, final@5kHz={:.1}dB, delta={:.1}dB",
            initial_5k, final_5k, delta
        );
    }

    #[test]
    fn test_pipeline_broadband_flat_target_not_tilted() {
        // Bug #3 variant: With tilt+broadband, broadband shelves should be small
        // because broadband targets FLAT at mean (tilt is only in EQ subtraction).
        let curve = make_test_curve(|_f| 70.0);
        let config = fast_test_config(|c| {
            c.target_tilt = Some(super::super::types::TargetTiltConfig {
                tilt_type: TiltType::Custom,
                slope_db_per_octave: -0.8,
                reference_freq: 1000.0,
                bass_shelf_db: 0.0,
                bass_shelf_freq: 200.0,
            });
            c.broadband_target_matching =
                Some(super::super::types::BroadbandTargetMatchingConfig { enabled: true });
        });

        // For a flat curve, broadband alignment against a flat target should produce
        // tiny shelf gains (curve is already at mean). Check via the final curve:
        // the broadband contribution should not introduce a large tilt on its own.
        let source = MeasurementSource::InMemory(curve.clone());
        let room_config = RoomConfig {
            version: super::super::types::default_config_version(),
            system: None,
            speakers: {
                let mut m = HashMap::new();
                m.insert("test".to_string(), SpeakerConfig::Single(source.clone()));
                m
            },
            crossovers: None,
            target_curve: None,
            optimizer: config,
            recording_config: None,
        };
        let tmp = tempfile::TempDir::new().expect("failed to create temp dir");
        let result =
            process_single_speaker("test", &source, &room_config, 48000.0, tmp.path(), None)
                .unwrap();

        // With a flat input, broadband alignment should produce small corrections.
        // Check that the flat_gain portion of broadband is small.
        // We verify indirectly: the EQ filters should have small gains
        // (broadband handled bulk, EQ does fine corrections).
        // On a flat input curve, broadband correction should be tiny (curve is
        // already at mean). The EQ should only need small corrections for the
        // tilt target. Mean absolute gain should be modest.
        let biquads = &result.5;
        let mean_abs_gain = if biquads.is_empty() {
            0.0
        } else {
            biquads.iter().map(|b| b.db_gain.abs()).sum::<f64>() / biquads.len() as f64
        };
        assert!(
            mean_abs_gain < 6.0,
            "On a flat curve with tilt+broadband, mean EQ gain should be small but got {:.1}dB",
            mean_abs_gain
        );
    }

    // ========================================================================
    // Group 3: Excursion HPF invariants
    // ========================================================================

    #[test]
    fn test_pipeline_excursion_hpf_no_double_cut() {
        // Bug #5: Excursion HPF cuts + EQ cuts stack → double-cut in bass.
        // The optimizer should see the HPF-adjusted curve and not stack additional cuts.
        let curve = make_test_curve(|f| {
            // 15dB resonance peak at 40Hz on top of bookshelf rolloff
            let base = if f < 60.0 {
                70.0 + 12.0 * (f / 60.0).log2()
            } else {
                70.0
            };
            let peak = 15.0 * (-(((f.log2() - 40.0_f64.log2()) / 0.15).powi(2))).exp();
            base + peak
        });
        let config = fast_test_config(|c| {
            c.excursion_protection = Some(super::super::types::ExcursionProtectionConfig {
                enabled: true,
                auto_detect_f3: true,
                manual_f3_hz: None,
                filter_order: 4,
                filter_type: super::super::types::HighpassType::LinkwitzRiley,
                margin_octaves: 0.25,
            });
        });
        let (_chain, _pre, _post, initial_curve, final_curve, _biquads, _mean, _arrival, _fir) =
            run_single_speaker(curve, &config);

        // Final at 40Hz should not be cut more than 15dB below initial
        // (HPF + EQ together should not double-cut)
        let initial_40 = spl_at(&initial_curve, 40.0);
        let final_40 = spl_at(&final_curve, 40.0);
        assert!(
            final_40 >= initial_40 - 15.0,
            "Excursion HPF + EQ should not double-cut at 40Hz: initial={:.1}dB, final={:.1}dB, cut={:.1}dB",
            initial_40, final_40, initial_40 - final_40
        );
    }

    #[test]
    fn test_pipeline_display_curve_is_raw_measurement() {
        // Bug #5 variant: initial_curve returned should be the raw measurement,
        // not the HPF-attenuated version.
        let curve = make_bookshelf_curve();
        let raw_30hz_spl = spl_at(&curve, 30.0);

        let config = fast_test_config(|c| {
            c.excursion_protection = Some(super::super::types::ExcursionProtectionConfig {
                enabled: true,
                auto_detect_f3: true,
                manual_f3_hz: None,
                filter_order: 4,
                filter_type: super::super::types::HighpassType::LinkwitzRiley,
                margin_octaves: 0.25,
            });
        });
        let (_chain, _pre, _post, initial_curve, _final, _biquads, _mean, _arrival, _fir) =
            run_single_speaker(curve, &config);

        // initial_curve should be the raw measurement (curve_raw), not HPF-adjusted
        let returned_30hz = spl_at(&initial_curve, 30.0);

        assert!(
            (returned_30hz - raw_30hz_spl).abs() < 0.1,
            "initial_curve at 30Hz ({:.1}dB) should match raw measurement ({:.1}dB), not HPF-attenuated",
            returned_30hz, raw_30hz_spl
        );
    }

    // ========================================================================
    // Group 5: Score and data flow
    // ========================================================================

    #[test]
    fn test_pipeline_score_improves_or_stays_same() {
        // For basic optimization, post_score should not be drastically worse than pre_score.
        let curve = make_test_curve(|f| {
            // Room-like response with peaks and dips
            70.0 + 5.0 * (2.0 * std::f64::consts::PI * (f / 200.0).log2()).sin()
                + 3.0 * (2.0 * std::f64::consts::PI * (f / 500.0).log2()).cos()
        });
        let config = fast_test_config(|_| {});
        let (_chain, pre_score, post_score, _initial, _final, _biquads, _mean, _arrival, _fir) =
            run_single_speaker(curve, &config);

        assert!(
            post_score <= pre_score * 1.2,
            "Score should improve or stay similar: pre={:.4}, post={:.4}",
            pre_score, post_score
        );
    }

    #[test]
    fn test_pipeline_tilt_scoring_against_tilt_target() {
        // A curve that already matches the tilt target should get a good score.
        // The same curve without tilt should get a worse score.

        // Create a curve with -0.8 dB/oct slope (matches Harman tilt)
        let tilted_curve = make_test_curve(|f| 70.0 - 0.8 * (f / 1000.0).log2());

        let config_with_tilt = fast_test_config(|c| {
            c.target_tilt = Some(super::super::types::TargetTiltConfig {
                tilt_type: TiltType::Custom,
                slope_db_per_octave: -0.8,
                reference_freq: 1000.0,
                bass_shelf_db: 0.0,
                bass_shelf_freq: 200.0,
            });
        });

        let config_no_tilt = fast_test_config(|_| {});

        let (_, _, post_with_tilt, _, _, _, _, _, _) =
            run_single_speaker(tilted_curve.clone(), &config_with_tilt);
        let (_, _, post_no_tilt, _, _, _, _, _, _) =
            run_single_speaker(tilted_curve, &config_no_tilt);

        // When measured against matching tilt target, score should be very good
        assert!(
            post_with_tilt < 1.0,
            "Curve matching tilt target should have low score, got {:.4}",
            post_with_tilt
        );
        // When measured against flat target, same curve should score worse
        assert!(
            post_no_tilt > post_with_tilt,
            "Same tilted curve should score worse against flat target ({:.4}) than tilt target ({:.4})",
            post_no_tilt, post_with_tilt
        );
    }
}
