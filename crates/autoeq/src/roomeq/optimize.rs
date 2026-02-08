//! Main optimization entry points for room EQ.
//!
//! This module provides the primary public API for room optimization.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::read as load;
use crate::response;
use log::{debug, info, warn};
use math_audio_dsp::analysis::{compute_average_response, find_db_point};
use math_audio_iir_fir::Biquad;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

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
    OptimizerConfig, OptimizationMetadata, RoomConfig, SpeakerConfig, SpeakerGroup,
    TargetCurveConfig, TiltType,
};

// ============================================================================
// Type Aliases
// ============================================================================

/// Internal result type for speaker processing to reduce type complexity
/// Returns: (channel_name, chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
type SpeakerProcessResult = std::result::Result<
    (String, ChannelDspChain, f64, f64, crate::Curve, crate::Curve, Vec<crate::iir::Biquad>, f64, Option<f64>),
    AutoeqError,
>;

/// Result type for mixed mode processing
/// Returns: (chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
type MixedModeResult = (ChannelDspChain, f64, f64, Curve, Curve, Vec<Biquad>, f64, Option<f64>);

/// Detect passband and compute mean SPL for normalization
fn detect_passband_and_mean(curve: &Curve) -> (Option<(f64, f64)>, f64) {
    let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();
    
    let f_low = find_db_point(&freqs_f32, &spl_f32, -3.0, true).unwrap_or(freqs_f32[0]);
    let f_high = find_db_point(&freqs_f32, &spl_f32, -3.0, false).unwrap_or(freqs_f32[freqs_f32.len()-1]);
    
    let norm_range_f32 = Some((f_low, f_high));
    let mean = compute_average_response(&freqs_f32, &spl_f32, norm_range_f32) as f64;
    
    (Some((f_low as f64, f_high as f64)), mean)
}

/// Threshold in dB above which to warn about channel level differences
const LEVEL_DIFFERENCE_WARNING_THRESHOLD: f64 = 6.0;

