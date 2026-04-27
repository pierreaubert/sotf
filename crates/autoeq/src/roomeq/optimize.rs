//! Main optimization entry points for room EQ.
//!
//! This module provides the primary public API for room optimization.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use log::{debug, info, warn};
use math_audio_dsp::analysis::compute_average_response;
use math_audio_iir_fir::Biquad;
use num_complex::Complex64;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::config::validate_room_config;
use super::fir;
use super::output;
use super::phase_alignment;
use super::types::{
    ChannelDspChain, DspChainOutput, MeasurementSource, OptimizationMetadata, OptimizerConfig,
    PerceptualMetrics, ProcessingMode, RoomConfig, SpeakerConfig, SystemModel, TargetCurveConfig,
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

    let phase_response = crate::response::compute_fir_complex_response(
        &phase_fir,
        &ch.final_curve.freq,
        sample_rate,
    );
    ch.final_curve = crate::response::apply_complex_response(&ch.final_curve, &phase_response);
    chain.final_curve = Some((&ch.final_curve).into());

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

fn apply_phase_only_adjustment_to_reported_curve(
    curve: &mut Curve,
    delay_ms: f64,
    invert_polarity: bool,
) {
    if curve.freq.is_empty() {
        return;
    }

    let base_phase = curve
        .phase
        .clone()
        .unwrap_or_else(|| ndarray::Array1::zeros(curve.freq.len()));
    let inversion_phase = if invert_polarity { 180.0 } else { 0.0 };
    let delay_s = delay_ms / 1000.0;

    let phase =
        ndarray::Array1::from_iter(curve.freq.iter().zip(base_phase.iter()).map(
            |(&freq_hz, &phase_deg)| phase_deg + inversion_phase - (360.0 * freq_hz * delay_s),
        ));
    curve.phase = Some(phase);
}

fn sync_reported_phase_adjustment(
    channel_name: &str,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    delay_ms: f64,
    invert_polarity: bool,
) {
    if let Some(ch_result) = channel_results.get_mut(channel_name) {
        apply_phase_only_adjustment_to_reported_curve(
            &mut ch_result.final_curve,
            delay_ms,
            invert_polarity,
        );

        if let Some(chain) = channel_chains.get_mut(channel_name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    } else if let Some(chain) = channel_chains.get_mut(channel_name)
        && let Some(final_curve) = chain.final_curve.clone()
    {
        let mut curve: Curve = final_curve.into();
        apply_phase_only_adjustment_to_reported_curve(&mut curve, delay_ms, invert_polarity);
        chain.final_curve = Some((&curve).into());
    }
}

fn sync_reported_gain_adjustment(
    channel_name: &str,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    gain_db: f64,
    invert_polarity: bool,
) {
    if let Some(ch_result) = channel_results.get_mut(channel_name) {
        ch_result.final_curve.spl = &ch_result.final_curve.spl + gain_db;
        apply_phase_only_adjustment_to_reported_curve(
            &mut ch_result.final_curve,
            0.0,
            invert_polarity,
        );

        if let Some(chain) = channel_chains.get_mut(channel_name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    } else if let Some(chain) = channel_chains.get_mut(channel_name)
        && let Some(final_curve) = chain.final_curve.clone()
    {
        let mut curve: Curve = final_curve.into();
        curve.spl = &curve.spl + gain_db;
        apply_phase_only_adjustment_to_reported_curve(&mut curve, 0.0, invert_polarity);
        chain.final_curve = Some((&curve).into());
    }
}

fn sync_reported_biquad_adjustment(
    channel_name: &str,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    filters: &[Biquad],
    sample_rate: f64,
) {
    if filters.is_empty() {
        return;
    }

    if let Some(ch_result) = channel_results.get_mut(channel_name) {
        let response = crate::response::compute_peq_complex_response(
            filters,
            &ch_result.final_curve.freq,
            sample_rate,
        );
        ch_result.final_curve =
            crate::response::apply_complex_response(&ch_result.final_curve, &response);

        if let Some(chain) = channel_chains.get_mut(channel_name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    } else if let Some(chain) = channel_chains.get_mut(channel_name)
        && let Some(final_curve) = chain.final_curve.clone()
    {
        let curve: Curve = final_curve.into();
        let response =
            crate::response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
        let corrected = crate::response::apply_complex_response(&curve, &response);
        chain.final_curve = Some((&corrected).into());
    }
}

fn sync_reported_fir_adjustment(
    channel_name: &str,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    coeffs: &[f64],
    sample_rate: f64,
) {
    if coeffs.is_empty() {
        return;
    }

    if let Some(ch_result) = channel_results.get_mut(channel_name) {
        let response = crate::response::compute_fir_complex_response(
            coeffs,
            &ch_result.final_curve.freq,
            sample_rate,
        );
        ch_result.final_curve =
            crate::response::apply_complex_response(&ch_result.final_curve, &response);

        if let Some(chain) = channel_chains.get_mut(channel_name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    } else if let Some(chain) = channel_chains.get_mut(channel_name)
        && let Some(final_curve) = chain.final_curve.clone()
    {
        let curve: Curve = final_curve.into();
        let response =
            crate::response::compute_fir_complex_response(coeffs, &curve.freq, sample_rate);
        let corrected = crate::response::apply_complex_response(&curve, &response);
        chain.final_curve = Some((&corrected).into());
    }
}

fn collect_current_final_curves(
    channel_results: &HashMap<String, ChannelOptimizationResult>,
) -> HashMap<String, Curve> {
    channel_results
        .iter()
        .map(|(name, result)| (name.clone(), result.final_curve.clone()))
        .collect()
}

fn total_chain_delay_ms(chain: &ChannelDspChain) -> f64 {
    chain
        .plugins
        .iter()
        .filter(|plugin| plugin.plugin_type == "delay")
        .filter_map(|plugin| {
            plugin
                .parameters
                .get("delay_ms")
                .and_then(|value| value.as_f64())
        })
        .sum()
}

fn compute_phase_alignment_delay_schedule(
    phase_alignment_results: &HashMap<String, (f64, bool, String)>,
) -> HashMap<String, f64> {
    let mut graph: HashMap<String, Vec<(String, f64)>> = HashMap::new();

    for (main_name, (relative_delay_ms, _invert, sub_name)) in phase_alignment_results {
        // The optimizer's delay means: delay(main) - delay(sub) = relative_delay_ms.
        graph
            .entry(sub_name.clone())
            .or_default()
            .push((main_name.clone(), *relative_delay_ms));
        graph
            .entry(main_name.clone())
            .or_default()
            .push((sub_name.clone(), -*relative_delay_ms));
    }

    let mut raw_offsets: HashMap<String, f64> = HashMap::new();
    let mut schedule: HashMap<String, f64> = HashMap::new();

    for start in graph.keys() {
        if raw_offsets.contains_key(start) {
            continue;
        }

        raw_offsets.insert(start.clone(), 0.0);
        let mut stack = vec![start.clone()];
        let mut component = Vec::new();

        while let Some(channel) = stack.pop() {
            component.push(channel.clone());
            let channel_offset = raw_offsets[&channel];

            if let Some(neighbors) = graph.get(&channel) {
                for (neighbor, delta_ms) in neighbors {
                    let neighbor_offset = channel_offset + *delta_ms;
                    if let Some(existing) = raw_offsets.get(neighbor) {
                        if (existing - neighbor_offset).abs() > 0.05 {
                            warn!(
                                "Conflicting phase-alignment delay constraints for '{}': {:.3} ms vs {:.3} ms; keeping first schedule",
                                neighbor, existing, neighbor_offset
                            );
                        }
                    } else {
                        raw_offsets.insert(neighbor.clone(), neighbor_offset);
                        stack.push(neighbor.clone());
                    }
                }
            }
        }

        let min_offset = component
            .iter()
            .filter_map(|name| raw_offsets.get(name))
            .copied()
            .fold(f64::INFINITY, f64::min);

        for name in component {
            if let Some(offset) = raw_offsets.get(&name) {
                let delay_ms = offset - min_offset;
                if delay_ms > 0.01 {
                    schedule.insert(name, delay_ms);
                }
            }
        }
    }

    schedule
}

fn apply_phase_alignment_delay_schedule(
    phase_alignment_results: &HashMap<String, (f64, bool, String)>,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
) -> HashMap<String, f64> {
    let schedule = compute_phase_alignment_delay_schedule(phase_alignment_results);

    for (channel_name, delay_ms) in &schedule {
        let applied = if let Some(chain) = channel_chains.get_mut(channel_name.as_str()) {
            output::add_delay_plugin(chain, *delay_ms);
            true
        } else {
            false
        };

        if applied {
            sync_reported_phase_adjustment(
                channel_name,
                channel_results,
                channel_chains,
                *delay_ms,
                false,
            );
            info!(
                "  Applied {:.3} ms phase alignment delay to '{}'",
                delay_ms, channel_name
            );
        }
    }

    schedule
}

/// Threshold in dB above which to warn about channel level differences
const LEVEL_DIFFERENCE_WARNING_THRESHOLD: f64 = 6.0;

/// Threshold in ms above which to warn about arrival time differences
const ARRIVAL_TIME_WARNING_THRESHOLD_MS: f64 = 50.0;

// ============================================================================
// Sub-Main Pairing Logic
// ============================================================================

fn is_subwoofer_channel(config: &RoomConfig, channel_name: &str) -> bool {
    if let Some(sys) = &config.system
        && let Some(subs) = &sys.subwoofers
    {
        if channel_name.eq_ignore_ascii_case("lfe") {
            return true;
        }

        if let Some(measurement_key) = sys.speakers.get(channel_name) {
            return subs.mapping.contains_key(measurement_key);
        }
    }

    let lower = channel_name.to_lowercase();
    lower == "lfe" || lower == "sub" || lower.starts_with("sub")
}

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
            .find(|name| is_subwoofer_channel(config, name))
            .cloned();
        if let Some(sub_name) = sub_channel {
            let main_channels: Vec<String> = curves
                .keys()
                .filter(|name| *name != &sub_name && !is_subwoofer_channel(config, name))
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

/// I6 — debug-only sanity invariants on the final `RoomOptimizationResult`.
///
/// Catches silent corruption bugs that would otherwise produce garbage DSP
/// chains (misaligned indexing, NaN fallout from the optimiser). A full
/// chain resynthesis would need to simulate every plugin type (gain /
/// delay / biquad / FIR) and reproduce each workflow's intermediate curve
/// derivation — that invariant is deferred to Phase 5. A magnitude-delta
/// envelope was considered but had to be removed: in 2.1 / home-cinema
/// workflows the Sub channel's `final_curve` legitimately reaches
/// −300 dB where the LP crossover attenuates far-above-passband content,
/// which is not a bug.
///
/// Invariants that do hold universally:
///   1. Every channel's `freq` and `spl` lengths match (on both the
///      initial and final curves).
///   2. No NaN or infinite SPL values in the final curve — they signal
///      optimiser divergence.
/// Runs in both debug AND release. Debug panics (via `debug_assert!`) so
/// tests surface the exact violated invariant; release returns a clean
/// `Err` so fuzz / QA runs report divergence instead of shipping a
/// corrupted DSP chain.
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
    let mut config = config.clone();

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
                let mut workflow_refresh_needed = false;

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
                    let names: Vec<String> = result.channel_results.keys().cloned().collect();
                    for name in names {
                        let generated = if let Some(ch) = result.channel_results.get_mut(&name) {
                            if ch.fir_coeffs.is_some() {
                                None
                            } else {
                                ch.fir_coeffs = post_generate_fir(
                                    &name,
                                    &ch.initial_curve,
                                    &ch.final_curve,
                                    &config.optimizer,
                                    config.target_curve.as_ref(),
                                    sample_rate,
                                    Some(out_dir),
                                );
                                ch.fir_coeffs.clone()
                            }
                        } else {
                            None
                        };

                        let Some(coeffs) = generated else {
                            continue;
                        };

                        if let Some(chain) = result.channels.get_mut(&name) {
                            let filename = format!("{}_fir.wav", name);
                            chain
                                .plugins
                                .push(super::output::create_convolution_plugin(&filename));
                        }
                        sync_reported_fir_adjustment(
                            &name,
                            &mut result.channel_results,
                            &mut result.channels,
                            &coeffs,
                            sample_rate,
                        );
                        workflow_refresh_needed = true;
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
                    let names: Vec<String> = result.channel_results.keys().cloned().collect();
                    for name in names {
                        let generated = if let Some(ch) = result.channel_results.get_mut(&name) {
                            if ch.fir_coeffs.is_some() {
                                None
                            } else {
                                ch.fir_coeffs = post_generate_mixed_phase_fir(
                                    &name,
                                    &ch.initial_curve,
                                    &config.optimizer,
                                    sample_rate,
                                    Some(out_dir),
                                );
                                ch.fir_coeffs.clone()
                            }
                        } else {
                            None
                        };

                        let Some(coeffs) = generated else {
                            continue;
                        };

                        if let Some(chain) = result.channels.get_mut(&name) {
                            let filename = format!("{}_excess_phase_fir.wav", name);
                            chain
                                .plugins
                                .push(super::output::create_convolution_plugin(&filename));
                        }
                        sync_reported_fir_adjustment(
                            &name,
                            &mut result.channel_results,
                            &mut result.channels,
                            &coeffs,
                            sample_rate,
                        );
                        workflow_refresh_needed = true;
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
                            let before_plugins = chain.plugins.len();
                            apply_phase_correction(
                                name,
                                ch,
                                chain,
                                pc_config,
                                sample_rate,
                                Some(out_dir),
                            );
                            workflow_refresh_needed |= chain.plugins.len() != before_plugins;
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
                        .map(total_chain_delay_ms)
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
                    let plugin_count_before_icd: usize = result
                        .channels
                        .values()
                        .map(|chain| chain.plugins.len())
                        .sum();
                    compute_and_correct_icd(&mut result, config, sample_rate);
                    let plugin_count_after_icd: usize = result
                        .channels
                        .values()
                        .map(|chain| chain.plugins.len())
                        .sum();
                    workflow_refresh_needed |= plugin_count_after_icd != plugin_count_before_icd;
                }
                if workflow_refresh_needed {
                    refresh_final_reports(&mut result, config, sample_rate);
                }
                update_perceptual_metrics(
                    &mut result.metadata,
                    Some(&result.channels),
                    Some(config),
                );

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
        n_params, population_size, max_iterations, config.optimizer.max_iter, DE_GENERATIONS_FLOOR,
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
        let mut post_generated_fir: Option<Vec<f64>> = None;
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
            let generated = post_generate_fir(
                &channel_name,
                &initial_curve,
                &final_curve,
                &config.optimizer,
                config.target_curve.as_ref(),
                sample_rate,
                output_dir,
            );
            post_generated_fir = generated.clone();
            generated
        } else {
            fir_coeffs
        };

        channel_results.insert(
            channel_name.clone(),
            ChannelOptimizationResult {
                name: channel_name.clone(),
                pre_score,
                post_score,
                initial_curve,
                final_curve,
                biquads,
                fir_coeffs,
            },
        );

        if let Some(coeffs) = post_generated_fir {
            if let Some(chain) = channel_chains.get_mut(&channel_name) {
                let filename = format!("{}_fir.wav", channel_name);
                chain
                    .plugins
                    .push(super::output::create_convolution_plugin(&filename));
            }
            sync_reported_fir_adjustment(
                &channel_name,
                &mut channel_results,
                &mut channel_chains,
                &coeffs,
                sample_rate,
            );
        }
    }

    curves = collect_current_final_curves(&channel_results);

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
            let applied = if *delay_ms > 0.01
                && let Some(chain) = channel_chains.get_mut(channel_name)
            {
                chain
                    .plugins
                    .insert(0, output::create_delay_plugin(*delay_ms));
                true
            } else {
                false
            };

            if applied {
                sync_reported_phase_adjustment(
                    channel_name,
                    &mut channel_results,
                    &mut channel_chains,
                    *delay_ms,
                    false,
                );
                info!(
                    "  Channel '{}': added {:.3} ms delay for time alignment",
                    channel_name, delay_ms
                );
            }
        }
        curves = collect_current_final_curves(&channel_results);
    } else if channel_arrivals.is_empty() && config.speakers.len() > 1 {
        info!("No arrival time data (WAV or phase) available for time alignment. Skipping.");
    }

    // Spectral channel alignment: fit low-shelf + high-shelf + flat gain to each
    // channel's deviation from the average post-EQ curve. This corrects both broadband
    // level differences and frequency-dependent tilt between channels.
    let spectral_curves: HashMap<String, Curve> = curves
        .iter()
        .filter(|(name, _)| !is_subwoofer_channel(config, name))
        .map(|(name, curve)| (name.clone(), curve.clone()))
        .collect();
    if spectral_curves.len() > 1 {
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
        for (channel_name, final_curve) in &spectral_curves {
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
            &spectral_curves,
            sample_rate,
            min_freq,
            max_freq,
        );
        super::spectral_align::log_spectral_alignment(&alignment_results);

        // Insert alignment plugins after the per-channel PEQ.
        //
        // Shelves and flat-gain are gated independently: a constant SPL shift
        // can never change flatness (the score normalizes by mean), so a
        // legitimate inter-channel level correction must not be discarded just
        // because the shelves would regress flatness.
        for (channel_name, result) in &alignment_results {
            let shelf_filters =
                super::spectral_align::create_alignment_filters(result, sample_rate);

            let (apply_shelves, apply_gain) = if channel_results.contains_key(channel_name) {
                let (score_min, score_max) = final_score_band_for_channel(config, channel_name);
                let shelves_ok = should_apply_spectral_shelves(
                    &curves,
                    channel_name,
                    &shelf_filters,
                    sample_rate,
                    score_min,
                    score_max,
                );

                let gain_ok = result.flat_gain_db.abs() >= super::spectral_align::MIN_CORRECTION_DB;
                (shelves_ok, gain_ok)
            } else {
                (false, false)
            };

            if !apply_shelves && !apply_gain {
                continue;
            }

            if let Some(chain) = channel_chains.get_mut(channel_name) {
                let (eq_plugin, gain_plugin) =
                    super::spectral_align::create_alignment_plugins(result, sample_rate);
                if apply_shelves && let Some(eq) = eq_plugin {
                    chain.plugins.push(eq);
                }
                if apply_gain && let Some(gain) = gain_plugin {
                    chain.plugins.push(gain);
                }
            }

            if apply_shelves {
                sync_reported_biquad_adjustment(
                    channel_name,
                    &mut channel_results,
                    &mut channel_chains,
                    &shelf_filters,
                    sample_rate,
                );
            }
            if apply_gain {
                sync_reported_gain_adjustment(
                    channel_name,
                    &mut channel_results,
                    &mut channel_chains,
                    result.flat_gain_db,
                    false,
                );
            }
        }
        curves = collect_current_final_curves(&channel_results);
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
                    if let Some(alignment) = &vog_result.alignment {
                        let shelf_filters =
                            super::spectral_align::create_alignment_filters(alignment, sample_rate);
                        sync_reported_biquad_adjustment(
                            channel_name,
                            &mut channel_results,
                            &mut channel_chains,
                            &shelf_filters,
                            sample_rate,
                        );
                        if alignment.flat_gain_db.abs() >= super::spectral_align::MIN_CORRECTION_DB
                        {
                            sync_reported_gain_adjustment(
                                channel_name,
                                &mut channel_results,
                                &mut channel_chains,
                                alignment.flat_gain_db,
                                false,
                            );
                        }
                    }
                }
                curves = collect_current_final_curves(&channel_results);
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
    // (delay_ms, invert_polarity, sub_name) keyed by main speaker name
    let mut phase_alignment_results: HashMap<String, (f64, bool, String)> = HashMap::new();

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
                                    (result.delay_ms, result.invert_polarity, sub_name.clone()),
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

    // Apply phase alignment results. The optimizer returns pairwise relative
    // delays, so resolve all pairs into one absolute non-negative delay schedule
    // before inserting DSP plugins.
    for (speaker_name, (delay_ms, invert, sub_name)) in &phase_alignment_results {
        if *invert {
            let applied = if let Some(chain) = channel_chains.get_mut(speaker_name) {
                // Insert polarity inversion at the beginning of the chain
                let invert_plugin = output::create_gain_plugin_with_invert(0.0, true);
                chain.plugins.insert(0, invert_plugin);
                true
            } else {
                false
            };

            if applied {
                sync_reported_phase_adjustment(
                    speaker_name,
                    &mut channel_results,
                    &mut channel_chains,
                    0.0,
                    true,
                );
                info!("  Applied polarity inversion to '{}'", speaker_name);
            }
        }

        debug!(
            "  Phase alignment constraint: delay('{}') - delay('{}') = {:.3} ms",
            speaker_name, sub_name, delay_ms
        );
    }

    apply_phase_alignment_delay_schedule(
        &phase_alignment_results,
        &mut channel_results,
        &mut channel_chains,
    );
    if !phase_alignment_results.is_empty() {
        curves = collect_current_final_curves(&channel_results);
    }

    // Group Delay Optimization (GD-Opt v1) was removed in the 2.0 simplification
    // pass. A redesigned v2 lives in docs/gd_opt_v2_plan.md and is scheduled for a
    // future release; it will plug in here.

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

    // ─── GD-Opt v2 integration (Phase GD-5) ──────────────────────────────
    // Run after all earlier phase/EQ stages have updated final_curve, but
    // before IR/EPA/metadata so exported reports reflect the audible chain.
    let group_delay_summary = try_run_gd_opt(
        config,
        &mut channel_results,
        &mut channel_chains,
        sample_rate,
    );

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
            .map(total_chain_delay_ms)
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
        group_delay: group_delay_summary,
        perceptual_metrics: None,
        home_cinema_layout: None,
        multi_seat_coverage: None,
        multi_seat_correction: None,
        bass_management: None,
        timing_diagnostics: build_timing_diagnostics(config, &channel_arrivals, &channel_chains),
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
    refresh_final_reports(&mut result, config, sample_rate);

    sanity_check_result(&result)?;
    Ok(result)
}

fn recompute_curve_flatness_score(curve: &Curve, min_freq: f64, max_freq: f64) -> f64 {
    let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();
    let mean = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;
    let normalized_spl = &curve.spl - mean;
    crate::loss::flat_loss(&curve.freq, &normalized_spl, min_freq, max_freq)
}

fn should_apply_spectral_shelves(
    current_curves: &HashMap<String, Curve>,
    channel_name: &str,
    shelf_filters: &[Biquad],
    sample_rate: f64,
    score_min: f64,
    score_max: f64,
) -> bool {
    if shelf_filters.is_empty() {
        return false;
    }

    let Some(curve) = current_curves.get(channel_name) else {
        return false;
    };

    let response =
        crate::response::compute_peq_complex_response(shelf_filters, &curve.freq, sample_rate);
    let corrected = crate::response::apply_complex_response(curve, &response);
    let flatness_before = recompute_curve_flatness_score(curve, score_min, score_max);
    let flatness_after = recompute_curve_flatness_score(&corrected, score_min, score_max);
    let flatness_regression = (flatness_after - flatness_before).max(0.0);

    let icd_before =
        super::spectral_align::compute_inter_channel_deviation(current_curves, score_min);
    if icd_before.deviation_per_freq.is_empty() {
        return false;
    }

    let mut corrected_curves = current_curves.clone();
    corrected_curves.insert(channel_name.to_string(), corrected);
    let icd_after =
        super::spectral_align::compute_inter_channel_deviation(&corrected_curves, score_min);
    if icd_after.deviation_per_freq.is_empty() {
        return false;
    }

    let icd_improvement = icd_before.passband_rms_db - icd_after.passband_rms_db;
    icd_improvement > flatness_regression + 1e-6
}

fn final_score_band_for_channel(config: &RoomConfig, channel_name: &str) -> (f64, f64) {
    let min_freq = config.optimizer.min_freq;
    let mut max_freq = config.optimizer.max_freq;
    if config.system.is_none() {
        return (min_freq, max_freq.max(min_freq));
    }
    let crossover_max = config.crossovers.as_ref().and_then(|xos| {
        xos.values()
            .filter_map(|xo| xo.frequency)
            .filter(|freq| freq.is_finite() && *freq > 0.0)
            .reduce(f64::max)
    });

    if is_subwoofer_channel(config, channel_name) {
        let crossover_max = crossover_max.unwrap_or(160.0);
        max_freq = max_freq.min((crossover_max * 2.0).clamp(120.0, 250.0));
    } else if config
        .system
        .as_ref()
        .is_some_and(|sys| sys.subwoofers.is_some())
    {
        let crossover_max = crossover_max.unwrap_or(80.0);
        return (
            min_freq.max(crossover_max),
            max_freq.max(min_freq.max(crossover_max)),
        );
    } else {
        let (role_min, role_max) = super::home_cinema::role_score_band(config, channel_name);
        return (role_min, role_max.max(role_min));
    }

    (min_freq, max_freq.max(min_freq))
}

fn refresh_final_reports(
    result: &mut RoomOptimizationResult,
    config: &RoomConfig,
    sample_rate: f64,
) {
    for ch_result in result.channel_results.values_mut() {
        let (score_min_freq, score_max_freq) =
            final_score_band_for_channel(config, &ch_result.name);
        ch_result.post_score =
            recompute_curve_flatness_score(&ch_result.final_curve, score_min_freq, score_max_freq);
        if let Some(chain) = result.channels.get_mut(&ch_result.name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    }

    let count = result.channel_results.len().max(1) as f64;
    let avg_pre = result
        .channel_results
        .values()
        .map(|ch| ch.pre_score)
        .sum::<f64>()
        / count;
    let avg_post = result
        .channel_results
        .values()
        .map(|ch| ch.post_score)
        .sum::<f64>()
        / count;
    result.combined_pre_score = avg_pre;
    result.combined_post_score = avg_post;
    result.metadata.pre_score = avg_pre;
    result.metadata.post_score = avg_post;
    result.metadata.home_cinema_layout = Some(super::home_cinema::analyze_layout(config));
    result.metadata.multi_seat_coverage = Some(super::home_cinema::multi_seat_coverage(config));
    let existing_bass_management = result.metadata.bass_management.clone();
    result.metadata.bass_management = if let Some(existing) = existing_bass_management {
        super::home_cinema::bass_management_report_with_optimization_and_sample_rate(
            config,
            existing.applied_sub_gain_db,
            existing.gain_limited,
            existing.optimization,
            sample_rate,
        )
    } else {
        super::home_cinema::bass_management_report(config, None, false)
    };

    let epa_cfg = config.optimizer.epa_config.clone().unwrap_or_default();
    result.metadata.epa_per_channel =
        crate::roomeq::output::compute_epa_per_channel(&result.channels, &epa_cfg);
    update_perceptual_metrics(&mut result.metadata, Some(&result.channels), Some(config));

    let ir_inputs: Vec<_> = result
        .channel_results
        .iter()
        .map(|(name, ch)| {
            let delay_ms = result
                .channels
                .get(name)
                .map(total_chain_delay_ms)
                .unwrap_or(0.0);
            (
                name.clone(),
                ch.initial_curve.clone(),
                ch.biquads.clone(),
                ch.fir_coeffs.clone(),
                delay_ms,
            )
        })
        .collect();

    for (channel_name, initial_curve, biquads, fir_coeffs, delay_ms) in ir_inputs {
        if let Some((pre_ir, post_ir)) = super::ir_waveform::compute_channel_ir_waveforms(
            &initial_curve,
            &biquads,
            fir_coeffs.as_deref(),
            delay_ms,
            sample_rate,
        ) && let Some(chain) = result.channels.get_mut(&channel_name)
        {
            chain.pre_ir = Some(pre_ir);
            chain.post_ir = Some(post_ir);
        }
    }
}

fn build_timing_diagnostics(
    config: &RoomConfig,
    arrivals_ms: &HashMap<String, f64>,
    chains: &HashMap<String, ChannelDspChain>,
) -> Option<super::home_cinema::TimingDiagnosticsReport> {
    if arrivals_ms.is_empty() {
        return None;
    }

    let mut channels = Vec::new();
    for (name, arrival_ms) in arrivals_ms {
        let applied_delay_ms = chains.get(name).map(total_chain_delay_ms).unwrap_or(0.0);
        let final_arrival_ms = arrival_ms + applied_delay_ms;
        channels.push(super::home_cinema::ChannelTimingReport {
            name: name.clone(),
            role: super::home_cinema::role_for_channel(name),
            measured_arrival_ms: *arrival_ms,
            acoustic_distance_m: arrival_ms * 0.343,
            applied_delay_ms,
            final_arrival_ms,
            final_offset_from_reference_ms: 0.0,
        });
    }
    channels.sort_by(|a, b| a.name.cmp(&b.name));

    let before_values: Vec<f64> = channels
        .iter()
        .map(|channel| channel.measured_arrival_ms)
        .collect();
    let after_values: Vec<f64> = channels
        .iter()
        .map(|channel| channel.final_arrival_ms)
        .collect();
    let arrival_spread_before_ms = spread(&before_values).unwrap_or(0.0);
    let arrival_spread_after_ms = spread(&after_values).unwrap_or(0.0);
    let reference_arrival_ms = after_values.iter().copied().reduce(f64::max);
    let reference_channel = reference_arrival_ms.and_then(|reference| {
        channels
            .iter()
            .find(|channel| (channel.final_arrival_ms - reference).abs() < 1e-6)
            .map(|channel| channel.name.clone())
    });
    if let Some(reference) = reference_arrival_ms {
        for channel in &mut channels {
            channel.final_offset_from_reference_ms = channel.final_arrival_ms - reference;
        }
    }

    let mut advisories = Vec::new();
    if arrival_spread_before_ms > ARRIVAL_TIME_WARNING_THRESHOLD_MS {
        advisories.push("large_measured_arrival_spread".to_string());
    }
    if arrival_spread_after_ms > 0.5 {
        advisories.push("post_dsp_arrivals_not_aligned".to_string());
    }
    if let Some(lcr_advisory) = lcr_timing_advisory(&channels) {
        advisories.push(lcr_advisory);
    }
    if surround_or_height_precedence_risk(&channels) {
        advisories.push("surround_or_height_precedence_risk".to_string());
    }
    if advisories.is_empty() {
        advisories.push("ok".to_string());
    }

    let _ = config;
    Some(super::home_cinema::TimingDiagnosticsReport {
        reference_channel,
        reference_arrival_ms,
        arrival_spread_before_ms,
        arrival_spread_after_ms,
        channels,
        advisories,
    })
}

fn lcr_timing_advisory(channels: &[super::home_cinema::ChannelTimingReport]) -> Option<String> {
    let front_or_center: Vec<_> = channels
        .iter()
        .filter(|channel| {
            matches!(
                channel.role,
                super::home_cinema::HomeCinemaRole::FrontLeft
                    | super::home_cinema::HomeCinemaRole::FrontRight
                    | super::home_cinema::HomeCinemaRole::Center
            )
        })
        .collect();
    if front_or_center.len() < 2 {
        return None;
    }
    let values: Vec<f64> = front_or_center
        .iter()
        .map(|channel| channel.final_arrival_ms)
        .collect();
    if spread(&values).unwrap_or(0.0) > 0.5 {
        Some("lcr_imaging_timing_spread".to_string())
    } else {
        None
    }
}

fn surround_or_height_precedence_risk(
    channels: &[super::home_cinema::ChannelTimingReport],
) -> bool {
    let front_reference = channels
        .iter()
        .filter(|channel| {
            matches!(
                channel.role,
                super::home_cinema::HomeCinemaRole::FrontLeft
                    | super::home_cinema::HomeCinemaRole::FrontRight
                    | super::home_cinema::HomeCinemaRole::Center
            )
        })
        .map(|channel| channel.final_arrival_ms)
        .reduce(f64::min);
    let Some(front_reference) = front_reference else {
        return false;
    };
    channels.iter().any(|channel| {
        let surround_or_height = matches!(
            channel.role,
            super::home_cinema::HomeCinemaRole::SideSurroundLeft
                | super::home_cinema::HomeCinemaRole::SideSurroundRight
                | super::home_cinema::HomeCinemaRole::RearSurroundLeft
                | super::home_cinema::HomeCinemaRole::RearSurroundRight
                | super::home_cinema::HomeCinemaRole::WideLeft
                | super::home_cinema::HomeCinemaRole::WideRight
        ) || channel.role.is_height();
        surround_or_height && channel.final_arrival_ms + 0.5 < front_reference
    })
}

fn spread(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let min = values.iter().copied().reduce(f64::min)?;
    let max = values.iter().copied().reduce(f64::max)?;
    Some(max - min)
}

fn update_perceptual_metrics(
    metadata: &mut OptimizationMetadata,
    channels: Option<&HashMap<String, ChannelDspChain>>,
    config: Option<&RoomConfig>,
) {
    let Some(epa_per_channel) = metadata.epa_per_channel.as_ref() else {
        metadata.perceptual_metrics = None;
        return;
    };
    if epa_per_channel.is_empty() {
        metadata.perceptual_metrics = None;
        return;
    }

    let count = epa_per_channel.len() as f64;
    let epa_preference_pre = epa_per_channel
        .values()
        .map(|metrics| metrics.pre.preference)
        .sum::<f64>()
        / count;
    let epa_preference_post = epa_per_channel
        .values()
        .map(|metrics| metrics.post.preference)
        .sum::<f64>()
        / count;
    let channel_matching_midrange_rms_db = metadata
        .inter_channel_deviation
        .as_ref()
        .map(|icd| icd.midrange_rms_db);
    let role_channel_matching_rms_db = channels.and_then(role_channel_matching_rms_db);
    let bass_consistency_rms_db = channels.and_then(bass_consistency_rms_db);
    let dialog_band_roughness_rms_db = channels.and_then(dialog_band_roughness_rms_db);
    let headroom_peak_boost_db = channels.and_then(headroom_peak_boost_db);
    let headroom_risk = headroom_peak_boost_db.map(|peak_boost| {
        let margin_db = config
            .and_then(|cfg| cfg.system.as_ref())
            .and_then(|system| system.bass_management.as_ref())
            .map(|bm| bm.headroom_margin_db)
            .unwrap_or(6.0);
        if peak_boost > margin_db {
            "high_boost_exceeds_headroom_margin".to_string()
        } else if peak_boost > margin_db * 0.5 {
            "moderate_boost_uses_headroom".to_string()
        } else {
            "ok".to_string()
        }
    });
    let timing_confidence = metadata.group_delay.as_ref().map(|gd| {
        if gd.applied {
            "gd_applied".to_string()
        } else if gd.advisory == "success" {
            "gd_success_not_applied".to_string()
        } else {
            format!("gd_{}", gd.advisory)
        }
    });

    metadata.perceptual_metrics = Some(PerceptualMetrics {
        epa_preference_pre,
        epa_preference_post,
        epa_preference_delta: epa_preference_post - epa_preference_pre,
        channel_matching_midrange_rms_db,
        role_channel_matching_rms_db,
        bass_consistency_rms_db,
        dialog_band_roughness_rms_db,
        headroom_peak_boost_db,
        headroom_risk,
        timing_confidence,
    });
}

fn role_channel_matching_rms_db(channels: &HashMap<String, ChannelDspChain>) -> Option<f64> {
    let mut grouped: HashMap<&'static str, Vec<&ChannelDspChain>> = HashMap::new();
    for (name, chain) in channels {
        if let Some(key) = channel_matching_role_key(name) {
            grouped.entry(key).or_default().push(chain);
        }
    }

    let mut group_rms = Vec::new();
    for group in grouped.values() {
        if group.len() < 2 {
            continue;
        }
        if let Some(rms) = group_mean_deviation_rms_db(group, (300.0, 4_000.0)) {
            group_rms.push(rms);
        }
    }
    mean(&group_rms)
}

fn bass_consistency_rms_db(channels: &HashMap<String, ChannelDspChain>) -> Option<f64> {
    let bass_channels: Vec<&ChannelDspChain> = channels
        .iter()
        .filter_map(|(name, chain)| {
            if super::home_cinema::role_for_channel(name).is_sub_or_lfe() {
                Some(chain)
            } else {
                None
            }
        })
        .collect();
    if bass_channels.len() < 2 {
        return None;
    }
    group_mean_deviation_rms_db(&bass_channels, (20.0, 160.0))
}

fn dialog_band_roughness_rms_db(channels: &HashMap<String, ChannelDspChain>) -> Option<f64> {
    let center = channels.iter().find_map(|(name, chain)| {
        if super::home_cinema::role_for_channel(name) == super::home_cinema::HomeCinemaRole::Center
        {
            Some(chain)
        } else {
            None
        }
    })?;
    curve_roughness_rms_db(center.final_curve.as_ref()?, (300.0, 4_000.0))
}

fn headroom_peak_boost_db(channels: &HashMap<String, ChannelDspChain>) -> Option<f64> {
    let mut peak = 0.0_f64;
    let mut saw_plugin = false;
    for chain in channels.values() {
        for plugin in &chain.plugins {
            if plugin.plugin_type == "gain" {
                if let Some(gain_db) = plugin.parameters.get("gain_db").and_then(|v| v.as_f64()) {
                    peak = peak.max(gain_db);
                    saw_plugin = true;
                }
            } else if plugin.plugin_type == "eq"
                && let Some(filters) = plugin.parameters.get("filters").and_then(|v| v.as_array())
            {
                for filter in filters {
                    if let Some(gain_db) = filter.get("db_gain").and_then(|v| v.as_f64()) {
                        peak = peak.max(gain_db);
                        saw_plugin = true;
                    }
                }
            }
        }
    }
    if saw_plugin { Some(peak) } else { None }
}

fn group_mean_deviation_rms_db(channels: &[&ChannelDspChain], band: (f64, f64)) -> Option<f64> {
    let reference = channels.first()?.final_curve.as_ref()?;
    if channels.iter().any(|chain| {
        chain.final_curve.as_ref().is_none_or(|curve| {
            curve.freq.len() != reference.freq.len()
                || curve.freq.iter().zip(reference.freq.iter()).any(|(a, b)| {
                    let scale = a.abs().max(b.abs()).max(1.0);
                    (a - b).abs() > scale * 1e-6
                })
        })
    }) {
        return None;
    }

    let mut deviations = Vec::new();
    for idx in 0..reference.freq.len() {
        let freq = reference.freq[idx];
        if freq < band.0 || freq > band.1 {
            continue;
        }
        let values: Vec<f64> = channels
            .iter()
            .filter_map(|chain| chain.final_curve.as_ref().map(|curve| curve.spl[idx]))
            .collect();
        let Some(avg) = mean(&values) else {
            continue;
        };
        deviations.extend(values.into_iter().map(|value| value - avg));
    }
    rms(&deviations)
}

fn curve_roughness_rms_db(curve: &super::types::CurveData, band: (f64, f64)) -> Option<f64> {
    let values: Vec<f64> = curve
        .freq
        .iter()
        .zip(curve.spl.iter())
        .filter_map(|(freq, spl)| {
            if *freq >= band.0 && *freq <= band.1 {
                Some(*spl)
            } else {
                None
            }
        })
        .collect();
    let avg = mean(&values)?;
    let deviations: Vec<f64> = values.into_iter().map(|value| value - avg).collect();
    rms(&deviations)
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn rms(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some((values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt())
    }
}

fn channel_matching_role_key(channel_name: &str) -> Option<&'static str> {
    super::home_cinema::matching_group_key(channel_name)
}

fn role_aware_channel_matching_groups(
    final_curves: &HashMap<String, crate::Curve>,
) -> Vec<HashMap<String, crate::Curve>> {
    let mut grouped: HashMap<&'static str, HashMap<String, crate::Curve>> = HashMap::new();

    for (name, curve) in final_curves {
        if let Some(key) = channel_matching_role_key(name) {
            grouped
                .entry(key)
                .or_default()
                .insert(name.clone(), curve.clone());
        }
    }

    let order = [
        "front_lr",
        "side_surrounds",
        "rear_surrounds",
        "wides",
        "top_front",
        "top_middle",
        "top_rear",
        "generic",
    ];

    order
        .iter()
        .filter_map(|key| grouped.remove(key))
        .filter(|group| group.len() > 1)
        .collect()
}

fn apply_channel_matching_correction(
    result: &mut RoomOptimizationResult,
    correction: &super::spectral_align::ChannelMatchingResult,
    sample_rate: f64,
) {
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

        if let Some(chain) = result.channels.get_mut(&correction.channel_name) {
            chain.plugins.push(plugin.clone());
        }

        if let Some(ch_result) = result.channel_results.get_mut(&correction.channel_name) {
            let resp = crate::response::compute_peq_complex_response(
                &correction.filters,
                &ch_result.final_curve.freq,
                sample_rate,
            );
            ch_result.final_curve =
                crate::response::apply_complex_response(&ch_result.final_curve, &resp);

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

fn channel_matching_worsens_reported_scores(
    result: &RoomOptimizationResult,
    config: &RoomConfig,
    baseline: &HashMap<String, ChannelOptimizationResult>,
) -> Option<(String, f64, f64)> {
    result.channel_results.iter().find_map(|(name, ch)| {
        let before = baseline.get(name)?.post_score;
        let (score_min, score_max) = final_score_band_for_channel(config, name);
        let after = recompute_curve_flatness_score(&ch.final_curve, score_min, score_max);
        if after > before + 1e-6 {
            Some((name.clone(), before, after))
        } else {
            None
        }
    })
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
        .filter(|(name, _)| !is_subwoofer_channel(config, name))
        .map(|(name, ch)| (name.clone(), ch.final_curve.clone()))
        .collect();
    if final_curves.len() <= 1 {
        result.metadata.inter_channel_deviation = None;
        return;
    }

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

    if enabled {
        let matching_groups = role_aware_channel_matching_groups(&final_curves);
        let mut applied_any = false;
        let baseline_channel_results = result.channel_results.clone();
        let baseline_channels = result.channels.clone();

        if matching_groups.is_empty() {
            info!("No role-compatible channel matching groups found; skipping ICD correction");
        }

        for group in matching_groups {
            let mut group_names: Vec<_> = group.keys().cloned().collect();
            group_names.sort();

            let group_icd = super::spectral_align::compute_inter_channel_deviation(&group, f3);
            if group_icd.midrange_rms_db <= threshold {
                info!(
                    "ICD group [{}] midrange_rms={:.2}dB <= threshold={:.1}dB - no correction needed",
                    group_names.join(", "),
                    group_icd.midrange_rms_db,
                    threshold,
                );
                continue;
            }

            info!(
                "ICD group [{}] midrange_rms={:.2}dB > threshold={:.1}dB - applying role-aware channel matching (max {} filters/ch)",
                group_names.join(", "),
                group_icd.midrange_rms_db,
                threshold,
                max_filters,
            );

            let corrections = super::spectral_align::correct_inter_channel_deviation(
                &group,
                f3,
                max_filters,
                sample_rate,
            );

            for correction in &corrections {
                if correction.plugin.is_some() {
                    apply_channel_matching_correction(result, correction, sample_rate);
                    applied_any = true;
                }
            }
        }

        if !applied_any {
            result.metadata.inter_channel_deviation = Some(icd);
            return;
        }

        if let Some((channel_name, before, after)) =
            channel_matching_worsens_reported_scores(result, config, &baseline_channel_results)
        {
            info!(
                "ICD correction discarded: channel '{}' score would regress from {:.4} to {:.4}",
                channel_name, before, after
            );
            result.channel_results = baseline_channel_results;
            result.channels = baseline_channels;
            result.metadata.inter_channel_deviation = Some(icd);
            return;
        }

        // Re-compute global ICD after role-aware correction.
        let corrected_curves: HashMap<String, crate::Curve> = result
            .channel_results
            .iter()
            .filter(|(name, _)| !is_subwoofer_channel(config, name))
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
    let optimizer_config = optimizer_config.clone();

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

// ─── GD-Opt v2 integration (Phase GD-5) ──────────────────────────────────────

/// Attempt to run GD-Opt v2 on the channel results.
///
/// Returns `Some(GroupDelayOptSummary)` if GD-Opt was attempted (success or
/// advisory skip), `None` if fewer than 2 channels have phase data.
fn try_run_gd_opt(
    config: &RoomConfig,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    sample_rate: f64,
) -> Option<super::gd_opt::GroupDelayOptSummary> {
    use super::gd_opt::*;

    let gd_user_config = config.optimizer.group_delay.as_ref()?;
    if !gd_user_config.enabled {
        return None;
    }

    // Collect channels with phase data from the current post-DSP curves.
    // Sort by name for deterministic ordering (HashMap iteration is arbitrary).
    let mut sorted_channels: Vec<(&String, &ChannelOptimizationResult)> =
        channel_results.iter().collect();
    sorted_channels.sort_by_key(|(name, _)| (*name).clone());

    let mut gd_channels: Vec<ChannelMeasurementInput> = Vec::new();
    let mut gd_channel_names: Vec<String> = Vec::new();
    let mut missing_coherence = false;

    for (name, ch) in &sorted_channels {
        // Curve.phase is in degrees — convert to radians for GD computation
        let phase = match ch.final_curve.phase.as_ref() {
            Some(p) => p.mapv(|deg| deg.to_radians()),
            None => continue, // skip channels without phase
        };

        // Coherence is a measurement-confidence signal, so carry it forward
        // from the measurement when the final curve lost metadata during DSP.
        let coherence = ch
            .final_curve
            .coherence
            .clone()
            .or_else(|| ch.initial_curve.coherence.clone())
            .unwrap_or_else(|| {
                missing_coherence = true;
                ndarray::Array1::from_elem(ch.final_curve.freq.len(), 1.0)
            });

        gd_channels.push(ChannelMeasurementInput {
            freq: ch.final_curve.freq.clone(),
            spl: ch.final_curve.spl.clone(),
            phase,
            coherence,
        });
        gd_channel_names.push((*name).clone());
    }

    // Need at least 2 channels with phase
    if gd_channels.len() < 2 {
        if !channel_results.is_empty() && channel_results.len() >= 2 {
            // Had enough channels but not enough phase data
            return Some(GroupDelayOptSummary::from_advisory(
                &GdOptAdvisory::NoPhaseData,
            ));
        }
        return None;
    }

    // Derive band from crossover config or use default (80 Hz XO assumption)
    let crossover_freq = config
        .crossovers
        .as_ref()
        .and_then(|xos| xos.values().filter_map(|xo| xo.frequency).reduce(f64::min))
        .unwrap_or(80.0);

    let band = derive_band(config.optimizer.min_freq, crossover_freq);

    // Validate consistent grid lengths and values across channels
    let n_freq = gd_channels[0].freq.len();
    for ch in &gd_channels[1..] {
        if ch.freq.len() != n_freq {
            info!("GD-Opt: skipped — inconsistent frequency grid lengths across channels");
            return Some(GroupDelayOptSummary::from_advisory(
                &GdOptAdvisory::FrequencyGridMismatch,
            ));
        }
        if !super::frequency_grid::same_frequency_grid(&gd_channels[0].freq, &ch.freq) {
            info!("GD-Opt: skipped — inconsistent frequency grid values across channels");
            return Some(GroupDelayOptSummary::from_advisory(
                &GdOptAdvisory::FrequencyGridMismatch,
            ));
        }
    }

    // Check that band is non-empty in the data
    let band_count = (0..n_freq)
        .filter(|&i| gd_channels[0].freq[i] >= band.0 && gd_channels[0].freq[i] <= band.1)
        .count();

    if band_count < 3 {
        return Some(GroupDelayOptSummary::from_advisory(
            &GdOptAdvisory::EmptyBand,
        ));
    }

    // Check mean coherence
    let mean_coh: f64 = gd_channels
        .iter()
        .flat_map(|ch| {
            (0..n_freq)
                .filter(|&i| ch.freq[i] >= band.0 && ch.freq[i] <= band.1)
                .map(|i| ch.coherence[i])
        })
        .sum::<f64>()
        / (gd_channels.len() * band_count) as f64;

    if !missing_coherence && mean_coh < gd_user_config.coherence_threshold {
        return Some(GroupDelayOptSummary::from_advisory(
            &GdOptAdvisory::CoherenceBelowThreshold {
                mean_coherence: mean_coh,
            },
        ));
    }

    // Configure and run.
    // AP frequency range is clamped to [20, 500] intersected with the band.
    // If the range is degenerate (min >= max), disable AP filters.
    let ap_min_freq = band.0.max(20.0);
    let ap_max_freq = band.1.min(500.0);
    let (mut ap_per_channel, ap_min_freq, ap_max_freq) = if ap_min_freq < ap_max_freq {
        (gd_user_config.ap_per_channel, ap_min_freq, ap_max_freq)
    } else {
        // AP range is empty — run delay-only
        (0, 20.0, 300.0)
    };
    let mut advisory_override: Option<GdOptAdvisory> = None;
    let mut optimize_polarity = gd_user_config.optimize_polarity;

    if missing_coherence {
        ap_per_channel = 0;
        optimize_polarity = false;
        advisory_override = Some(GdOptAdvisory::MissingCoherenceDelayOnly);
    } else if gd_user_config.adaptive_allpass && ap_per_channel > 0 {
        // Production currently has only the averaged measurement, not
        // independent sweep realisations. Avoid single-measurement AP overfit.
        ap_per_channel = 0;
        advisory_override = Some(GdOptAdvisory::AllPassDisabledNoBootstrapRealisations);
    }

    let gd_config = GdOptConfig {
        sample_rate,
        max_delay_ms: gd_user_config.max_delay_ms,
        ap_per_channel,
        ap_min_freq,
        ap_max_freq,
        ap_min_q: gd_user_config.ap_min_q,
        ap_max_q: gd_user_config.ap_max_q,
        optimize_polarity,
        max_iter: gd_user_config.max_iter,
        popsize: gd_user_config.popsize,
        tol: gd_user_config.tol,
        seed: config.optimizer.seed,
    };

    let result = optimize_group_delay_for_mode(
        &gd_channels,
        band,
        &gd_config,
        &config.optimizer.processing_mode,
        config.optimizer.mixed_config.as_ref(),
    );

    match result {
        Ok(gd_result) => {
            if gd_result.improvement_db < gd_user_config.min_improvement_db {
                info!(
                    "GD-Opt: minimal improvement ({:.1} dB), skipping",
                    gd_result.improvement_db
                );
                Some(GroupDelayOptSummary::from_advisory(
                    &GdOptAdvisory::MinimalImprovement {
                        improvement_db: gd_result.improvement_db,
                    },
                ))
            } else {
                let applied = apply_gd_opt_result(
                    &gd_result,
                    &gd_channel_names,
                    channel_results,
                    channel_chains,
                    sample_rate,
                );
                info!(
                    "GD-Opt: improvement {:.1} dB (pre={:.2}ms, post={:.2}ms) in band [{:.0}, {:.0}] Hz; applied={}",
                    gd_result.improvement_db,
                    gd_result.sum_gd_pre_rms_ms,
                    gd_result.sum_gd_post_rms_ms,
                    band.0,
                    band.1,
                    applied,
                );
                let mut summary = GroupDelayOptSummary::from_result_with_names(
                    &gd_result,
                    gd_channel_names.clone(),
                )
                .with_applied(applied);
                if let Some(advisory) = advisory_override {
                    summary.advisory = GroupDelayOptSummary::from_advisory(&advisory).advisory;
                    if matches!(advisory, GdOptAdvisory::MissingCoherenceDelayOnly) {
                        summary.mean_coherence = 0.0;
                    }
                }
                Some(summary)
            }
        }
        Err(e) => {
            info!("GD-Opt: skipped — {}", e);
            // Map known error messages to advisories
            if e.contains("PhaseLinear") {
                Some(GroupDelayOptSummary::from_advisory(
                    &GdOptAdvisory::PhaseLinearNoTarget,
                ))
            } else {
                None
            }
        }
    }
}

fn apply_gd_opt_result(
    result: &super::gd_opt::GroupDelayOptResult,
    channel_names: &[String],
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    sample_rate: f64,
) -> bool {
    let mut applied_any = false;

    for (name, ch_result) in channel_names.iter().zip(result.per_channel.iter()) {
        let mut inserted_for_channel = false;
        if let Some(chain) = channel_chains.get_mut(name.as_str()) {
            if ch_result.polarity_inverted {
                chain
                    .plugins
                    .push(output::create_gain_plugin_with_invert(0.0, true));
                inserted_for_channel = true;
            }
            if ch_result.delay_ms > 0.01 {
                chain
                    .plugins
                    .push(output::create_delay_plugin(ch_result.delay_ms));
                inserted_for_channel = true;
            }
            if !ch_result.ap_filters.is_empty() {
                chain
                    .plugins
                    .push(output::create_eq_plugin(&ch_result.ap_filters));
                inserted_for_channel = true;
            }
        }

        if let Some(ch) = channel_results.get_mut(name.as_str()) {
            let response = gd_phase_response_for_curve(
                &ch.final_curve.freq,
                ch_result.delay_ms,
                ch_result.polarity_inverted,
                &ch_result.ap_filters,
                sample_rate,
            );
            ch.final_curve = crate::response::apply_complex_response(&ch.final_curve, &response);
            if let Some(chain) = channel_chains.get_mut(name.as_str()) {
                chain.final_curve = Some((&ch.final_curve).into());
            }
        }

        applied_any |= inserted_for_channel;
    }

    applied_any
}

fn gd_phase_response_for_curve(
    freqs: &ndarray::Array1<f64>,
    delay_ms: f64,
    polarity_inverted: bool,
    ap_filters: &[Biquad],
    sample_rate: f64,
) -> Vec<Complex64> {
    freqs
        .iter()
        .map(|&f| {
            let mut h = Complex64::new(1.0, 0.0);
            if delay_ms.abs() > 1e-12 {
                h *= Complex64::from_polar(1.0, -2.0 * PI * f * delay_ms * 1e-3);
            }
            for ap in ap_filters {
                // Rebuild with the active sample rate so persisted filter
                // metadata and phase-curve reporting stay aligned.
                let ap = Biquad::new(ap.filter_type, ap.freq, sample_rate, ap.q, ap.db_gain);
                h *= ap.complex_response(f);
            }
            if polarity_inverted {
                h = -h;
            }
            h
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use math_audio_iir_fir::BiquadFilterType;
    use ndarray::array;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    fn test_chain(channel: &str, final_curve: &Curve) -> ChannelDspChain {
        ChannelDspChain {
            channel: channel.to_string(),
            plugins: Vec::new(),
            drivers: None,
            initial_curve: None,
            final_curve: Some(final_curve.into()),
            eq_response: None,
            target_curve: None,
            pre_ir: None,
            post_ir: None,
        }
    }

    #[test]
    fn phase_alignment_reporting_adjusts_phase_without_touching_magnitude() {
        let mut curve = Curve {
            freq: array![50.0, 100.0],
            spl: array![1.0, 2.0],
            phase: Some(array![10.0, -20.0]),
            ..Default::default()
        };

        apply_phase_only_adjustment_to_reported_curve(&mut curve, 2.0, true);

        assert_eq!(curve.spl, array![1.0, 2.0]);
        let phase = curve.phase.as_ref().unwrap();
        assert_close(phase[0], 154.0);
        assert_close(phase[1], 88.0);
    }

    #[test]
    fn phase_alignment_reporting_creates_phase_when_missing() {
        let mut curve = Curve {
            freq: array![100.0],
            spl: array![0.0],
            phase: None,
            ..Default::default()
        };

        apply_phase_only_adjustment_to_reported_curve(&mut curve, 1.0, false);

        let phase = curve.phase.as_ref().unwrap();
        assert_close(phase[0], -36.0);
    }

    #[test]
    fn reported_phase_sync_updates_channel_result_and_chain_curve() {
        let curve = Curve {
            freq: array![100.0],
            spl: array![0.0],
            phase: Some(array![0.0]),
            ..Default::default()
        };
        let mut channel_results = HashMap::from([(
            "L".to_string(),
            ChannelOptimizationResult {
                name: "L".to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: curve.clone(),
                final_curve: curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
        )]);
        let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);

        sync_reported_phase_adjustment("L", &mut channel_results, &mut channel_chains, 1.0, true);

        let result_phase = channel_results["L"].final_curve.phase.as_ref().unwrap()[0];
        let chain_phase = channel_chains["L"]
            .final_curve
            .as_ref()
            .unwrap()
            .phase
            .as_ref()
            .unwrap()[0];
        assert_close(result_phase, 144.0);
        assert_close(chain_phase, 144.0);
    }

    #[test]
    fn reported_gain_sync_updates_result_and_chain_magnitude() {
        let curve = Curve {
            freq: array![100.0],
            spl: array![1.0],
            phase: Some(array![0.0]),
            ..Default::default()
        };
        let mut channel_results = HashMap::from([(
            "L".to_string(),
            ChannelOptimizationResult {
                name: "L".to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: curve.clone(),
                final_curve: curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
        )]);
        let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);

        sync_reported_gain_adjustment("L", &mut channel_results, &mut channel_chains, 2.5, true);

        assert_close(channel_results["L"].final_curve.spl[0], 3.5);
        assert_close(
            channel_chains["L"].final_curve.as_ref().unwrap().spl[0],
            3.5,
        );
        assert_close(
            channel_results["L"].final_curve.phase.as_ref().unwrap()[0],
            180.0,
        );
    }

    #[test]
    fn reported_biquad_sync_updates_result_and_chain_curve() {
        let curve = Curve {
            freq: array![1000.0],
            spl: array![0.0],
            phase: Some(array![0.0]),
            ..Default::default()
        };
        let mut channel_results = HashMap::from([(
            "L".to_string(),
            ChannelOptimizationResult {
                name: "L".to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: curve.clone(),
                final_curve: curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
        )]);
        let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            6.0,
        )];

        sync_reported_biquad_adjustment(
            "L",
            &mut channel_results,
            &mut channel_chains,
            &filters,
            48000.0,
        );

        assert!(
            channel_results["L"].final_curve.spl[0] > 5.5,
            "expected PEQ boost in reported result"
        );
        assert_close(
            channel_results["L"].final_curve.spl[0],
            channel_chains["L"].final_curve.as_ref().unwrap().spl[0],
        );
    }

    #[test]
    fn reported_fir_sync_updates_result_and_chain_curve() {
        let curve = Curve {
            freq: array![1000.0],
            spl: array![0.0],
            phase: Some(array![0.0]),
            ..Default::default()
        };
        let mut channel_results = HashMap::from([(
            "L".to_string(),
            ChannelOptimizationResult {
                name: "L".to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: curve.clone(),
                final_curve: curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
        )]);
        let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);

        sync_reported_fir_adjustment(
            "L",
            &mut channel_results,
            &mut channel_chains,
            &[1.0, -1.0],
            48000.0,
        );

        assert!(
            channel_results["L"].final_curve.spl[0] < -17.0,
            "expected differentiator FIR attenuation at 1 kHz"
        );
        assert_close(
            channel_results["L"].final_curve.spl[0],
            channel_chains["L"].final_curve.as_ref().unwrap().spl[0],
        );
    }

    #[test]
    fn perceptual_metrics_report_epa_and_timing_confidence() {
        let score = |preference| crate::loss::epa::score::EpaScore {
            evaluation: 0.0,
            potency: 0.0,
            activity: 0.0,
            preference,
            sharpness_acum: 0.0,
            roughness: 0.0,
            total_loudness_sone: 0.0,
            loudness_balance: 0.0,
        };
        let group_delay = super::super::gd_opt::GroupDelayOptSummary {
            band: (20.0, 120.0),
            channel_names: Vec::new(),
            per_channel_delay_ms: Vec::new(),
            per_channel_polarity_inverted: Vec::new(),
            per_channel_ap_count: Vec::new(),
            sum_gd_pre_rms_ms: 1.0,
            sum_gd_post_rms_ms: 0.5,
            mean_coherence: 1.0,
            improvement_db: 0.0,
            advisory: "success".to_string(),
            applied: true,
        };
        let mut metadata = OptimizationMetadata {
            pre_score: 0.0,
            post_score: 0.0,
            algorithm: "test".to_string(),
            loss_type: Some("flat".to_string()),
            iterations: 0,
            timestamp: "test".to_string(),
            inter_channel_deviation: Some(super::super::types::InterChannelDeviation {
                deviation_per_freq: Vec::new(),
                midrange_rms_db: 1.25,
                passband_rms_db: 2.0,
                midrange_peak_db: 3.0,
                midrange_peak_freq: 1000.0,
            }),
            epa_per_channel: Some(HashMap::from([
                (
                    "L".to_string(),
                    super::super::types::EpaChannelMetrics {
                        pre: score(4.0),
                        post: score(6.0),
                    },
                ),
                (
                    "R".to_string(),
                    super::super::types::EpaChannelMetrics {
                        pre: score(5.0),
                        post: score(7.0),
                    },
                ),
            ])),
            group_delay: Some(group_delay),
            perceptual_metrics: None,
            home_cinema_layout: None,
            multi_seat_coverage: None,
            multi_seat_correction: None,
            bass_management: None,
            timing_diagnostics: None,
        };

        update_perceptual_metrics(&mut metadata, None, None);

        let metrics = metadata.perceptual_metrics.expect("metrics");
        assert_close(metrics.epa_preference_pre, 4.5);
        assert_close(metrics.epa_preference_post, 6.5);
        assert_close(metrics.epa_preference_delta, 2.0);
        assert_close(metrics.channel_matching_midrange_rms_db.unwrap(), 1.25);
        assert_eq!(metrics.timing_confidence.as_deref(), Some("gd_applied"));
    }

    #[test]
    fn perceptual_metrics_report_role_bass_dialog_and_headroom_guards() {
        let score = |preference| crate::loss::epa::score::EpaScore {
            evaluation: 0.0,
            potency: 0.0,
            activity: 0.0,
            preference,
            sharpness_acum: 0.0,
            roughness: 0.0,
            total_loudness_sone: 0.0,
            loudness_balance: 0.0,
        };
        let freqs = array![40.0, 80.0, 300.0, 1000.0, 4000.0];
        let left = Curve {
            freq: freqs.clone(),
            spl: array![0.0, 0.0, 0.0, 0.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let right = Curve {
            freq: freqs.clone(),
            spl: array![0.0, 0.0, 0.0, 2.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let center = Curve {
            freq: freqs.clone(),
            spl: array![0.0, 0.0, -1.0, 1.0, -1.0],
            phase: None,
            ..Default::default()
        };
        let sub = Curve {
            freq: freqs.clone(),
            spl: array![1.0, 1.0, 0.0, 0.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let lfe = Curve {
            freq: freqs,
            spl: array![-1.0, -1.0, 0.0, 0.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let mut boosted_left = test_chain("L", &left);
        boosted_left.plugins.push(output::create_gain_plugin(5.0));
        let channels = HashMap::from([
            ("L".to_string(), boosted_left),
            ("R".to_string(), test_chain("R", &right)),
            ("C".to_string(), test_chain("C", &center)),
            ("Sub".to_string(), test_chain("Sub", &sub)),
            ("LFE".to_string(), test_chain("LFE", &lfe)),
        ]);
        let mut metadata = OptimizationMetadata {
            pre_score: 0.0,
            post_score: 0.0,
            algorithm: "test".to_string(),
            loss_type: Some("flat".to_string()),
            iterations: 0,
            timestamp: "test".to_string(),
            inter_channel_deviation: None,
            epa_per_channel: Some(HashMap::from([(
                "L".to_string(),
                super::super::types::EpaChannelMetrics {
                    pre: score(1.0),
                    post: score(2.0),
                },
            )])),
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: None,
            multi_seat_coverage: None,
            multi_seat_correction: None,
            bass_management: None,
            timing_diagnostics: None,
        };

        update_perceptual_metrics(&mut metadata, Some(&channels), None);

        let metrics = metadata.perceptual_metrics.expect("metrics");
        assert!(metrics.role_channel_matching_rms_db.unwrap() > 0.0);
        assert_close(metrics.bass_consistency_rms_db.unwrap(), 1.0);
        assert!(metrics.dialog_band_roughness_rms_db.unwrap() > 0.8);
        assert_close(metrics.headroom_peak_boost_db.unwrap(), 5.0);
        assert_eq!(
            metrics.headroom_risk.as_deref(),
            Some("moderate_boost_uses_headroom")
        );
    }

    #[test]
    fn role_channel_matching_metric_skips_invalid_groups() {
        let valid_l = Curve {
            freq: array![300.0, 1000.0, 4000.0],
            spl: array![0.0, 0.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let valid_r = Curve {
            freq: array![300.0, 1000.0, 4000.0],
            spl: array![0.0, 2.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let invalid_sl = Curve {
            freq: array![300.0, 1000.0, 4000.0],
            spl: array![0.0, 0.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let invalid_sr = Curve {
            freq: array![301.0, 1000.0, 4000.0],
            spl: array![0.0, 0.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let channels = HashMap::from([
            ("L".to_string(), test_chain("L", &valid_l)),
            ("R".to_string(), test_chain("R", &valid_r)),
            ("SL".to_string(), test_chain("SL", &invalid_sl)),
            ("SR".to_string(), test_chain("SR", &invalid_sr)),
        ]);

        let rms = role_channel_matching_rms_db(&channels).expect("valid L/R group");

        assert!(rms > 0.0);
    }

    #[test]
    fn timing_diagnostics_reflect_measured_arrivals_and_exported_delays() {
        let curve = Curve {
            freq: array![100.0, 1000.0],
            spl: array![0.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let mut left = test_chain("L", &curve);
        left.plugins.push(output::create_delay_plugin(2.0));
        let channels = HashMap::from([
            ("L".to_string(), left),
            ("R".to_string(), test_chain("R", &curve)),
            ("SL".to_string(), test_chain("SL", &curve)),
        ]);
        let arrivals = HashMap::from([
            ("L".to_string(), 8.0),
            ("R".to_string(), 10.0),
            ("SL".to_string(), 7.0),
        ]);
        let config = RoomConfig {
            version: "test".to_string(),
            system: None,
            speakers: HashMap::new(),
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            cea2034_cache: None,
        };

        let report = build_timing_diagnostics(&config, &arrivals, &channels).expect("timing");
        assert_eq!(report.reference_channel.as_deref(), Some("L"));
        assert_close(report.reference_arrival_ms.unwrap(), 10.0);
        assert_close(report.arrival_spread_before_ms, 3.0);
        assert_close(report.arrival_spread_after_ms, 3.0);
        let left_report = report
            .channels
            .iter()
            .find(|channel| channel.name == "L")
            .unwrap();
        assert_close(left_report.measured_arrival_ms, 8.0);
        assert_close(left_report.applied_delay_ms, 2.0);
        assert_close(left_report.final_arrival_ms, 10.0);
        assert!(
            report
                .advisories
                .contains(&"surround_or_height_precedence_risk".to_string())
        );
    }

    #[test]
    fn total_chain_delay_sums_stacked_delay_plugins() {
        let curve = Curve {
            freq: array![100.0],
            spl: array![0.0],
            phase: None,
            ..Default::default()
        };
        let mut chain = test_chain("L", &curve);
        chain.plugins.push(output::create_delay_plugin(2.0));
        chain.plugins.push(output::create_gain_plugin(1.5));
        chain.plugins.push(output::create_delay_plugin(3.5));

        assert_close(total_chain_delay_ms(&chain), 5.5);
    }

    #[test]
    fn channel_matching_role_key_groups_like_roles_only() {
        assert_eq!(channel_matching_role_key("L"), Some("front_lr"));
        assert_eq!(channel_matching_role_key("front-right"), Some("front_lr"));
        assert_eq!(channel_matching_role_key("C"), None);
        assert_eq!(channel_matching_role_key("SL"), Some("side_surrounds"));
        assert_eq!(
            channel_matching_role_key("surround right"),
            Some("side_surrounds")
        );
        assert_eq!(channel_matching_role_key("TFL"), Some("top_front"));
        assert_eq!(
            channel_matching_role_key("rear.height.right"),
            Some("top_rear")
        );
    }

    #[test]
    fn role_aware_channel_matching_groups_exclude_center() {
        let curve = Curve {
            freq: array![500.0, 1000.0],
            spl: array![0.0, 0.0],
            phase: None,
            ..Default::default()
        };
        let curves = HashMap::from([
            ("L".to_string(), curve.clone()),
            ("R".to_string(), curve.clone()),
            ("C".to_string(), curve.clone()),
            ("SL".to_string(), curve.clone()),
            ("SR".to_string(), curve),
        ]);

        let groups = role_aware_channel_matching_groups(&curves);
        let mut group_names: Vec<Vec<String>> = groups
            .into_iter()
            .map(|group| {
                let mut names: Vec<String> = group.keys().cloned().collect();
                names.sort();
                names
            })
            .collect();
        group_names.sort();

        assert_eq!(
            group_names,
            vec![
                vec!["L".to_string(), "R".to_string()],
                vec!["SL".to_string(), "SR".to_string()],
            ]
        );
    }

    #[test]
    fn phase_alignment_delay_schedule_preserves_shared_sub_constraints() {
        let phase_results = HashMap::from([
            ("L".to_string(), (-5.0, false, "LFE".to_string())),
            ("R".to_string(), (2.0, false, "LFE".to_string())),
        ]);

        let schedule = compute_phase_alignment_delay_schedule(&phase_results);

        assert_close(*schedule.get("L").unwrap_or(&0.0), 0.0);
        assert_close(schedule["LFE"], 5.0);
        assert_close(schedule["R"], 7.0);
        assert_close(
            schedule.get("L").unwrap_or(&0.0) - schedule.get("LFE").unwrap_or(&0.0),
            -5.0,
        );
        assert_close(schedule["R"] - schedule["LFE"], 2.0);
    }

    #[test]
    fn phase_alignment_delay_schedule_normalizes_independent_components() {
        let phase_results = HashMap::from([
            ("L".to_string(), (-3.0, false, "SubL".to_string())),
            ("R".to_string(), (4.0, false, "SubR".to_string())),
        ]);

        let schedule = compute_phase_alignment_delay_schedule(&phase_results);

        assert_close(*schedule.get("L").unwrap_or(&0.0), 0.0);
        assert_close(schedule["SubL"], 3.0);
        assert_close(*schedule.get("SubR").unwrap_or(&0.0), 0.0);
        assert_close(schedule["R"], 4.0);
    }

    #[test]
    fn negative_phase_alignment_delay_applies_sub_delay_and_reported_phase() {
        let main_curve = Curve {
            freq: array![100.0],
            spl: array![0.0],
            phase: Some(array![0.0]),
            ..Default::default()
        };
        let sub_curve = Curve {
            freq: array![100.0],
            spl: array![0.0],
            phase: Some(array![0.0]),
            ..Default::default()
        };
        let mut channel_results = HashMap::from([
            (
                "L".to_string(),
                ChannelOptimizationResult {
                    name: "L".to_string(),
                    pre_score: 0.0,
                    post_score: 0.0,
                    initial_curve: main_curve.clone(),
                    final_curve: main_curve.clone(),
                    biquads: Vec::new(),
                    fir_coeffs: None,
                },
            ),
            (
                "LFE".to_string(),
                ChannelOptimizationResult {
                    name: "LFE".to_string(),
                    pre_score: 0.0,
                    post_score: 0.0,
                    initial_curve: sub_curve.clone(),
                    final_curve: sub_curve.clone(),
                    biquads: Vec::new(),
                    fir_coeffs: None,
                },
            ),
        ]);
        let mut channel_chains = HashMap::from([
            ("L".to_string(), test_chain("L", &main_curve)),
            ("LFE".to_string(), test_chain("LFE", &sub_curve)),
        ]);
        let phase_results = HashMap::from([("L".to_string(), (-5.0, false, "LFE".to_string()))]);

        let applied = apply_phase_alignment_delay_schedule(
            &phase_results,
            &mut channel_results,
            &mut channel_chains,
        );

        assert!(!applied.contains_key("L"));
        assert_close(applied["LFE"], 5.0);
        assert_close(total_chain_delay_ms(&channel_chains["LFE"]), 5.0);
        assert_eq!(total_chain_delay_ms(&channel_chains["L"]), 0.0);
        assert_close(
            channel_results["LFE"].final_curve.phase.as_ref().unwrap()[0],
            -180.0,
        );
        assert_close(
            channel_chains["LFE"]
                .final_curve
                .as_ref()
                .unwrap()
                .phase
                .as_ref()
                .unwrap()[0],
            -180.0,
        );
    }

    #[test]
    fn phase_alignment_reported_curve_collection_sees_applied_delay() {
        let main_curve = Curve {
            freq: array![100.0],
            spl: array![0.0],
            phase: Some(array![0.0]),
            ..Default::default()
        };
        let sub_curve = Curve {
            freq: array![100.0],
            spl: array![0.0],
            phase: Some(array![0.0]),
            ..Default::default()
        };
        let mut channel_results = HashMap::from([
            (
                "L".to_string(),
                ChannelOptimizationResult {
                    name: "L".to_string(),
                    pre_score: 0.0,
                    post_score: 0.0,
                    initial_curve: main_curve.clone(),
                    final_curve: main_curve.clone(),
                    biquads: Vec::new(),
                    fir_coeffs: None,
                },
            ),
            (
                "LFE".to_string(),
                ChannelOptimizationResult {
                    name: "LFE".to_string(),
                    pre_score: 0.0,
                    post_score: 0.0,
                    initial_curve: sub_curve.clone(),
                    final_curve: sub_curve.clone(),
                    biquads: Vec::new(),
                    fir_coeffs: None,
                },
            ),
        ]);
        let mut channel_chains = HashMap::from([
            ("L".to_string(), test_chain("L", &main_curve)),
            ("LFE".to_string(), test_chain("LFE", &sub_curve)),
        ]);
        let phase_results = HashMap::from([("L".to_string(), (-2.5, false, "LFE".to_string()))]);

        apply_phase_alignment_delay_schedule(
            &phase_results,
            &mut channel_results,
            &mut channel_chains,
        );
        let current_curves = collect_current_final_curves(&channel_results);

        assert_close(current_curves["LFE"].phase.as_ref().unwrap()[0], -90.0);
        assert_close(current_curves["L"].phase.as_ref().unwrap()[0], 0.0);
    }

    #[test]
    fn gd_result_application_inserts_dsp_and_updates_reported_phase() {
        let curve = Curve {
            freq: array![100.0, 200.0],
            spl: array![0.0, 0.0],
            phase: Some(array![0.0, 0.0]),
            ..Default::default()
        };
        let mut channel_results = HashMap::from([(
            "L".to_string(),
            ChannelOptimizationResult {
                name: "L".to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: curve.clone(),
                final_curve: curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
        )]);
        let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);
        let result = super::super::gd_opt::GroupDelayOptResult {
            band: (20.0, 160.0),
            per_channel: vec![super::super::gd_opt::ChannelGdResult {
                delay_ms: 1.0,
                polarity_inverted: true,
                ap_filters: vec![Biquad::new(
                    BiquadFilterType::AllPass,
                    80.0,
                    48000.0,
                    1.0,
                    0.0,
                )],
                channel_gd_pre_rms_ms: 0.0,
                channel_gd_post_rms_ms: 0.0,
            }],
            sum_gd_pre_rms_ms: 1.0,
            sum_gd_post_rms_ms: 0.1,
            mean_coherence: 1.0,
            improvement_db: 20.0,
        };

        let applied = apply_gd_opt_result(
            &result,
            &["L".to_string()],
            &mut channel_results,
            &mut channel_chains,
            48000.0,
        );

        assert!(applied);
        let plugins = &channel_chains["L"].plugins;
        assert_eq!(plugins.len(), 3);
        assert_eq!(plugins[0].plugin_type, "gain");
        assert_eq!(plugins[1].plugin_type, "delay");
        assert_eq!(plugins[2].plugin_type, "eq");

        let result_phase = channel_results["L"].final_curve.phase.as_ref().unwrap()[0];
        let chain_phase = channel_chains["L"]
            .final_curve
            .as_ref()
            .unwrap()
            .phase
            .as_ref()
            .unwrap()[0];
        assert!(
            result_phase.abs() > 1.0,
            "GD DSP must be reflected in final_curve phase"
        );
        assert_close(result_phase, chain_phase);
    }

    #[test]
    fn generic_path_score_band_uses_optimizer_bounds() {
        let mut config = test_room_config_with_gd(Default::default());
        config.system = None;
        config.optimizer.min_freq = 35.0;
        config.optimizer.max_freq = 12_500.0;

        assert_eq!(
            final_score_band_for_channel(&config, "left"),
            (35.0, 12_500.0)
        );
    }

    #[test]
    fn gd_opt_rejects_same_length_mismatched_frequency_grids() {
        let curve_l = Curve {
            freq: array![20.0, 40.0, 80.0, 160.0],
            spl: array![0.0, 0.0, 0.0, 0.0],
            phase: Some(array![0.0, 0.0, 0.0, 0.0]),
            coherence: Some(array![0.95, 0.95, 0.95, 0.95]),
            ..Default::default()
        };
        let curve_r = Curve {
            freq: array![20.0, 41.0, 80.0, 160.0],
            spl: array![0.0, 0.0, 0.0, 0.0],
            phase: Some(array![0.0, 0.0, 0.0, 0.0]),
            coherence: Some(array![0.95, 0.95, 0.95, 0.95]),
            ..Default::default()
        };
        let mut channel_results = HashMap::from([
            ("L".to_string(), test_channel_result("L", &curve_l)),
            ("R".to_string(), test_channel_result("R", &curve_r)),
        ]);
        let mut channel_chains = HashMap::from([
            ("L".to_string(), test_chain("L", &curve_l)),
            ("R".to_string(), test_chain("R", &curve_r)),
        ]);
        let config = test_room_config_with_gd(super::super::types::GroupDelayOptimizationConfig {
            enabled: true,
            ..Default::default()
        });

        let summary = try_run_gd_opt(&config, &mut channel_results, &mut channel_chains, 48000.0)
            .expect("GD summary should explain the skip");

        assert_eq!(summary.advisory, "frequency_grid_mismatch");
        assert!(!summary.applied);
        assert!(channel_chains["L"].plugins.is_empty());
        assert!(channel_chains["R"].plugins.is_empty());
    }

    #[test]
    fn gd_opt_missing_coherence_downgrades_to_delay_only() {
        let curve_l = gd_test_curve(0.0, None);
        let curve_r = gd_test_curve(2.0, None);
        let mut channel_results = HashMap::from([
            ("L".to_string(), test_channel_result("L", &curve_l)),
            ("R".to_string(), test_channel_result("R", &curve_r)),
        ]);
        let mut channel_chains = HashMap::from([
            ("L".to_string(), test_chain("L", &curve_l)),
            ("R".to_string(), test_chain("R", &curve_r)),
        ]);
        let config = test_room_config_with_gd(super::super::types::GroupDelayOptimizationConfig {
            enabled: true,
            min_improvement_db: -100.0,
            max_iter: 300,
            popsize: 10,
            adaptive_allpass: false,
            ap_per_channel: 2,
            optimize_polarity: true,
            ..Default::default()
        });

        let summary = try_run_gd_opt(&config, &mut channel_results, &mut channel_chains, 48000.0)
            .expect("GD summary");

        assert_eq!(summary.advisory, "missing_coherence_delay_only");
        assert!(summary.per_channel_ap_count.iter().all(|&n| n == 0));
        assert!(
            summary
                .per_channel_polarity_inverted
                .iter()
                .all(|&inverted| !inverted)
        );
    }

    #[test]
    fn gd_opt_disables_allpass_without_bootstrap_realisations() {
        let coherence = Some(array![0.95, 0.95, 0.95, 0.95, 0.95, 0.95]);
        let curve_l = gd_test_curve(0.0, coherence.clone());
        let curve_r = gd_test_curve(2.0, coherence);
        let mut channel_results = HashMap::from([
            ("L".to_string(), test_channel_result("L", &curve_l)),
            ("R".to_string(), test_channel_result("R", &curve_r)),
        ]);
        let mut channel_chains = HashMap::from([
            ("L".to_string(), test_chain("L", &curve_l)),
            ("R".to_string(), test_chain("R", &curve_r)),
        ]);
        let config = test_room_config_with_gd(super::super::types::GroupDelayOptimizationConfig {
            enabled: true,
            min_improvement_db: -100.0,
            max_iter: 300,
            popsize: 10,
            adaptive_allpass: true,
            ap_per_channel: 2,
            ..Default::default()
        });

        let summary = try_run_gd_opt(&config, &mut channel_results, &mut channel_chains, 48000.0)
            .expect("GD summary");

        assert_eq!(
            summary.advisory,
            "allpass_disabled_no_bootstrap_realisations"
        );
        assert!(summary.per_channel_ap_count.iter().all(|&n| n == 0));
    }

    fn gd_test_curve(delay_ms: f64, coherence: Option<ndarray::Array1<f64>>) -> Curve {
        let freq = array![20.0, 40.0, 80.0, 120.0, 160.0, 200.0];
        let phase = freq.mapv(|f| -360.0 * f * delay_ms * 1e-3);
        Curve {
            freq,
            spl: array![0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            phase: Some(phase),
            coherence,
            ..Default::default()
        }
    }

    fn test_channel_result(name: &str, curve: &Curve) -> ChannelOptimizationResult {
        ChannelOptimizationResult {
            name: name.to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: curve.clone(),
            final_curve: curve.clone(),
            biquads: Vec::new(),
            fir_coeffs: None,
        }
    }

    fn spectral_gate_curve(spl_fn: impl Fn(f64) -> f64) -> Curve {
        let n = 200;
        let log_start = 20f64.log10();
        let log_end = 20000f64.log10();
        let freq: Vec<f64> = (0..n)
            .map(|i| 10f64.powf(log_start + (log_end - log_start) * i as f64 / (n - 1) as f64))
            .collect();
        let spl: Vec<f64> = freq.iter().map(|&f| spl_fn(f)).collect();
        Curve {
            freq: ndarray::Array1::from(freq),
            spl: ndarray::Array1::from(spl),
            phase: None,
            ..Default::default()
        }
    }

    #[test]
    fn spectral_shelf_gate_accepts_icd_improvement_despite_flatness_regression() {
        let sample_rate = 48_000.0;
        let mut candidate = None;

        for left_low in [-3.0, -1.5, 0.0, 1.5, 3.0] {
            for left_high in [-3.0, -1.5, 0.0, 1.5, 3.0] {
                for right_low in [-3.0, -1.5, 0.0, 1.5, 3.0] {
                    for right_high in [-3.0, -1.5, 0.0, 1.5, 3.0] {
                        let make_shaped = |low_gain, high_gain| {
                            let low = Biquad::new(
                                math_audio_iir_fir::BiquadFilterType::Lowshelf,
                                super::super::spectral_align::LOWSHELF_FREQ,
                                sample_rate,
                                math_audio_iir_fir::DEFAULT_Q_HIGH_LOW_SHELF,
                                low_gain,
                            );
                            let high = Biquad::new(
                                math_audio_iir_fir::BiquadFilterType::Highshelf,
                                super::super::spectral_align::HIGHSHELF_FREQ,
                                sample_rate,
                                math_audio_iir_fir::DEFAULT_Q_HIGH_LOW_SHELF,
                                high_gain,
                            );
                            spectral_gate_curve(|f| {
                                low.np_log_result(&ndarray::arr1(&[f]))[0]
                                    + high.np_log_result(&ndarray::arr1(&[f]))[0]
                            })
                        };

                        let curves = HashMap::from([
                            ("L".to_string(), make_shaped(left_low, left_high)),
                            ("R".to_string(), make_shaped(right_low, right_high)),
                        ]);
                        let alignment = super::super::spectral_align::compute_spectral_alignment(
                            &curves,
                            sample_rate,
                            20.0,
                            20_000.0,
                        );

                        for channel in ["L", "R"] {
                            let shelf_filters =
                                super::super::spectral_align::create_alignment_filters(
                                    &alignment[channel],
                                    sample_rate,
                                );
                            if shelf_filters.is_empty() {
                                continue;
                            }

                            let before_flat =
                                recompute_curve_flatness_score(&curves[channel], 20.0, 20_000.0);
                            let response = crate::response::compute_peq_complex_response(
                                &shelf_filters,
                                &curves[channel].freq,
                                sample_rate,
                            );
                            let corrected = crate::response::apply_complex_response(
                                &curves[channel],
                                &response,
                            );
                            let after_flat =
                                recompute_curve_flatness_score(&corrected, 20.0, 20_000.0);
                            if after_flat <= before_flat + 1e-3 {
                                continue;
                            }

                            let icd_before =
                                super::super::spectral_align::compute_inter_channel_deviation(
                                    &curves, 20.0,
                                );
                            let mut corrected_curves = curves.clone();
                            corrected_curves.insert(channel.to_string(), corrected);
                            let icd_after =
                                super::super::spectral_align::compute_inter_channel_deviation(
                                    &corrected_curves,
                                    20.0,
                                );
                            let flatness_regression = after_flat - before_flat;
                            let icd_improvement =
                                icd_before.passband_rms_db - icd_after.passband_rms_db;
                            if icd_improvement > flatness_regression + 1e-6 {
                                candidate = Some((
                                    curves,
                                    channel.to_string(),
                                    shelf_filters,
                                    before_flat,
                                    after_flat,
                                    icd_before.passband_rms_db,
                                    icd_after.passband_rms_db,
                                ));
                                break;
                            }
                        }
                        if candidate.is_some() {
                            break;
                        }
                    }
                    if candidate.is_some() {
                        break;
                    }
                }
                if candidate.is_some() {
                    break;
                }
            }
            if candidate.is_some() {
                break;
            }
        }

        let Some((curves, channel, shelf_filters, before_flat, after_flat, icd_before, icd_after)) =
            candidate
        else {
            panic!("test setup could not find an ICD-improving shelf with a flatness regression");
        };
        assert!(
            after_flat > before_flat + 1e-3
                && icd_before - icd_after > after_flat - before_flat + 1e-6,
            "invalid test candidate: flatness {before_flat}->{after_flat}, ICD {icd_before}->{icd_after}"
        );

        assert!(
            should_apply_spectral_shelves(
                &curves,
                &channel,
                &shelf_filters,
                sample_rate,
                20.0,
                20_000.0,
            ),
            "shelves should be accepted when they improve inter-channel deviation"
        );
    }

    fn test_room_config_with_gd(
        group_delay: super::super::types::GroupDelayOptimizationConfig,
    ) -> RoomConfig {
        RoomConfig {
            version: super::super::types::default_config_version(),
            system: None,
            speakers: HashMap::new(),
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig {
                group_delay: Some(group_delay),
                seed: Some(11),
                ..Default::default()
            },
            recording_config: None,
            cea2034_cache: None,
        }
    }
}
