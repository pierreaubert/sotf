//\! Single-speaker EQ optimization
//\!
//\! This module handles optimization of individual speakers with a single measurement.
//\! Includes support for Schroeder split, multi-measurement strategies, and target curve matching.

use super::spectral_align;
use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::read as load;
use crate::response;
use log::{debug, info, warn};
use math_audio_dsp::analysis::compute_average_response;
use math_audio_iir_fir::Biquad;
use ndarray::Array1;
use std::path::Path;

use super::eq;
use super::excursion;
use super::fir;
use super::output;
use super::target_tilt;
use super::types::{
    ChannelDspChain, MeasurementSource, OptimizerConfig, ProcessingMode, RoomConfig, TargetShape,
    TiltType,
};

// Import from optimize and group_processing modules
use super::optimize::{detect_passband_and_mean, extract_wav_path};
use super::group_processing::process_mixed_mode_crossover;

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

pub(super) fn optimize_eq_maybe_multi(
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
pub(super) fn process_single_speaker(
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
    // Build the unified target curve from target_response (or migrated legacy fields).
    // This curve is the single source of truth for both broadband pre-correction
    // and EQ optimization, eliminating double-tilt bugs.
    //
    // When 3-pass CEA2034 correction is active, user preferences (bass/treble shelves)
    // are emitted as Pass 3 filters rather than being baked into the target curve.
    let cea2034_active = room_config
        .optimizer
        .cea2034_correction
        .as_ref()
        .is_some_and(|c| c.enabled);

    let target_tilt_curve = if let Some(ref target_resp) = room_config.optimizer.target_response {
        // When 3-pass is active, strip preferences from the target
        // (they become Pass 3 output filters instead)
        let effective_target = if cea2034_active {
            let mut stripped = target_resp.clone();
            stripped.preference = super::types::UserPreference::default();
            stripped
        } else {
            target_resp.clone()
        };

        if effective_target.shape != TargetShape::Flat
            || effective_target.preference.bass_shelf_db.abs() > 1e-6
            || effective_target.preference.treble_shelf_db.abs() > 1e-6
        {
            info!(
                "  Building target curve: shape={:?}, slope={:.2} dB/oct, bass={:+.1}dB, treble={:+.1}dB{}",
                effective_target.shape,
                match effective_target.shape {
                    TargetShape::Harman => -0.8,
                    TargetShape::Custom => effective_target.slope_db_per_octave,
                    _ => 0.0,
                },
                effective_target.preference.bass_shelf_db,
                effective_target.preference.treble_shelf_db,
                if cea2034_active { " (preferences extracted to Pass 3)" } else { "" },
            );
            Some(target_tilt::build_complete_target_curve(
                &curve.freq,
                &effective_target,
            ))
        } else {
            None
        }
    } else if let Some(tilt_config) = &room_config.optimizer.target_tilt {
        // Legacy path: target_tilt without migration (shouldn't happen after migrate_target_config)
        if tilt_config.tilt_type != TiltType::Flat {
            info!(
                "  Building target curve with legacy {:?} tilt ({:.2} dB/octave)",
                tilt_config.tilt_type, tilt_config.slope_db_per_octave
            );
            Some(target_tilt::build_target_curve_with_tilt(
                &curve.freq,
                tilt_config,
            ))
        } else {
            None
        }
    } else {
        None
    };

    // When target curve is active, it is baked into the measurement before optimization.
    // Passing target_curve on top would double-apply.
    if target_tilt_curve.is_some() && room_config.target_curve.is_some() {
        warn!(
            "  Both target_curve and target_response are configured for '{}'. \
             target_response is baked into the measurement; target_curve will be \
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

    // ========================================================================
    // Pass 1: CEA2034 Speaker Correction (above Schroeder frequency)
    // ========================================================================
    let (curve, cea2034_filters, cea2034_plugins) =
        if let Some(cea_config) = &room_config.optimizer.cea2034_correction {
            if cea_config.enabled {
                // Resolve speaker name: config override > MeasurementSource
                let speaker_name = cea_config
                    .speaker_name
                    .as_deref()
                    .or_else(|| source.speaker_name());

                if let Some(name) = speaker_name {
                    // Look up pre-fetched CEA2034 data
                    let cea_data = room_config
                        .cea2034_cache
                        .as_ref()
                        .and_then(|cache| cache.get(name));

                    if let Some(data) = cea_data {
                        // Determine Schroeder frequency
                        let schroeder_freq = cea_config.min_freq.unwrap_or_else(|| {
                            room_config
                                .optimizer
                                .schroeder_split
                                .as_ref()
                                .filter(|s| s.enabled)
                                .map(|s| s.schroeder_freq)
                                .unwrap_or(300.0)
                        });

                        match super::cea2034_correction::compute_speaker_correction(
                            data,
                            cea_config,
                            &curve,
                            schroeder_freq,
                            arrival_time_ms,
                            sample_rate,
                        ) {
                            Ok((filters, corrected_curve)) => {
                                info!(
                                    "  Pass 1 CEA2034 correction: {} filters above {:.0} Hz for '{}'",
                                    filters.len(),
                                    schroeder_freq,
                                    name
                                );
                                let plugin = output::create_labeled_eq_plugin(
                                    &filters,
                                    "cea2034_speaker_correction",
                                );
                                (corrected_curve, filters, vec![plugin])
                            }
                            Err(e) => {
                                warn!(
                                    "  CEA2034 correction failed for '{}': {}. Skipping Pass 1.",
                                    name, e
                                );
                                (curve, vec![], vec![])
                            }
                        }
                    } else {
                        warn!(
                            "  No CEA2034 data in cache for speaker '{}'. Skipping Pass 1.",
                            name
                        );
                        (curve, vec![], vec![])
                    }
                } else {
                    debug!("  No speaker_name configured. Skipping CEA2034 correction.");
                    (curve, vec![], vec![])
                }
            } else {
                (curve, vec![], vec![])
            }
        } else {
            (curve, vec![], vec![])
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
    // Broadband Pre-Correction
    // ========================================================================
    // Fit shelves/gain to the complete target curve (including tilt + preference)
    // within the speaker's passband, establishing a balanced baseline before
    // fine-grained EQ optimization. Both broadband and optimizer share the SAME
    // target curve, so there is no double-application of tilt.
    let broadband_enabled = room_config
        .optimizer
        .target_response
        .as_ref()
        .map(|tr| tr.broadband_precorrection)
        .unwrap_or(false)
        || room_config
            .optimizer
            .broadband_target_matching
            .as_ref()
            .map(|bb| bb.enabled)
            .unwrap_or(false);

    let (curve_for_optim, broadband_plugins, broadband_biquads, bb_mean_shift) =
        if broadband_enabled {
                info!("  Broadband pre-correction enabled...");

                // Detect F3 to avoid shelf-correcting below the speaker's rolloff.
                let detected_f3 = match excursion::detect_f3(&curve, None) {
                    Ok(f3_result) if f3_result.f3_hz > min_freq && f3_result.f3_hz < max_freq * 0.5 => {
                        info!(
                            "  Broadband: detected speaker F3={:.1}Hz",
                            f3_result.f3_hz
                        );
                        Some(f3_result.f3_hz)
                    }
                    _ => None,
                };
                let bb_min_freq = detected_f3.unwrap_or(min_freq);

                // Construct target at the measurement's mean level, INCLUDING the
                // target shape (tilt + preference). This ensures broadband and
                // optimizer pull toward the same goal — no double-tilt.
                let target = if let Some(ref tilt_curve) = target_tilt_curve {
                    Curve {
                        freq: curve.freq.clone(),
                        spl: &tilt_curve.spl + mean_spl,
                        phase: None,
                    }
                } else {
                    Curve {
                        freq: curve.freq.clone(),
                        spl: Array1::from_elem(curve.freq.len(), mean_spl),
                        phase: None,
                    }
                };

                // 2. Compute alignment within the speaker's passband (F3 to 20kHz).
                // The target is flat at mean_spl, so the alignment fits gentle
                // shelves + gain to correct the measurement's broadband shape.
                if let Some(mut result) = spectral_align::compute_target_alignment(
                    &curve,
                    &target,
                    bb_min_freq,
                    20000.0,
                    sample_rate,
                ) {
                    // Suppress the low-shelf when a rolloff is detected below the
                    // shelf frequency: the shelf response extends to DC and would
                    // partially boost the rolloff region, creating a worse shape
                    // than leaving it uncorrected.
                    if let Some(f3) = detected_f3
                        && f3 < spectral_align::LOWSHELF_FREQ {
                            info!(
                                "  Broadband: suppressing low-shelf (F3={:.1}Hz < shelf={:.1}Hz)",
                                f3, spectral_align::LOWSHELF_FREQ
                            );
                            result.lowshelf_gain_db = 0.0;
                        }
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

            // Pass 3: User Preference Filters (bass/treble shelves as separate pass)
            // When 3-pass mode is active, extract preference as separate output filters
            // instead of baking them into the target curve.
            // (reuses cea2034_active computed at the start of process_single_speaker)
            let preference_filters = if cea2034_active {
                if let Some(ref target_resp) = room_config.optimizer.target_response {
                    super::cea2034_correction::generate_preference_filters(
                        &target_resp.preference,
                        sample_rate,
                    )
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            // all_filters includes every biquad for response simulation only.
            // The DSP chain uses separate labeled plugins to avoid double-application.
            let mut all_filters = excursion_filters.clone();
            all_filters.extend(cea2034_filters.iter().cloned());
            all_filters.extend(broadband_biquads.iter().cloned());
            all_filters.extend(eq_filters.clone());
            all_filters.extend(preference_filters.iter().cloned());

            // Filters for the main EQ plugin in the chain (only excursion + room EQ).
            // CEA2034, broadband, and preference are added as separate labeled plugins.
            let mut main_eq_filters = excursion_filters.clone();
            main_eq_filters.extend(eq_filters.clone());

            // Build plugin chain: CEA2034 + broadband as separate plugins,
            // then main EQ, then preference — each applied exactly once.
            let mut pre_plugins = Vec::new();
            pre_plugins.extend(cea2034_plugins.iter().cloned());
            pre_plugins.extend(broadband_plugins.iter().cloned());

            let mut chain = output::build_channel_dsp_chain_with_curves(
                channel_name,
                None,
                pre_plugins,
                &main_eq_filters,
                None,
                None,
            );

            // Add Pass 3 preference EQ plugin if non-empty
            if !preference_filters.is_empty() {
                chain.plugins.push(output::create_labeled_eq_plugin(
                    &preference_filters,
                    "user_preference",
                ));
            }

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
pub(super) fn optimize_with_schroeder_split(
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
    let has_non_flat_target = optimizer
        .target_response
        .as_ref()
        .map(|tr| tr.shape != TargetShape::Flat)
        .unwrap_or(false)
        || optimizer.target_tilt.is_some();

    let low_max_db = if low_config.allow_boost {
        optimizer.max_db
    } else if has_non_flat_target {
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
pub(super) fn clamp_filter_q(filters: Vec<Biquad>, min_q: f64, max_q: f64) -> Vec<Biquad> {
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
pub(super) fn determine_optimization_bands(
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

