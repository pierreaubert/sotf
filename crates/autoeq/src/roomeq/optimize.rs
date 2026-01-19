//! Main optimization entry points for room EQ.
//!
//! This module provides the primary public API for room optimization.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::read as load;
use crate::response;
use log::{debug, info, warn};
use math_audio_iir_fir::Biquad;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

use super::config::validate_room_config;
use super::crossover;
use super::dba;
use super::eq;
use super::fir;
use super::group_delay;
use super::multisub;
use super::output;
use super::types::{
    ChannelDspChain, DspChainOutput, MeasurementSource, MultiSubGroup, OptimizerConfig,
    OptimizationMetadata, RoomConfig, SpeakerConfig, SpeakerGroup, TargetCurveConfig,
};

// ============================================================================
// Type Aliases
// ============================================================================

/// Internal result type for speaker processing to reduce type complexity
type SpeakerProcessResult = std::result::Result<
    (String, ChannelDspChain, f64, f64, crate::Curve, crate::Curve, Vec<crate::iir::Biquad>),
    AutoeqError,
>;

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
    _callback: Option<RoomOptimizationCallback>,
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

    info!("Found {} speakers", config.speakers.len());

    // Process each speaker in parallel
    let results: Vec<SpeakerProcessResult> = config
        .speakers
        .par_iter()
        .map(|(channel_name, speaker_config)| {
            info!("Processing channel: {}", channel_name);

            let (chain, pre_score, post_score, initial_curve, final_curve, biquads) =
                process_speaker_internal(
                    channel_name,
                    speaker_config,
                    config,
                    sample_rate,
                    None,
                )?;

            Ok((
                channel_name.clone(),
                chain,
                pre_score,
                post_score,
                initial_curve,
                final_curve,
                biquads,
            ))
        })
        .collect();

    // Collect results
    let mut channel_chains: HashMap<String, ChannelDspChain> = HashMap::new();
    let mut channel_results: HashMap<String, ChannelOptimizationResult> = HashMap::new();
    let mut pre_scores: Vec<f64> = Vec::new();
    let mut post_scores: Vec<f64> = Vec::new();
    let mut curves: HashMap<String, crate::Curve> = HashMap::new();

    for res in results {
        let (channel_name, chain, pre_score, post_score, initial_curve, final_curve, biquads) = res?;

        channel_chains.insert(channel_name.clone(), chain);
        curves.insert(channel_name.clone(), final_curve.clone());
        pre_scores.push(pre_score);
        post_scores.push(post_score);

        channel_results.insert(
            channel_name.clone(),
            ChannelOptimizationResult {
                name: channel_name,
                pre_score,
                post_score,
                initial_curve,
                final_curve,
                biquads,
                fir_coeffs: None,
            },
        );
    }

    // Group Delay Optimization
    if let Some(gd_configs) = &config.group_delay {
        info!("Optimizing group delay alignments...");

        let mut calculated_rel_delays = Vec::new();
        let mut sub_base_delays: HashMap<String, f64> = HashMap::new();

        for gd_config in gd_configs {
            let sub_curve = match curves.get(&gd_config.subwoofer) {
                Some(c) => c,
                None => {
                    warn!(
                        "Subwoofer channel '{}' not found for group delay optimization",
                        gd_config.subwoofer
                    );
                    continue;
                }
            };

            for speaker_name in &gd_config.speakers {
                if let Some(speaker_curve) = curves.get(speaker_name) {
                    info!(
                        "  Aligning '{}' with '{}'",
                        speaker_name, gd_config.subwoofer
                    );

                    let delay_res = group_delay::optimize_group_delay(
                        sub_curve,
                        speaker_curve,
                        gd_config.min_freq,
                        gd_config.max_freq,
                    );

                    match delay_res {
                        Ok(delay_ms) => {
                            info!(
                                "    Optimal relative delay: {:.3} ms (positive = delay speaker)",
                                delay_ms
                            );

                            calculated_rel_delays.push((
                                gd_config.subwoofer.clone(),
                                speaker_name.clone(),
                                delay_ms,
                            ));

                            if delay_ms < 0.0 {
                                let current_base =
                                    *sub_base_delays.get(&gd_config.subwoofer).unwrap_or(&0.0);
                                if -delay_ms > current_base {
                                    sub_base_delays.insert(gd_config.subwoofer.clone(), -delay_ms);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("    Group delay optimization failed: {}", e);
                        }
                    }
                } else {
                    warn!("Speaker channel '{}' not found", speaker_name);
                }
            }
        }

        // Apply delays
        for (sub_name, base_delay) in &sub_base_delays {
            if *base_delay > 1e-3
                && let Some(chain) = channel_chains.get_mut(sub_name)
            {
                output::add_delay_plugin(chain, *base_delay);
                info!(
                    "    Applied base delay of {:.3} ms to subwoofer '{}'",
                    base_delay, sub_name
                );
            }
        }

        for (sub_name, speaker_name, rel_delay) in calculated_rel_delays {
            let base_delay = *sub_base_delays.get(&sub_name).unwrap_or(&0.0);
            let final_speaker_delay = rel_delay + base_delay;

            if final_speaker_delay > 1e-3
                && let Some(chain) = channel_chains.get_mut(&speaker_name)
            {
                output::add_delay_plugin(chain, final_speaker_delay);
                info!(
                    "    Applied {:.3} ms delay to '{}' (rel: {:.3} + sub_base: {:.3})",
                    final_speaker_delay, speaker_name, rel_delay, base_delay
                );
            }
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
        speakers: HashMap::new(),
        crossovers: None,
        target_curve: target_curve.cloned(),
        group_delay: None,
        optimizer: optimizer_config.clone(),
    };

    let (chain, pre_score, post_score, initial_curve, final_curve, biquads) =
        process_speaker_internal(channel_name, speaker_config, &room_config, sample_rate, None)?;

    Ok(SpeakerOptimizationResult {
        chain,
        pre_score,
        post_score,
        initial_curve,
        final_curve,
        biquads,
        fir_coeffs: None,
    })
}

// ============================================================================
// Internal Processing Functions
// ============================================================================

/// Process a single speaker (simple or group)
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads)
#[allow(clippy::type_complexity)]
fn process_speaker_internal(
    channel_name: &str,
    speaker_config: &SpeakerConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    output_dir: Option<&Path>,
) -> Result<(ChannelDspChain, f64, f64, Curve, Curve, Vec<Biquad>)> {
    let output_dir = output_dir.unwrap_or(Path::new("."));

    match speaker_config {
        SpeakerConfig::Single(source) => {
            process_single_speaker(channel_name, source, room_config, sample_rate, output_dir)
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
    }
}

/// Process a simple speaker with a single measurement
#[allow(clippy::type_complexity)]
fn process_single_speaker(
    channel_name: &str,
    source: &MeasurementSource,
    room_config: &RoomConfig,
    sample_rate: f64,
    output_dir: &Path,
) -> Result<(ChannelDspChain, f64, f64, Curve, Curve, Vec<Biquad>)> {
    // Load measurement
    let curve = load::load_source(source)
        .map_err(|e| AutoeqError::InvalidMeasurement { message: format!("Failed to load measurement for channel {}: {}", channel_name, e) })?;

    debug!(
        "  Loaded measurement: {:.1} Hz - {:.1} Hz",
        curve.freq[0],
        curve.freq[curve.freq.len() - 1]
    );

    // Compute pre-score
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;

    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..curve.freq.len() {
        if curve.freq[i] >= min_freq && curve.freq[i] <= max_freq {
            sum += curve.spl[i];
            count += 1;
        }
    }
    let mean = if count > 0 { sum / count as f64 } else { 0.0 };
    let normalized_spl = &curve.spl - mean;
    let pre_score = crate::loss::flat_loss(&curve.freq, &normalized_spl, min_freq, max_freq);

    match room_config.optimizer.mode.as_str() {
        "fir" => {
            info!("  Generating FIR filter...");
            let coeffs = fir::generate_fir_correction(
                &curve,
                &room_config.optimizer,
                room_config.target_curve.as_ref(),
                sample_rate,
            )
            .map_err(|e| AutoeqError::OptimizationFailed { message: format!("FIR generation failed: {}", e) })?;

            let filename = format!("{}_fir.wav", channel_name);
            let wav_path = output_dir.join(&filename);
            crate::fir::save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path)
                .map_err(|e| AutoeqError::OptimizationFailed { message: format!("Failed to save FIR WAV: {}", e) })?;

            info!("  Saved FIR filter to {}", wav_path.display());

            let _plugin = output::create_convolution_plugin(&filename);
            let chain = output::build_channel_dsp_chain_with_curves(
                channel_name,
                None,
                Vec::new(),
                &[],
                Some(&curve),
                None,
            );

            let complex_resp =
                response::compute_fir_complex_response(&coeffs, &curve.freq, sample_rate);
            let final_curve = response::apply_complex_response(&curve, &complex_resp);

            let mut chain = chain;
            chain.final_curve = Some((&final_curve).into());

            Ok((chain, pre_score, 0.0, curve.clone(), final_curve, vec![]))
        }
        "mixed" => {
            let (eq_filters, post_iir_score) = eq::optimize_channel_eq(
                &curve,
                &room_config.optimizer,
                room_config.target_curve.as_ref(),
                sample_rate,
            )
            .map_err(|e| AutoeqError::OptimizationFailed { message: format!("IIR optimization failed for channel {}: {}", channel_name, e) })?;

            info!(
                "  IIR stage: {} filters, score={:.6}",
                eq_filters.len(),
                post_iir_score
            );

            let iir_resp =
                response::compute_peq_complex_response(&eq_filters, &curve.freq, sample_rate);
            let final_curve_iir = response::apply_complex_response(&curve, &iir_resp);
            let input_plus_iir = final_curve_iir.clone();

            info!("  Generating FIR for residual...");
            let coeffs = fir::generate_fir_correction(
                &input_plus_iir,
                &room_config.optimizer,
                room_config.target_curve.as_ref(),
                sample_rate,
            )
            .map_err(|e| AutoeqError::OptimizationFailed { message: format!("FIR generation failed: {}", e) })?;

            let filename = format!("{}_residual_fir.wav", channel_name);
            let wav_path = output_dir.join(&filename);
            crate::fir::save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path)
                .map_err(|e| AutoeqError::OptimizationFailed { message: format!("Failed to save FIR WAV: {}", e) })?;

            info!("  Saved FIR filter to {}", wav_path.display());

            let conv_plugin = output::create_convolution_plugin(&filename);
            let mut chain =
                output::build_channel_dsp_chain(channel_name, None, Vec::new(), &eq_filters);
            chain.plugins.push(conv_plugin);

            let fir_resp =
                response::compute_fir_complex_response(&coeffs, &curve.freq, sample_rate);
            let final_curve = response::apply_complex_response(&input_plus_iir, &fir_resp);

            chain.initial_curve = Some((&curve).into());
            chain.final_curve = Some((&final_curve).into());

            Ok((chain, pre_score, 0.0, curve.clone(), final_curve, eq_filters))
        }
        _ => {
            // Default IIR mode
            let (eq_filters, post_score) = eq::optimize_channel_eq(
                &curve,
                &room_config.optimizer,
                room_config.target_curve.as_ref(),
                sample_rate,
            )
            .map_err(|e| AutoeqError::OptimizationFailed { message: format!("EQ optimization failed for channel {}: {}", channel_name, e) })?;

            info!("  Optimized {} EQ filters", eq_filters.len());
            info!(
                "  Pre-score: {:.6}, Post-score: {:.6}",
                pre_score, post_score
            );

            let chain = output::build_channel_dsp_chain_with_curves(
                channel_name,
                None,
                Vec::new(),
                &eq_filters,
                Some(&curve),
                None,
            );

            let iir_resp =
                response::compute_peq_complex_response(&eq_filters, &curve.freq, sample_rate);
            let final_curve = response::apply_complex_response(&curve, &iir_resp);

            let mut chain = chain;
            chain.final_curve = Some((&final_curve).into());

            Ok((chain, pre_score, post_score, curve.clone(), final_curve, eq_filters))
        }
    }
}