/// Threshold in ms above which to warn about arrival time differences
const ARRIVAL_TIME_WARNING_THRESHOLD_MS: f64 = 50.0;

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

    info!("Found {} speakers", config.speakers.len());

    // Process each speaker in parallel
    let results: Vec<SpeakerProcessResult> = config
        .speakers
        .par_iter()
        .map(|(channel_name, speaker_config)| {
            info!("Processing channel: {}", channel_name);

            let (chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms) =
                process_speaker_internal(
                    channel_name,
                    speaker_config,
                    config,
                    sample_rate,
                    output_dir,
                )?;

            Ok((
                channel_name.clone(),
                chain,
                pre_score,
                post_score,
                initial_curve,
                final_curve,
                biquads,
                mean_spl,
                arrival_time_ms,
            ))
        })
        .collect();

    // Collect results
    let mut channel_chains: HashMap<String, ChannelDspChain> = HashMap::new();
    let mut channel_results: HashMap<String, ChannelOptimizationResult> = HashMap::new();
    let mut pre_scores: Vec<f64> = Vec::new();
    let mut post_scores: Vec<f64> = Vec::new();
    let mut curves: HashMap<String, crate::Curve> = HashMap::new();
    let mut channel_means: HashMap<String, f64> = HashMap::new();
    let mut channel_arrivals: HashMap<String, f64> = HashMap::new();

    for res in results {
        let (channel_name, chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms) = res?;

        channel_chains.insert(channel_name.clone(), chain);
        curves.insert(channel_name.clone(), final_curve.clone());
        pre_scores.push(pre_score);
        post_scores.push(post_score);
        channel_means.insert(channel_name.clone(), mean_spl);
        if let Some(arrival_ms) = arrival_time_ms {
            channel_arrivals.insert(channel_name.clone(), arrival_ms);
        }

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

    // Time alignment: add delay plugins to align all channels to the slowest one
    // This is done PRE-EQ by inserting at the beginning of the plugin chain
    if channel_arrivals.len() > 1 {
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
                chain.plugins.insert(0, output::create_delay_plugin(*delay_ms));
                info!(
                    "  Channel '{}': added {:.3} ms delay for time alignment",
                    channel_name, delay_ms
                );
            }
        }
    } else if channel_arrivals.is_empty() && config.speakers.len() > 1 {
        info!("No WAV files available for time alignment. Skipping time alignment.");
    }

    // Channel level alignment: add gain plugins to align all channels to the same level
    if channel_means.len() > 1 {
        let means: Vec<f64> = channel_means.values().copied().collect();
        let min_mean = means.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_mean = means.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let level_spread = max_mean - min_mean;

        // Use the minimum level as reference (avoid boosting, prefer cutting)
        let reference_level = min_mean;

        // Warn if level differences are significant (might indicate measurement issues)
        if level_spread > LEVEL_DIFFERENCE_WARNING_THRESHOLD {
            warn!(
                "Channel levels differ by {:.1} dB (threshold: {:.1} dB). \
                This may indicate measurement issues (mic placement, cable problems, etc.).",
                level_spread, LEVEL_DIFFERENCE_WARNING_THRESHOLD
            );
            for (name, mean) in &channel_means {
                info!("  Channel '{}': mean SPL = {:.1} dB", name, mean);
            }
        }

        // Add gain plugins to align channels
        for (channel_name, mean_spl) in &channel_means {
            let alignment_gain = reference_level - mean_spl;

            // Only add gain plugin if the adjustment is significant (> 0.1 dB)
            if alignment_gain.abs() > 0.1
                && let Some(chain) = channel_chains.get_mut(channel_name)
            {
                // Insert gain plugin at the end (after EQ) to adjust final level
                chain.plugins.push(output::create_gain_plugin(alignment_gain));
                info!(
                    "  Channel '{}': added {:.2} dB gain for level alignment",
                    channel_name, alignment_gain
                );
            }
        }
    }

    // ========================================================================
    // Phase Alignment Optimization (Scenario A: WITH Subwoofers)
    // ========================================================================
    // Phase alignment maximizes energy sum in the crossover region by optimizing
    // delay and polarity. This runs BEFORE group delay optimization.
    let mut phase_alignment_results: HashMap<String, (f64, bool)> = HashMap::new();

    if let Some(phase_config) = &config.optimizer.phase_alignment
        && phase_config.enabled
        && let Some(gd_configs) = &config.group_delay
    {
        info!("Running phase alignment optimization...");

        for gd_config in gd_configs {
            let sub_curve = match curves.get(&gd_config.subwoofer) {
                Some(c) => c,
                None => {
                    warn!(
                        "Subwoofer channel '{}' not found for phase alignment",
                        gd_config.subwoofer
                    );
                    continue;
                }
            };

            for speaker_name in &gd_config.speakers {
                if let Some(speaker_curve) = curves.get(speaker_name) {
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
                                    speaker_name, gd_config.subwoofer,
                                    result.delay_ms, result.invert_polarity, result.improvement_db
                                );
                                phase_alignment_results.insert(
                                    speaker_name.clone(),
                                    (result.delay_ms, result.invert_polarity),
                                );
                            }
                            Err(e) => {
                                warn!("  Phase alignment failed for '{}': {}", speaker_name, e);
                            }
                        }
                    } else {
                        debug!(
                            "  Skipping phase alignment for '{}': no phase data available",
                            speaker_name
                        );
                    }
                }
            }
        }
    }

    // Apply phase alignment results (polarity inversion)
    for (speaker_name, (_delay, invert)) in &phase_alignment_results {
        if *invert
            && let Some(chain) = channel_chains.get_mut(speaker_name)
        {
            // Insert polarity inversion at the beginning of the chain
            let invert_plugin = output::create_gain_plugin_with_invert(0.0, true);
            chain.plugins.insert(0, invert_plugin);
            info!("  Applied polarity inversion to '{}'", speaker_name);
        }
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

            // Include phase alignment delay if available
            let phase_delay = phase_alignment_results
                .get(&speaker_name)
                .map(|(d, _)| *d)
                .unwrap_or(0.0);

            let final_speaker_delay = rel_delay + base_delay + phase_delay;

            if final_speaker_delay > 1e-3
                && let Some(chain) = channel_chains.get_mut(&speaker_name)
            {
                output::add_delay_plugin(chain, final_speaker_delay);
                if phase_delay.abs() > 0.01 {
                    info!(
                        "    Applied {:.3} ms delay to '{}' (rel: {:.3} + sub_base: {:.3} + phase: {:.3})",
                        final_speaker_delay, speaker_name, rel_delay, base_delay, phase_delay
                    );
                } else {
                    info!(
                        "    Applied {:.3} ms delay to '{}' (rel: {:.3} + sub_base: {:.3})",
                        final_speaker_delay, speaker_name, rel_delay, base_delay
                    );
                }
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
            groups.entry(speaker_name.to_string()).or_default().push(channel_name.clone());
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
            if let Some(c1) = positioned_channels.remove(p1) { group.push(c1); }
            if let Some(c2) = positioned_channels.remove(p2) { group.push(c2); }
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
        speakers: HashMap::new(),
        crossovers: None,
        target_curve: target_curve.cloned(),
        group_delay: None,
        optimizer: optimizer_config.clone(),
        recording_config: None,
    };

    let (chain, pre_score, post_score, initial_curve, final_curve, biquads, _mean_spl, _arrival_time_ms) =
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
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
fn process_speaker_internal(
    channel_name: &str,
    speaker_config: &SpeakerConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    output_dir: Option<&Path>,
) -> Result<MixedModeResult> {
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
        MeasurementSource::InMemory(_) => None,
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
) -> Result<MixedModeResult> {
    // Load measurement
    let curve = load::load_source(source)
        .map_err(|e| AutoeqError::InvalidMeasurement { message: format!("Failed to load measurement for channel {}: {}", channel_name, e) })?;

    debug!(
        "  Loaded measurement: {:.1} Hz - {:.1} Hz",
        curve.freq[0],
        curve.freq[curve.freq.len() - 1]
    );

    // Extract wav_path and calculate arrival time for time alignment
    let arrival_time_ms: Option<f64> = extract_wav_path(source)
        .and_then(|wav_path| {
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
                        debug!("  Could not determine arrival time for '{}': {}", channel_name, e);
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
        if tilt_config.tilt_type != TiltType::Flat {
            info!("  Building target curve with {:?} tilt ({:.2} dB/octave)",
                  tilt_config.tilt_type, tilt_config.slope_db_per_octave);
            Some(target_tilt::build_target_curve_with_tilt(&curve.freq, tilt_config))
        } else {
            None
        }
    } else {
        None
    };

    // ========================================================================
    // Excursion Protection (detect F3, generate HPF)
    // ========================================================================
    let excursion_filters: Vec<Biquad> = if let Some(exc_config) = &room_config.optimizer.excursion_protection {
        if exc_config.enabled {
            info!("  Applying excursion protection...");
            match excursion::generate_excursion_protection(&curve, exc_config, sample_rate) {
                Ok(result) => {
                    info!("  Excursion protection: F3={:.1}Hz, HPF={:.1}Hz ({} filters)",
                          result.f3_hz, result.hpf_frequency, result.filters.len());
                    result.filters
                }
                Err(e) => {
                    warn!("  Excursion protection failed: {}. Continuing without protection.", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Compute pre-score (within EQ range)
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;

    // Detect passband for normalization
    let (norm_range, mean) = detect_passband_and_mean(&curve);
    
    if let Some((f_low, f_high)) = norm_range {
        info!("  Detected passband for '{}': {:.1} Hz - {:.1} Hz (Mean SPL: {:.2} dB)", 
              channel_name, f_low, f_high, mean);
    }

    let normalized_spl = &curve.spl - mean;
    let pre_score = crate::loss::flat_loss(&curve.freq, &normalized_spl, min_freq, max_freq);

    // Compute full-range mean SPL for level alignment between channels
    let mean_spl = curve.spl.mean().unwrap_or(0.0);

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

            // Build DSP chain with convolution plugin referencing the FIR WAV file
            let convolution_plugin = output::create_convolution_plugin(&filename);
            let mut chain = output::build_channel_dsp_chain_with_curves(
                channel_name,
                None,
                Vec::new(),
                &[],
                None,
                None,
            );
            chain.plugins.push(convolution_plugin);

            let complex_resp =
                response::compute_fir_complex_response(&coeffs, &curve.freq, sample_rate);
            let final_curve = response::apply_complex_response(&curve, &complex_resp);

            // Compute post_score consistently with pre_score
            let (_, mean_final) = detect_passband_and_mean(&final_curve);
            let normalized_final_spl = &final_curve.spl - mean_final;
            let post_score =
                crate::loss::flat_loss(&final_curve.freq, &normalized_final_spl, min_freq, max_freq);

            info!(
                "  Pre-score: {:.6}, Post-score: {:.6}",
                pre_score, post_score
            );

            // Extend curves to 20 Hz – 20 kHz for display output
            let display_initial = output::extend_curve_to_full_range(&curve);
            let display_fir_resp =
                response::compute_fir_complex_response(&coeffs, &display_initial.freq, sample_rate);
            let display_final = response::apply_complex_response(&display_initial, &display_fir_resp);

            let mut initial_data: super::types::CurveData = (&display_initial).into();
            initial_data.norm_range = norm_range;
            let mut final_data: super::types::CurveData = (&display_final).into();
            final_data.norm_range = norm_range;

            chain.initial_curve = Some(initial_data);
            chain.final_curve = Some(final_data);

            Ok((chain, pre_score, post_score, curve.clone(), final_curve, vec![], mean_spl, arrival_time_ms))
        }
        "mixed" => {
            // Check for frequency-based crossover configuration
            if let Some(mixed_config) = &room_config.optimizer.mixed_config {
                // New frequency-based mixed mode: FIR on one band, IIR on the other
                return process_mixed_mode_crossover(
                    channel_name,
                    &curve,
                    room_config,
                    mixed_config,
                    sample_rate,
                    output_dir,
                    min_freq,
                    max_freq,
                    mean_spl,
                    pre_score,
                    arrival_time_ms,
                );
            }

            // Legacy sequential mixed mode: IIR first, then FIR on residual
            let (eq_filters, _opt_loss) = eq::optimize_channel_eq(
                &curve,
                &room_config.optimizer,
                room_config.target_curve.as_ref(),
                sample_rate,
            )
            .map_err(|e| AutoeqError::OptimizationFailed { message: format!("IIR optimization failed for channel {}: {}", channel_name, e) })?;

            info!("  IIR stage: {} filters", eq_filters.len());

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

            // Compute post_score consistently with pre_score
            let (_, mean_final) = detect_passband_and_mean(&final_curve);
            let normalized_final_spl = &final_curve.spl - mean_final;
            let post_score =
                crate::loss::flat_loss(&final_curve.freq, &normalized_final_spl, min_freq, max_freq);

            info!(
                "  Pre-score: {:.6}, Post-score: {:.6}",
                pre_score, post_score
            );

            // Extend curves to 20 Hz – 20 kHz for display output
            let display_initial = output::extend_curve_to_full_range(&curve);
            let display_iir_resp =
                response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
            let display_iir_corrected = response::apply_complex_response(&display_initial, &display_iir_resp);
            let display_fir_resp =
                response::compute_fir_complex_response(&coeffs, &display_initial.freq, sample_rate);
            let display_final = response::apply_complex_response(&display_iir_corrected, &display_fir_resp);

            let mut initial_data: super::types::CurveData = (&display_initial).into();
            initial_data.norm_range = norm_range;
            let mut final_data: super::types::CurveData = (&display_final).into();
            final_data.norm_range = norm_range;

            chain.initial_curve = Some(initial_data);
            chain.final_curve = Some(final_data);

            Ok((chain, pre_score, post_score, curve.clone(), final_curve, eq_filters, mean_spl, arrival_time_ms))
        }
        _ => {
            // Default IIR mode with enhanced processing

            // Apply target tilt to the curve (subtract tilt from measurement)
            let optimization_curve = if let Some(ref tilt_curve) = target_tilt_curve {
                Curve {
                    freq: curve.freq.clone(),
                    spl: &curve.spl - &tilt_curve.spl,
                    phase: curve.phase.clone(),
                }
            } else {
                curve.clone()
            };

            // ================================================================
            // Schroeder Split Optimization (if configured)
            // ================================================================
            let eq_filters = if let Some(schroeder_config) = &room_config.optimizer.schroeder_split {
                if schroeder_config.enabled {
                    let schroeder_freq = if let Some(ref dims) = schroeder_config.room_dimensions {
                        let calculated = dims.schroeder_frequency();
                        info!("  Schroeder split: calculated frequency {:.1} Hz from room dimensions", calculated);
                        calculated
                    } else {
                        schroeder_config.schroeder_freq
                    };
                    info!("  Schroeder split: optimizing below {:.1} Hz with max_q={:.1}, above with max_q={:.1}",
                          schroeder_freq, schroeder_config.low_freq_config.max_q, schroeder_config.high_freq_config.max_q);

                    // Two-pass optimization with different Q constraints
                    let (low_filters, high_filters) = optimize_with_schroeder_split(
                        &optimization_curve,
                        &room_config.optimizer,
                        schroeder_config,
                        sample_rate,
                    )?;

                    let mut combined_filters = low_filters;
                    combined_filters.extend(high_filters);
                    info!("  Schroeder split: {} low-freq filters + {} high-freq filters",
                          combined_filters.iter().filter(|f| f.freq < schroeder_freq).count(),
                          combined_filters.iter().filter(|f| f.freq >= schroeder_freq).count());
                    combined_filters
                } else {
                    // Standard optimization
                    let (filters, _opt_loss) = eq::optimize_channel_eq(
                        &optimization_curve,
                        &room_config.optimizer,
                        room_config.target_curve.as_ref(),
                        sample_rate,
                    )
                    .map_err(|e| AutoeqError::OptimizationFailed { message: format!("EQ optimization failed for channel {}: {}", channel_name, e) })?;
                    filters
                }
            } else {
                // Standard optimization
                let (filters, _opt_loss) = eq::optimize_channel_eq(
                    &optimization_curve,
                    &room_config.optimizer,
                    room_config.target_curve.as_ref(),
                    sample_rate,
                )
                .map_err(|e| AutoeqError::OptimizationFailed { message: format!("EQ optimization failed for channel {}: {}", channel_name, e) })?;
                filters
            };

            info!("  Optimized {} EQ filters", eq_filters.len());

            // Combine excursion protection filters with EQ filters
            let mut all_filters = excursion_filters.clone();
            all_filters.extend(eq_filters.clone());

            let mut chain = output::build_channel_dsp_chain_with_curves(
                channel_name,
                None,
                Vec::new(),
                &all_filters,
                None,
                None,
            );

            // Compute final response including all filters
            let all_resp =
                response::compute_peq_complex_response(&all_filters, &curve.freq, sample_rate);
            let final_curve = response::apply_complex_response(&curve, &all_resp);

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

            let (_, mean_final) = detect_passband_and_mean(&score_curve);
            let normalized_final_spl = &score_curve.spl - mean_final;
            let post_score =
                crate::loss::flat_loss(&score_curve.freq, &normalized_final_spl, min_freq, max_freq);

            info!(
                "  Pre-score: {:.6}, Post-score: {:.6}",
                pre_score, post_score
            );

            // Extend curves to 20 Hz – 20 kHz for display output
            let display_initial = output::extend_curve_to_full_range(&curve);
            let display_resp =
                response::compute_peq_complex_response(&all_filters, &display_initial.freq, sample_rate);
            let display_final = response::apply_complex_response(&display_initial, &display_resp);

            let mut initial_data: super::types::CurveData = (&display_initial).into();
            initial_data.norm_range = norm_range;
            let mut final_data: super::types::CurveData = (&display_final).into();
            final_data.norm_range = norm_range;

            chain.initial_curve = Some(initial_data);
            chain.final_curve = Some(final_data);

            Ok((chain, pre_score, post_score, curve.clone(), final_curve, eq_filters, mean_spl, arrival_time_ms))
        }
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

    let low_filters = ((total_filters as f64 * low_ratio).round() as usize).max(1).min(total_filters - 1);
    let high_filters = total_filters - low_filters;

    debug!("  Schroeder split: {} filters below {:.1}Hz, {} filters above",
           low_filters, schroeder_freq, high_filters);

    // Low frequency optimization (below Schroeder)
    let low_optimizer = OptimizerConfig {
        num_filters: low_filters,
        min_freq: optimizer.min_freq,
        max_freq: schroeder_freq,
        min_q: low_config.min_q,
        max_q: low_config.max_q,
        min_db: optimizer.min_db,
        max_db: if low_config.allow_boost { optimizer.max_db } else { 0.0 }, // Cuts only if !allow_boost
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
    let low_resp = response::compute_peq_complex_response(&low_eq_filters, &curve.freq, sample_rate);
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

    Ok((low_eq_filters, high_eq_filters))
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
    let mut chain = output::build_multidriver_dsp_chain_with_curves(
        channel_name,
        &gains,
        &delays,
        &crossover_freqs,
        crossover::crossover_type_to_string(&crossover_type),
        &eq_filters,
        None,
        None,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    // Detect passband for normalization
    let (norm_range, mean_spl) = detect_passband_and_mean(&combined_curve);

    // Extend curves to 20 Hz – 20 kHz for display output
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data);
    chain.final_curve = Some(final_data);

    Ok((
        chain,
        pre_score,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
        mean_spl,
        None, // No single WAV for speaker groups
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

    let mut chain = output::build_multisub_dsp_chain_with_curves(
        channel_name,
        &group.name,
        group.subwoofers.len(),
        &result.gains,
        &result.delays,
        &eq_filters,
        None,
        None,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    // Detect passband for normalization
    let (norm_range, mean_spl) = detect_passband_and_mean(&combined_curve);

    // Extend curves to 20 Hz – 20 kHz for display output
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data);
    chain.final_curve = Some(final_data);

    Ok((
        chain,
        result.pre_objective,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
        mean_spl,
        None, // No single WAV for multi-sub groups
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

    let mut chain = output::build_dba_dsp_chain_with_curves(
        channel_name,
        &result.gains,
        &result.delays,
        &eq_filters,
        None,
        None,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    // Detect passband for normalization
    let (norm_range, mean_spl) = detect_passband_and_mean(&combined_curve);

    // Extend curves to 20 Hz – 20 kHz for display output
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data);
    chain.final_curve = Some(final_data);

    Ok((
        chain,
        result.pre_objective,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
        mean_spl,
        None, // No single WAV for DBA
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

    let (eq_filters, _) = eq::optimize_channel_eq(
        iir_curve,
        &iir_config,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("IIR optimization failed for {} band: {}", if fir_uses_low { "high" } else { "low" }, e),
    })?;

    info!("  IIR stage: {} filters for {} band", eq_filters.len(), if fir_uses_low { "high" } else { "low" });

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
        message: format!("FIR generation failed for {} band: {}", if fir_uses_low { "low" } else { "high" }, e),
    })?;

    // Save FIR to WAV
    let fir_filename = format!("{}_band_fir.wav", channel_name);
    let wav_path = output_dir.join(&fir_filename);
    crate::fir::save_fir_to_wav(&fir_coeffs, sample_rate as u32, &wav_path)
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("Failed to save FIR WAV: {}", e),
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
    let (lp_resp, hp_resp) = compute_lr24_crossover_responses(&curve.freq, crossover_freq, sample_rate);

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
    let post_score = crate::loss::flat_loss(&final_curve.freq, &normalized_final_spl, min_freq, max_freq);

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

    chain.initial_curve = Some(initial_data);
    chain.final_curve = Some(final_data);

    Ok((
        chain,
        pre_score,
        post_score,
        curve.clone(),
        final_curve,
        eq_filters,
        mean,
        arrival_time_ms,
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
fn compute_lr24_crossover_responses(
    frequencies: &ndarray::Array1<f64>,
    crossover_freq: f64,
    _sample_rate: f64,
) -> (Vec<num_complex::Complex<f64>>, Vec<num_complex::Complex<f64>>) {
    use num_complex::Complex;

    let mut lp_resp = Vec::with_capacity(frequencies.len());
    let mut hp_resp = Vec::with_capacity(frequencies.len());

    // LR24 = two cascaded 2nd-order Butterworth filters
    // Using simplified magnitude response formula

    for &freq in frequencies.iter() {
        // 2nd-order Butterworth lowpass transfer function (analog)
        // H(s) = 1 / (s^2 + s/Q + 1)  (normalized)
        // After bilinear transform and cascading twice for LR24

        // Simplified: compute magnitude response directly
        // For LR24 lowpass: |H|^2 = 1 / (1 + (f/fc)^8)
        let ratio = freq / crossover_freq;
        let ratio_sq = ratio * ratio;
        let ratio_4 = ratio_sq * ratio_sq;
        let ratio_8 = ratio_4 * ratio_4;

        let lp_mag_sq = 1.0 / (1.0 + ratio_8);
        let hp_mag_sq = ratio_8 / (1.0 + ratio_8);

        // LR crossovers have 0 or 180 degree phase at crossover
        // For scoring purposes, we primarily care about magnitude
        let lp_mag = lp_mag_sq.sqrt();
        let hp_mag = hp_mag_sq.sqrt();

        lp_resp.push(Complex::new(lp_mag, 0.0));
        hp_resp.push(Complex::new(hp_mag, 0.0));
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
    let octave_centers = [31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
    
    for &center in &octave_centers {
        let f_min = center / 2.0_f64.sqrt();
        let f_max = center * 2.0_f64.sqrt();
        
        // Find overlap range
        let start_freq = f_min.max(curve1.freq[0]).max(curve2.freq[0]);
        let end_freq = f_max.min(curve1.freq[curve1.freq.len()-1]).min(curve2.freq[curve2.freq.len()-1]);
        
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
