//\! Multi-speaker group and advanced processing
//\!
//\! This module handles optimization of speaker groups (multi-driver), multi-subwoofer arrays,
//\! DBA configurations, Cardioid arrays, and mixed-mode frequency-split processing.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::read as load;
use crate::response;
use log::{debug, info, warn};
use math_audio_dsp::analysis::compute_average_response;
use math_audio_iir_fir::Biquad;
use std::collections::HashMap;
use std::path::Path;

use super::crossover;
use super::dba;
use super::eq;
use super::fir;
use super::multisub;
use super::output;
use super::types::{
    ChannelDspChain, MixedModeConfig, MultiSubGroup, OptimizerConfig, RoomConfig, SpeakerGroup,
};

// Type aliases from optimize module
pub(super) type MixedModeResult = (
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

// Import helper functions from optimize and speaker_eq modules
use super::optimize::detect_passband_and_mean;
use super::speaker_eq::determine_optimization_bands;

pub(super) fn process_speaker_group(
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
pub(super) fn process_multisub_group(
    channel_name: &str,
    group: &MultiSubGroup,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<MixedModeResult> {
    let (result, combined_curve, allpass_filters) = if group.allpass_optimization {
        // All-pass enhanced optimization
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
pub(super) fn process_dba(
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
pub(super) fn process_mixed_mode_crossover(
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
        ..Default::default()
    };

    let high_curve = Curve {
        freq: curve.freq.slice(ndarray::s![high_start..]).to_owned(),
        spl: curve.spl.slice(ndarray::s![high_start..]).to_owned(),
        phase: curve
            .phase
            .as_ref()
            .map(|p| p.slice(ndarray::s![high_start..]).to_owned()),
        ..Default::default()
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
#[allow(dead_code)]
pub(super) fn check_group_consistency(
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
#[allow(dead_code)]
pub(super) fn check_octave_consistency(
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
pub(super) fn process_cardioid(
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
        ..Default::default()
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