/// Process a speaker group with multiple drivers and crossovers
#[allow(clippy::type_complexity)]
fn process_speaker_group(
    channel_name: &str,
    group: &SpeakerGroup,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<(ChannelDspChain, f64, f64, Curve, Curve, Vec<Biquad>)> {
    // Load all measurements in the group
    let mut driver_curves = Vec::new();
    for (i, source) in group.measurements.iter().enumerate() {
        let curve = load::load_source(source)
            .map_err(|e| AutoeqError::InvalidMeasurement {
                message: format!(
                    "Failed to load driver {} measurement for channel {}: {}",
                    i, channel_name, e
                ),
            })?;
        driver_curves.push(curve);
    }

    debug!("  Loaded {} driver measurements", driver_curves.len());

    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let max_db = room_config.optimizer.max_db;

    // Compute peak SPL for each driver
    let mut peaks = Vec::with_capacity(driver_curves.len());
    for driver in &driver_curves {
        let mut peak_spl = f64::NEG_INFINITY;
        for j in 0..driver.freq.len() {
            if driver.freq[j] >= min_freq && driver.freq[j] <= max_freq && driver.spl[j] > peak_spl
            {
                peak_spl = driver.spl[j];
            }
        }
        peaks.push(peak_spl);
    }

    let max_peak = peaks.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_peak = peaks.iter().cloned().fold(f64::INFINITY, f64::min);
    let peak_spread = max_peak - min_peak;

    for (i, peak) in peaks.iter().enumerate() {
        debug!("    Driver {}: peak {:.1} dB", i, peak);
    }
    debug!(
        "    Level spread: {:.1} dB (max gain bounds: ±{:.1} dB)",
        peak_spread, max_db
    );

    if peak_spread > max_db {
        warn!(
            "Driver levels differ by {:.1} dB, which exceeds the gain bounds of ±{:.1} dB.",
            peak_spread, max_db
        );
    }

    // Get crossover configuration
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

    let crossover_type = crossover::parse_crossover_type(&crossover_config.crossover_type)
        .map_err(|e| AutoeqError::InvalidConfiguration { message: e.to_string() })?;

    let fixed_freqs: Option<Vec<f64>> = if let Some(ref freqs) = crossover_config.frequencies {
        Some(freqs.clone())
    } else if let Some(freq) = crossover_config.frequency {
        Some(vec![freq])
    } else {
        None
    };

    if let Some(ref freqs) = fixed_freqs {
        info!("  Using fixed crossover frequencies: {:?} Hz", freqs);
    }

    // Compute pre-score
    let n_drivers = driver_curves.len();
    let initial_gains = vec![0.0; n_drivers];

    let mut initial_xover_freqs = Vec::new();
    for i in 0..(n_drivers - 1) {
        let lower_mean =
            driver_curves[i].freq.iter().sum::<f64>() / driver_curves[i].freq.len() as f64;
        let upper_mean =
            driver_curves[i + 1].freq.iter().sum::<f64>() / driver_curves[i + 1].freq.len() as f64;
        let geom_mean = (lower_mean * upper_mean).sqrt();
        initial_xover_freqs.push(geom_mean);
    }

    let driver_measurements: Vec<crate::loss::DriverMeasurement> = driver_curves
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

    // Optimize crossover
    let (gains, delays, crossover_freqs, combined_curve) = crossover::optimize_crossover(
        driver_curves,
        crossover_type,
        sample_rate,
        &room_config.optimizer,
        fixed_freqs,
    )
    .map_err(|e| AutoeqError::OptimizationFailed { message: format!("Crossover optimization failed: {}", e) })?;

    info!(
        "  Optimized crossover: freqs={:?}, gains={:?}, delays={:?}",
        crossover_freqs, gains, delays
    );

    // Optimize EQ on the combined response
    let (eq_filters, post_score) = eq::optimize_channel_eq(
        &combined_curve,
        &room_config.optimizer,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed { message: format!("EQ optimization failed for channel {}: {}", channel_name, e) })?;

    info!("  Optimized {} EQ filters", eq_filters.len());
    info!(
        "  Pre-score: {:.6}, Post-score: {:.6}",
        pre_score, post_score
    );

    // Build multi-driver DSP chain
    let chain = output::build_multidriver_dsp_chain_with_curves(
        channel_name,
        &gains,
        &delays,
        &crossover_freqs,
        crossover::crossover_type_to_string(&crossover_type),
        &eq_filters,
        Some(&combined_curve),
        None,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    let mut chain = chain;
    chain.final_curve = Some((&final_curve).into());

    Ok((
        chain,
        pre_score,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
    ))
}

/// Process multi-subwoofer group
#[allow(clippy::type_complexity)]
fn process_multisub_group(
    channel_name: &str,
    group: &MultiSubGroup,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<(ChannelDspChain, f64, f64, Curve, Curve, Vec<Biquad>)> {
    let (result, combined_curve) =
        multisub::optimize_multisub(&group.subwoofers, &room_config.optimizer, sample_rate)
            .map_err(|e| AutoeqError::OptimizationFailed { message: format!("Multi-sub optimization failed: {}", e) })?;

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
    .map_err(|e| AutoeqError::OptimizationFailed { message: format!("EQ optimization failed for multi-sub sum: {}", e) })?;

    info!(
        "  Global EQ: {} filters, score={:.6}",
        eq_filters.len(),
        post_score
    );

    let chain = output::build_multisub_dsp_chain_with_curves(
        channel_name,
        &group.name,
        group.subwoofers.len(),
        &result.gains,
        &result.delays,
        &eq_filters,
        Some(&combined_curve),
        None,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    let mut chain = chain;
    chain.final_curve = Some((&final_curve).into());

    Ok((
        chain,
        result.pre_objective,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
    ))
}

/// Process DBA configuration
#[allow(clippy::type_complexity)]
fn process_dba(
    channel_name: &str,
    dba_config: &super::types::DBAConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<(ChannelDspChain, f64, f64, Curve, Curve, Vec<Biquad>)> {
    let (result, combined_curve) =
        dba::optimize_dba(dba_config, &room_config.optimizer, sample_rate)
            .map_err(|e| AutoeqError::OptimizationFailed { message: format!("DBA optimization failed: {}", e) })?;

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
    .map_err(|e| AutoeqError::OptimizationFailed { message: format!("EQ optimization failed for DBA sum: {}", e) })?;

    info!(
        "  Global EQ: {} filters, score={:.6}",
        eq_filters.len(),
        post_score
    );

    let chain = output::build_dba_dsp_chain_with_curves(
        channel_name,
        &result.gains,
        &result.delays,
        &eq_filters,
        Some(&combined_curve),
        None,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    let mut chain = chain;
    chain.final_curve = Some((&final_curve).into());

    Ok((
        chain,
        result.pre_objective,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
    ))
}
