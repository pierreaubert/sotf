//! EQ optimization for individual channels
//!
//! Provides per-channel PEQ optimization using autoeq's workflow.

use crate::Curve;
use crate::cli::{Args, PeqModel};
use crate::loss::LossType;
use crate::workflow::setup_objective_data;
use clap::{Parser, ValueEnum};
use log::debug;
use math_audio_iir_fir::Biquad;
use ndarray::Array1;
use std::error::Error;

use super::impulse_analysis;
use super::spatial_robustness::{self, SpatialRobustnessConfig};
use super::types::{
    DecomposedCorrectionSerdeConfig, MultiMeasurementConfig, MultiMeasurementStrategy,
    OptimizerConfig, TargetCurveConfig,
};
use crate::optim::MultiObjectiveData;
use hound;
use math_audio_iir_fir::Peq;

/// Optimize EQ filters for a single channel using autoeq's workflow
///
/// # Arguments
/// * `curve` - Frequency response curve to optimize (on-axis measurement)
/// * `config` - Optimizer configuration
/// * `target_config` - Optional target curve configuration
/// * `sample_rate` - Sample rate for filter design
///
/// # Returns
/// * Tuple of (optimized Biquad filters, final loss value)
pub fn optimize_channel_eq(
    curve: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
    optimize_channel_eq_inner(curve, config, target_config, sample_rate, None)
}

/// Optimize EQ filters for a single channel with per-iteration progress callback
pub fn optimize_channel_eq_with_callback(
    curve: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
    callback: crate::optim::OptimProgressCallback,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
    optimize_channel_eq_inner(curve, config, target_config, sample_rate, Some(callback))
}

/// Prepared data for single-channel EQ optimization.
/// Contains all pre-processed data that is independent of filter count.
struct PreparedSingleChannelEq {
    objective_data: crate::optim::ObjectiveData,
    args_template: Args,
    peq_model: PeqModel,
    sample_rate: f64,
    /// Schroeder frequency in Hz above which the room stops being
    /// modal. Used by `run_optimization_pass` to clamp the gain upper
    /// bound of any filter whose entire frequency range lives below
    /// Schroeder — so the DE optimizer can't waste filter slots
    /// boosting modal nulls it physically cannot fill. `None` means
    /// decomposed-correction is disabled and no modal-region
    /// constraint is applied.
    schroeder_hz: Option<f64>,
}

/// Plausibility range for a measurement-driven Schroeder frequency.
///
/// Values outside this band almost always indicate a malformed IR
/// (e.g. a raw sweep capture instead of a deconvolved impulse
/// response), an incorrect `room_dimensions`, or a numerical quirk
/// in the T20 fit. Anything outside the range triggers a warning
/// and a fallback to the config-supplied value rather than silently
/// corrupting the modal-region bounds downstream.
///
/// 50 Hz floor: the lowest plausible Schroeder for a very large,
/// long-RT60 room (e.g. a large untreated home theatre). Below this
/// the measured value is almost certainly the result of an
/// over-long RT60 estimate from a contaminated IR.
///
/// 800 Hz ceiling: a very small room with a very short RT60 can
/// push Schroeder into the low-mid range, but values above ~800 Hz
/// would start clamping the bounds of filters that ought to be
/// free and is a strong indicator that the RT60 fit ran on a
/// truncated IR.
const SCHROEDER_PLAUSIBLE_MIN_HZ: f64 = 50.0;
const SCHROEDER_PLAUSIBLE_MAX_HZ: f64 = 800.0;

/// Find the length (in samples) at which to truncate an impulse
/// response so the Schroeder backward integration sees clean decay
/// without post-decay noise contamination.
///
/// Late microphone self-noise, HVAC rumble, or ambient pickup that
/// sits below the direct sound but above `-∞ dB` flattens the
/// Schroeder decay slope once the actual room decay has reached
/// the noise floor: the curve stops falling and levels off,
/// pulling the T20 slope fit shallower and inflating the measured
/// RT60. Truncating the IR at (or just past) the noise-floor
/// crossing removes that contamination cleanly — the backward
/// integral then sees `energy = 0` at the end of the buffer and
/// the slope stays true to the room's actual decay.
///
/// Single-pass variant of Lundeby's method:
/// 1. Window the IR into short 10 ms segments and compute each
///    segment's mean-squared energy.
/// 2. Estimate the noise floor as the mean energy of the last 10 %
///    of segments (assumed to be post-decay noise only).
/// 3. Walk backward and find the latest segment whose energy still
///    exceeds the noise floor by +10 dB (factor of 10) — this is
///    the last point where signal is cleanly above noise.
/// 4. Keep a few segments of headroom past that point (~30 ms) so
///    the T20 fit still has some decay curvature immediately
///    before the noise crossover, then return that length.
///
/// Returns the full IR length unchanged when:
/// - The buffer is shorter than `MIN_IR_LENGTH_FOR_TRIM_MS` (100 ms),
///   i.e. too short to have a meaningful tail region.
/// - Fewer than 20 windows fit in the buffer (tail fraction would
///   be a single window, not statistically meaningful).
/// - The noise-floor estimate is zero (a synthetic / perfectly
///   clean IR with digital silence tail — nothing to trim).
/// - No segment exceeds the +10 dB threshold (looks like pure
///   noise or a dead channel — don't mangle it).
fn trim_ir_length_to_noise_floor(ir: &[f32], sr: f32) -> usize {
    /// Segment duration in ms — short enough to resolve the decay
    /// crossover but long enough to smooth out individual sample
    /// noise.
    const WINDOW_MS: f32 = 10.0;
    /// Fraction of the buffer used to estimate the noise floor.
    const TAIL_FRACTION: f32 = 0.10;
    /// Signal-to-noise threshold above which a segment still
    /// counts as "signal" (linear, +10 dB).
    const SNR_THRESHOLD: f32 = 10.0;
    /// Extra segments kept past the last signal window so the
    /// T20 slope fit has some decay data immediately before the
    /// noise crossover.
    const HEADROOM_WINDOWS: usize = 3;
    /// Minimum IR length for any trimming to be attempted. Below
    /// this we can't build a meaningful tail-noise estimate.
    const MIN_IR_LENGTH_FOR_TRIM_MS: f32 = 100.0;

    let window_samples = (sr * WINDOW_MS / 1000.0) as usize;
    let min_samples = (sr * MIN_IR_LENGTH_FOR_TRIM_MS / 1000.0) as usize;
    if window_samples == 0 || ir.len() < min_samples {
        return ir.len();
    }
    let num_windows = ir.len() / window_samples;
    if num_windows < 20 {
        return ir.len();
    }

    // Mean squared energy per window.
    let energies: Vec<f32> = (0..num_windows)
        .map(|w| {
            let start = w * window_samples;
            let end = start + window_samples;
            let sum: f32 = ir[start..end].iter().map(|s| s * s).sum();
            sum / window_samples as f32
        })
        .collect();

    // Noise floor = mean of last TAIL_FRACTION of windows.
    let tail_count = ((num_windows as f32 * TAIL_FRACTION).ceil() as usize).max(1);
    let tail_start = num_windows - tail_count;
    let tail = &energies[tail_start..];
    let noise_floor: f32 = tail.iter().sum::<f32>() / tail.len() as f32;
    if noise_floor <= 0.0 {
        // Perfectly silent tail — nothing to gain from trimming.
        return ir.len();
    }

    let signal_threshold = noise_floor * SNR_THRESHOLD;
    let Some(last_signal_window) = energies
        .iter()
        .enumerate()
        .rev()
        .find(|(_, e)| **e > signal_threshold)
        .map(|(idx, _)| idx)
    else {
        // No segment exceeds the threshold — pure noise / dead
        // channel. Leave the buffer alone; downstream DSP will
        // return a zero RT60 and the helper falls back to config.
        return ir.len();
    };

    let keep_windows = (last_signal_window + 1 + HEADROOM_WINDOWS).min(num_windows);
    let keep_samples = (keep_windows * window_samples).min(ir.len());
    if keep_samples >= ir.len() {
        return ir.len();
    }

    log::debug!(
        "  IR noise-floor trim: {} → {} samples ({:.0} → {:.0} ms, \
         noise_floor={:.2e}, last_signal_window={}/{})",
        ir.len(),
        keep_samples,
        ir.len() as f32 * 1000.0 / sr,
        keep_samples as f32 * 1000.0 / sr,
        noise_floor,
        last_signal_window,
        num_windows,
    );
    keep_samples
}

/// Measure RT60 from the recorded IR in the bass region (125 /
/// 250 Hz octave bands) and return the bass-band RT60 that actually
/// governs modal decay. Broadband RT60 averages across the whole
/// spectrum, but bass RT60 is typically 1.5–2× mid-range RT60 in
/// real rooms and is what the Schroeder formula `2000 · √(RT60/V)`
/// is derived from, so the bass band is a systematically better
/// input than the broadband number.
///
/// Before running the band-pass analysis, the IR is truncated at
/// the late-noise floor via `trim_ir_length_to_noise_floor` so that
/// ambient noise / mic self-noise in the tail doesn't flatten the
/// Schroeder decay slope and inflate the measured RT60.
///
/// Returns `None` if neither band gave a positive fit (too short an
/// IR, too much floor noise, or a non-IR input).
fn measure_bass_rt60(mono_ir: &[f32], ir_sr: f32) -> Option<f64> {
    // Strip the noise tail so the Schroeder slope is computed only
    // over the clean signal decay. No-op for short or synthetic
    // IRs — see `trim_ir_length_to_noise_floor` for the exact
    // short-circuit conditions.
    let trim_len = trim_ir_length_to_noise_floor(mono_ir, ir_sr);
    let trimmed = &mono_ir[..trim_len];

    // The two octave bands immediately below the typical Schroeder
    // region. `compute_rt60_spectrum` runs a bandpass filter per
    // band and feeds each band to `compute_rt60_broadband`. Pick
    // the longer of the two valid values because (a) the lower
    // band is usually what governs modal density, and (b) taking
    // the max is conservative — it biases toward a higher
    // Schroeder frequency, which widens the modal-constraint
    // region rather than under-covering it.
    let bass_centers = [125.0_f32, 250.0];
    let bass_rt60s = math_audio_dsp::analysis::compute_rt60_spectrum(trimmed, ir_sr, &bass_centers);
    let rt60_max = bass_rt60s
        .iter()
        .copied()
        .filter(|v| *v > 0.0)
        .fold(0.0_f32, f32::max);
    if rt60_max > 0.0 {
        Some(rt60_max as f64)
    } else {
        None
    }
}

/// Decide whether a measured RT60 + `room_dimensions` should
/// override the config-supplied Schroeder frequency, returning
/// `Some(measured_schroeder_hz)` only when all preconditions hold:
///
/// 1. RT60 is positive (the fit succeeded).
/// 2. `room_dimensions` is present so we have a room volume V.
/// 3. The resulting Schroeder `2000 · √(RT60 / V)` lands in the
///    plausible range
///    [`SCHROEDER_PLAUSIBLE_MIN_HZ`, `SCHROEDER_PLAUSIBLE_MAX_HZ`].
///
/// Each branch logs the decision at info or warn level. The
/// function is intentionally free of DSP and file I/O so it can be
/// unit-tested with synthetic RT60 values — see the tests module
/// at the bottom of this file.
fn decide_schroeder_override(
    rt60_seconds: Option<f64>,
    dc_config: &DecomposedCorrectionSerdeConfig,
    current_schroeder_hz: f64,
) -> Option<f64> {
    let Some(rt60) = rt60_seconds.filter(|v| *v > 0.0) else {
        log::info!(
            "  RT60 fit failed on measured IR (bass bands 125/250 Hz) \
             — keeping config Schroeder value {:.1} Hz",
            current_schroeder_hz
        );
        return None;
    };

    log::info!(
        "  RT60 from measured IR (bass band, max of 125/250 Hz): {:.3} s \
         (Schroeder backward integration, T20 × 3)",
        rt60
    );

    let Some(dims) = dc_config.room_dimensions.as_ref() else {
        log::info!(
            "  Schroeder frequency: room_dimensions not provided, using \
             config value {:.1} Hz (measured RT60 is available but V is not)",
            current_schroeder_hz
        );
        return None;
    };

    let volume = dims.length * dims.width * dims.height;
    let measured = dims.schroeder_frequency_with_rt60(rt60);

    if measured <= 0.0 {
        log::warn!(
            "  Measured Schroeder non-positive ({:.3} Hz) for RT60={:.3} s, \
             V={:.1} m³ — keeping config value {:.1} Hz",
            measured,
            rt60,
            volume,
            current_schroeder_hz
        );
        return None;
    }

    if !(SCHROEDER_PLAUSIBLE_MIN_HZ..=SCHROEDER_PLAUSIBLE_MAX_HZ).contains(&measured) {
        log::warn!(
            "  Measured Schroeder {:.1} Hz outside plausible range [{:.0}, {:.0}] Hz \
             (RT60={:.3} s, V={:.1} m³) — keeping config value {:.1} Hz. \
             Check that ssir_wav_path is a deconvolved IR and room_dimensions are correct.",
            measured,
            SCHROEDER_PLAUSIBLE_MIN_HZ,
            SCHROEDER_PLAUSIBLE_MAX_HZ,
            rt60,
            volume,
            current_schroeder_hz,
        );
        return None;
    }

    log::info!(
        "  Schroeder frequency (measured): {:.1} Hz — overriding config value \
         {:.1} Hz (room V={:.1} m³, RT60={:.3} s)",
        measured,
        current_schroeder_hz,
        volume,
        rt60
    );
    Some(measured)
}

/// Prepare shared data for single-channel EQ optimization.
///
/// Handles normalization, psychoacoustic smoothing, target curve, deviation,
/// and objective data setup. The result is independent of filter count so it
/// can be reused across multiple optimization passes.
fn prepare_single_channel_eq(
    curve: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
) -> Result<PreparedSingleChannelEq, Box<dyn Error>> {
    // Clamp optimizer frequency range to measurement data range.
    let data_min_freq = curve.freq[0];
    let data_max_freq = curve.freq[curve.freq.len() - 1];
    let effective_min_freq = config.min_freq.max(data_min_freq);
    let effective_max_freq = config.max_freq.min(data_max_freq);

    let points_in_range = curve
        .freq
        .iter()
        .filter(|&&f| f >= effective_min_freq && f <= effective_max_freq)
        .count();
    log::info!(
        "  Optimizer freq range: configured=[{:.1}, {:.1}], data=[{:.1}, {:.1}], effective=[{:.1}, {:.1}], {} points in range",
        config.min_freq,
        config.max_freq,
        data_min_freq,
        data_max_freq,
        effective_min_freq,
        effective_max_freq,
        points_in_range,
    );
    if effective_max_freq < config.max_freq || effective_min_freq > config.min_freq {
        log::warn!(
            "  Clamping optimizer freq range [{:.1}, {:.1}] to measurement data range [{:.1}, {:.1}]",
            config.min_freq,
            config.max_freq,
            effective_min_freq,
            effective_max_freq
        );
    }

    // Normalize the input curve by subtracting the mean SPL in the optimization range
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..curve.freq.len() {
        if curve.freq[i] >= effective_min_freq && curve.freq[i] <= effective_max_freq {
            sum += curve.spl[i];
            count += 1;
        }
    }
    let mean_spl = if count > 0 { sum / count as f64 } else { 0.0 };
    let normalized_curve_unsmoothed = Curve {
        freq: curve.freq.clone(),
        spl: &curve.spl - mean_spl,
        phase: curve.phase.clone(),
        ..Default::default()
    };

    // Compute decomposed correction weights BEFORE psychoacoustic smoothing.
    // We keep both the per-frequency `correction_weights` (used to weight
    // the deviation the optimizer sees) and the list of detected
    // `room_modes` (used to seed the DE optimizer's smart-initial-guess
    // generator via `ObjectiveData.detected_problems`). Previously this
    // closure returned only the weights and the mode list was discarded
    // after logging — which meant the optimizer re-ran its own cruder
    // `find_peaks` over the smoothed deviation and landed on different
    // frequencies than the SSIR modes.
    let decomposed_result: Option<impulse_analysis::DecomposedCorrectionResult> = config
        .decomposed_correction
        .as_ref()
        .filter(|dc| dc.enabled)
        .map(|dc_config| {
            let mut dc_analysis_config = impulse_analysis::DecomposedCorrectionConfig {
                schroeder_freq: dc_config.schroeder_freq,
                min_mode_q: dc_config.min_mode_q,
                min_mode_prominence_db: dc_config.min_mode_prominence_db,
                mode_correction_weight: dc_config.mode_correction_weight,
                early_reflection_weight: dc_config.early_reflection_weight,
                steady_state_weight: dc_config.steady_state_weight,
                ..Default::default()
            };

            let result = if let Some(path) = config.ssir_wav_path.as_deref() {
                match try_ssir_analysis(path, sample_rate) {
                    Some((ssir_result, mono_ir, ir_sr)) => {
                        log::info!(
                            "  SSIR analysis: {} reflections, mixing time={:.1} ms",
                            ssir_result.num_reflections(),
                            ssir_result.mixing_time_ms(),
                        );

                        // Measurement-driven Schroeder frequency.
                        //
                        // The IR is already in memory, so instead of
                        // using the config-supplied `schroeder_freq`
                        // guess we measure the bass-band RT60 via
                        // `compute_rt60_spectrum` and plug it into
                        // `2000 · √(RT60 / V)` with V from
                        // `dc_config.room_dimensions`. The override is
                        // gated by `decide_schroeder_override` which
                        // requires the result to land in a plausible
                        // band — anything outside is treated as a
                        // malformed IR / wrong dimensions and we fall
                        // back to the config value. See that helper
                        // for the exact decision logic and its tests.
                        let rt60_bass = measure_bass_rt60(&mono_ir, ir_sr as f32);
                        if let Some(measured) = decide_schroeder_override(
                            rt60_bass,
                            dc_config,
                            dc_analysis_config.schroeder_freq,
                        ) {
                            dc_analysis_config.schroeder_freq = measured;
                        }

                        impulse_analysis::build_ssir_correction_weights(
                            &normalized_curve_unsmoothed.freq,
                            &normalized_curve_unsmoothed.spl,
                            &ssir_result,
                            &dc_analysis_config,
                        )
                    }
                    None => {
                        log::info!(
                            "  SSIR analysis failed, falling back to Schroeder-based decomposition"
                        );
                        impulse_analysis::analyze_decomposed_correction(
                            &normalized_curve_unsmoothed.freq,
                            &normalized_curve_unsmoothed.spl,
                            &dc_analysis_config,
                        )
                    }
                }
            } else {
                impulse_analysis::analyze_decomposed_correction(
                    &normalized_curve_unsmoothed.freq,
                    &normalized_curve_unsmoothed.spl,
                    &dc_analysis_config,
                )
            };

            log::info!(
                "  Decomposed correction: {} room modes detected, boundary={:.0} Hz",
                result.room_modes.len(),
                result.schroeder_freq,
            );
            for mode in &result.room_modes {
                log::info!(
                    "    Mode: {:.1} Hz, Q={:.1}, prominence={:.1} dB",
                    mode.frequency,
                    mode.q,
                    mode.prominence_db,
                );
            }
            result
        });

    let decomposed_weights = decomposed_result
        .as_ref()
        .map(|r| r.correction_weights.clone());

    // Convert detected room modes into `(freq_hz, q, gain_db)` seed
    // problems for the smart initial-guess generator. A mode is always
    // a *peak* in the smoothed response (that's how `detect_room_modes`
    // finds it), so the seeded filter is always a cut — gain is the
    // negative of the mode's prominence in dB. The list is sorted by
    // `|gain|` descending so the most prominent modes take priority
    // when the optimizer has fewer filters than modes.
    let detected_problems: Vec<(f64, f64, f64)> = match &decomposed_result {
        Some(r) => {
            let mut v: Vec<(f64, f64, f64)> = r
                .room_modes
                .iter()
                .map(|m| (m.frequency, m.q, -m.prominence_db))
                .collect();
            v.sort_by(|a, b| {
                b.2.abs()
                    .partial_cmp(&a.2.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            v
        }
        None => Vec::new(),
    };

    // Detect narrow nulls on the unsmoothed deviation curve and build a
    // per-sample suppression mask for the asymmetric loss dip branch.
    // High-Q dips = acoustic cancellation nulls that cannot be filled by
    // EQ boost; the mask drives their contribution to the loss toward
    // zero so the optimizer does not waste filters boosting into them.
    // Low-Q dips are left at full weight and stay legitimate correction
    // targets. The mask is only built when asymmetric loss is active —
    // other loss types do not consume it.
    let null_suppression_mask = if config.asymmetric_loss {
        let null_config = impulse_analysis::NullDetectionConfig::default();
        let nulls = impulse_analysis::detect_narrow_nulls(
            &normalized_curve_unsmoothed.freq,
            &normalized_curve_unsmoothed.spl,
            &null_config,
        );
        log::info!(
            "  Narrow-null detection: {} high-Q dip(s) suppressed for asymmetric loss",
            nulls.len()
        );
        for n in &nulls {
            log::info!(
                "    Null: {:.1} Hz, Q={:.1}, depth={:.1} dB",
                n.frequency,
                n.q,
                n.depth_db,
            );
        }
        Some(impulse_analysis::build_null_suppression_mask(
            &normalized_curve_unsmoothed.freq,
            &nulls,
        ))
    } else {
        None
    };

    // Apply psychoacoustic smoothing if enabled
    let mut normalized_curve = normalized_curve_unsmoothed;
    if config.psychoacoustic {
        log::info!("  Applying psychoacoustic smoothing (1/48 oct < 100 Hz, 1/6 oct > 1 kHz)");
        let smoothing_config = crate::read::PsychoacousticSmoothingConfig::default();
        normalized_curve = crate::read::smooth_psychoacoustic(&normalized_curve, &smoothing_config);
    }

    // Parse PEQ model
    let peq_model = PeqModel::from_str(&config.peq_model, true)
        .map_err(|e| format!("Invalid PEQ model '{}': {}", config.peq_model, e))?;

    // Create target curve
    let target_curve = match target_config {
        Some(TargetCurveConfig::Path(path)) => {
            let target = crate::read::read_curve_from_csv(path)?;
            crate::read::normalize_and_interpolate_response(&normalized_curve.freq, &target)
        }
        Some(TargetCurveConfig::Predefined(name)) => {
            let dummy_args = Args::parse_from(["autoeq", "--curve-name", name]);
            match crate::workflow::build_target_curve(
                &dummy_args,
                &normalized_curve.freq,
                &normalized_curve,
            ) {
                Ok(curve) => curve,
                Err(_) => {
                    debug!(
                        "  Target '{}' not a predefined curve, trying as file path...",
                        name
                    );
                    let target = crate::read::read_curve_from_csv(&std::path::PathBuf::from(name))?;
                    crate::read::normalize_and_interpolate_response(&normalized_curve.freq, &target)
                }
            }
        }
        None => Curve {
            freq: normalized_curve.freq.clone(),
            spl: Array1::zeros(normalized_curve.freq.len()),
            phase: None,
            ..Default::default()
        },
    };

    // Parse loss type
    let loss_type = match config.loss_type.as_str() {
        "flat" => {
            if config.asymmetric_loss {
                log::info!("  Using asymmetric loss (peaks penalized 2x more than dips)");
                LossType::SpeakerFlatAsymmetric
            } else {
                LossType::SpeakerFlat
            }
        }
        "score" => LossType::SpeakerScore,
        "epa" => LossType::Epa,
        _ => return Err(format!("Unknown loss type: {}", config.loss_type).into()),
    };

    // Build Args template (num_filters and maxeval will be overridden per pass)
    let args_template = build_args(
        config,
        effective_min_freq,
        effective_max_freq,
        sample_rate,
        loss_type,
        peq_model,
    );

    // Create deviation curve
    let raw_deviation = &target_curve.spl - &normalized_curve.spl;
    let final_deviation = if let Some(weights) = &decomposed_weights {
        &raw_deviation * weights
    } else {
        raw_deviation
    };
    let deviation_curve = Curve {
        freq: normalized_curve.freq.clone(),
        spl: final_deviation,
        phase: None,
        ..Default::default()
    };

    // Log deviation at key frequencies for diagnostics
    {
        let key_freqs = [30.0, 55.0, 80.0, 100.0, 150.0, 200.0, 300.0];
        let mut diag = String::from("  Deviation at key freqs:");
        for &kf in &key_freqs {
            if kf >= effective_min_freq && kf <= effective_max_freq {
                if let Some(idx) = deviation_curve
                    .freq
                    .iter()
                    .position(|&f| f >= kf * 0.95 && f <= kf * 1.05)
                {
                    diag.push_str(&format!(
                        " {:.0}Hz={:+.1}dB",
                        deviation_curve.freq[idx], deviation_curve.spl[idx]
                    ));
                }
            }
        }
        log::info!("{}", diag);
    }

    // Setup objective data
    let (mut objective_data, _use_cea) = setup_objective_data(
        &args_template,
        &normalized_curve,
        &target_curve,
        &deviation_curve,
        &None,
    )
    .expect("setup_objective_data should not fail without spin data");

    // Propagate frequency-dependent boost/cut envelopes for per-filter gain clamping
    objective_data.max_boost_envelope = config.max_boost_envelope.clone();
    objective_data.min_cut_envelope = config.min_cut_envelope.clone();
    // Propagate EPA config so compute_base_fitness uses user-provided
    // weights when loss_type == LossType::Epa.
    objective_data.epa_config = config.epa_config.clone();
    // Hand the SSIR / decomposed-correction mode list over to the DE
    // optimizer's smart initial-guess generator so filters actually
    // land on detected room modes instead of on whatever
    // `create_smart_initial_guesses::find_peaks` decides to flag.
    objective_data.detected_problems = detected_problems;
    // Hand the narrow-null suppression mask over to the asymmetric loss
    // branch of `compute_base_fitness`. `None` when `asymmetric_loss` is
    // disabled, in which case the loss does not consume the mask anyway.
    objective_data.null_suppression = null_suppression_mask;

    // Schroeder frequency — used downstream by `run_optimization_pass`
    // to forbid boost filters below the modal crossover. Pull it from
    // the decomposition result (which after the recent SSIR-path fix
    // just mirrors `config.decomposed_correction.schroeder_freq`).
    // When decomposition is disabled, leave as `None` and the
    // asymmetric bounds are not applied.
    let schroeder_hz = decomposed_result.as_ref().map(|r| r.schroeder_freq);

    Ok(PreparedSingleChannelEq {
        objective_data,
        args_template,
        peq_model,
        sample_rate,
        schroeder_hz,
    })
}

/// Run a single optimization pass with the given number of filters.
///
/// Returns (filters, loss, parameter_vector).
#[allow(clippy::type_complexity)]
fn run_optimization_pass(
    prep: &PreparedSingleChannelEq,
    num_filters: usize,
    max_iter: usize,
    config: &OptimizerConfig,
    callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<(Vec<Biquad>, f64, Vec<f64>), Box<dyn Error>> {
    let mut args = prep.args_template.clone();
    args.num_filters = num_filters;
    args.maxeval = max_iter;

    let (lower_bounds, mut upper_bounds) = crate::workflow::setup_bounds(&args);

    // Log per-filter frequency bounds for diagnostics
    {
        let ppf = crate::param_utils::params_per_filter(prep.peq_model);
        for i in 0..num_filters {
            let freq_idx = i * ppf;
            let f_low = 10.0_f64.powf(lower_bounds[freq_idx]);
            let f_high = 10.0_f64.powf(upper_bounds[freq_idx]);
            let gain_idx = freq_idx + ppf - 1;
            log::debug!(
                "  Filter {} bounds: freq=[{:.1}, {:.1}] Hz, gain=[{:+.1}, {:+.1}] dB",
                i,
                f_low,
                f_high,
                lower_bounds[gain_idx],
                upper_bounds[gain_idx],
            );
        }
    }

    // Physics constraint: below the Schroeder frequency the room is
    // modal — mode peaks can be cut with EQ but mode nulls can't be
    // filled with boost, so letting the optimizer place boost filters
    // there wastes slots and headroom. Clamp the gain upper bound of
    // any filter whose frequency range sits entirely below Schroeder
    // to 0 dB (cuts only). Skipped when decomposed-correction is
    // disabled (no trustworthy Schroeder value available).
    if let Some(sf) = prep.schroeder_hz {
        crate::workflow::restrict_boost_above_schroeder(&mut upper_bounds, &args, sf);
    }

    let mut x = crate::workflow::initial_guess(&args, &lower_bounds, &upper_bounds);

    // Global optimization
    let opt_result = if let Some(cb) = callback {
        crate::optim::optimize_filters_with_callback(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            prep.objective_data.clone(),
            &args,
            cb,
        )
    } else {
        crate::optim::optimize_filters(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            prep.objective_data.clone(),
            &args,
        )
    };

    let (converged_msg, global_loss) = match opt_result {
        Ok((msg, loss)) => (msg, loss),
        Err((msg, loss)) => {
            log::warn!("  Global optimization did not fully converge: {}", msg);
            (msg, loss)
        }
    };
    log::info!(
        "  Global optimizer result: {} (loss={:.6})",
        converged_msg,
        global_loss
    );

    // Local refinement (COBYLA)
    let final_loss = if config.refine {
        log::info!(
            "  Running local refinement ({}) from global loss={:.6}",
            config.local_algo,
            global_loss
        );
        let x_before_refine = x.to_vec();
        let local_result = crate::optim::optimize_filters_with_algo_override(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            prep.objective_data.clone(),
            &args,
            Some(&config.local_algo),
        );
        let local_loss = match local_result {
            Ok((_msg, loss)) => loss,
            Err((msg, loss)) => {
                log::warn!("  Local refinement did not converge: {}", msg);
                loss
            }
        };
        if local_loss < global_loss {
            log::info!(
                "  Local refinement: {:.6} -> {:.6} (improved {:.6})",
                global_loss,
                local_loss,
                global_loss - local_loss
            );
            local_loss
        } else {
            log::info!("  Local refinement did not improve, keeping global result");
            x.copy_from_slice(&x_before_refine);
            global_loss
        }
    } else {
        global_loss
    };

    // Apply boost and cut envelope clamps to the final result so deployed filters
    // respect the same gain limits used during fitness evaluation.
    let x_after_boost = if let Some(ref env) = prep.objective_data.max_boost_envelope {
        crate::optim::clamp_gains_to_envelope(&x, env, prep.peq_model)
    } else {
        x.to_vec()
    };
    let x_final = if let Some(ref env) = prep.objective_data.min_cut_envelope {
        crate::optim::clamp_cuts_to_envelope(&x_after_boost, env, prep.peq_model)
    } else {
        x_after_boost
    };

    // Convert to Biquad filters, pruning near-zero gain
    let peq = crate::x2peq::x2peq(&x_final, prep.sample_rate, prep.peq_model);
    let filters: Vec<Biquad> = peq
        .into_iter()
        .map(|(_weight, biquad)| biquad)
        .filter(|b| b.db_gain.abs() >= 0.05)
        .collect();

    Ok((filters, final_loss, x))
}

/// Forward iterative optimization: try 1..=max_filters, stop when improvement stalls.
fn optimize_channel_eq_adaptive(
    curve: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
    let prep = prepare_single_channel_eq(curve, config, target_config, sample_rate)?;
    let max_filters = config.num_filters;
    let budget_per_step = (config.max_iter / max_filters).max(5000);

    let mut best_filters: Vec<Biquad> = vec![];
    let mut best_loss = f64::INFINITY;

    log::info!(
        "  Adaptive filter selection: up to {} filters, threshold={:.6}, budget/step={}",
        max_filters,
        config.min_filter_improvement,
        budget_per_step
    );

    for k in 1..=max_filters {
        let (filters, loss, _x) = run_optimization_pass(&prep, k, budget_per_step, config, None)?;

        let improvement = best_loss - loss;
        log::info!(
            "  Adaptive: k={}/{}, loss={:.6}, improvement={:.6}",
            k,
            max_filters,
            loss,
            improvement
        );

        if k > 1 && improvement < config.min_filter_improvement {
            log::info!(
                "  Stopping at {} filters: improvement {:.6} < threshold {:.6}",
                k - 1,
                improvement,
                config.min_filter_improvement
            );
            break;
        }

        best_filters = filters;
        best_loss = loss;
    }

    // Backward elimination
    if config.elimination_threshold > 0.0 && best_filters.len() > 1 {
        let (pruned, pruned_loss) = backward_eliminate(
            best_filters,
            &prep.objective_data,
            prep.peq_model,
            config.elimination_threshold,
        );
        best_filters = pruned;
        best_loss = pruned_loss;
    }

    log::info!(
        "  Adaptive EQ optimization: {} filters, final loss={:.6}",
        best_filters.len(),
        best_loss
    );

    Ok((best_filters, best_loss))
}

/// Remove filters whose individual contribution is below the threshold.
///
/// Greedily removes the least-impactful filter, re-evaluates, and repeats
/// until no more filters can be removed without exceeding the threshold.
fn backward_eliminate(
    filters: Vec<Biquad>,
    objective_data: &crate::optim::ObjectiveData,
    peq_model: PeqModel,
    threshold: f64,
) -> (Vec<Biquad>, f64) {
    let mut remaining = filters;

    // Evaluate current loss from the full filter set
    let peq_vec: Peq = remaining.iter().map(|b| (1.0, b.clone())).collect();
    let x_full = crate::x2peq::peq2x(&peq_vec, peq_model);
    let mut current_loss = crate::optim::compute_base_fitness(&x_full, objective_data);

    loop {
        if remaining.len() <= 1 {
            break;
        }

        // Find the filter whose removal has the least impact on loss
        let mut min_impact = f64::INFINITY;
        let mut min_idx = 0;

        for i in 0..remaining.len() {
            let subset: Peq = remaining
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, b)| (1.0, b.clone()))
                .collect();

            let x_subset = crate::x2peq::peq2x(&subset, peq_model);
            let subset_loss = crate::optim::compute_base_fitness(&x_subset, objective_data);
            let impact = subset_loss - current_loss;

            if impact < min_impact {
                min_impact = impact;
                min_idx = i;
            }
        }

        if min_impact < threshold {
            log::info!(
                "  Backward elimination: removing filter at {:.0} Hz (impact={:.6} < threshold={:.6})",
                remaining[min_idx].freq,
                min_impact,
                threshold
            );
            remaining.remove(min_idx);
            current_loss += min_impact;
        } else {
            break;
        }
    }

    (remaining, current_loss)
}

fn optimize_channel_eq_inner(
    curve: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
    callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
    // Use adaptive filter selection when enabled and no callback
    if config.min_filter_improvement > 0.0 && config.num_filters > 1 && callback.is_none() {
        return optimize_channel_eq_adaptive(curve, config, target_config, sample_rate);
    }

    // Single-pass optimization (legacy path or callback path)
    let prep = prepare_single_channel_eq(curve, config, target_config, sample_rate)?;
    let (filters, loss, _x) =
        run_optimization_pass(&prep, config.num_filters, config.max_iter, config, callback)?;

    log::info!(
        "EQ optimization: {} filters, final loss={:.6}",
        filters.len(),
        loss
    );

    Ok((filters, loss))
}

/// Optimize EQ filters across multiple measurement curves simultaneously.
///
/// Finds a single shared EQ that works well across all measurements,
/// using the configured multi-measurement strategy to combine per-curve losses.
///
/// # Arguments
/// * `curves` - Multiple frequency response curves (different positions/measurements)
/// * `config` - Optimizer configuration
/// * `multi_config` - Multi-measurement strategy configuration
/// * `target_config` - Optional target curve configuration
/// * `sample_rate` - Sample rate for filter design
///
/// # Returns
/// * Tuple of (optimized Biquad filters, final loss value)
pub fn optimize_channel_eq_multi(
    curves: &[Curve],
    config: &OptimizerConfig,
    multi_config: &MultiMeasurementConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
    optimize_channel_eq_multi_inner(
        curves,
        config,
        multi_config,
        target_config,
        sample_rate,
        None,
    )
}

/// Optimize EQ across multiple measurement curves with per-iteration progress callback
pub fn optimize_channel_eq_multi_with_callback(
    curves: &[Curve],
    config: &OptimizerConfig,
    multi_config: &MultiMeasurementConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
    callback: crate::optim::OptimProgressCallback,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
    optimize_channel_eq_multi_inner(
        curves,
        config,
        multi_config,
        target_config,
        sample_rate,
        Some(callback),
    )
}

#[allow(clippy::too_many_arguments)]
fn optimize_channel_eq_multi_inner(
    curves: &[Curve],
    config: &OptimizerConfig,
    multi_config: &MultiMeasurementConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
    callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
    assert!(!curves.is_empty(), "curves must not be empty");

    // =========================================================================
    // SpatialRobustness strategy: early return with single-curve optimization
    // on the RMS-averaged curve, using correction depth mask to scale deviation.
    // =========================================================================
    if multi_config.strategy == MultiMeasurementStrategy::SpatialRobustness {
        return optimize_spatial_robustness(
            curves,
            config,
            multi_config,
            target_config,
            sample_rate,
            callback,
        );
    }

    // Clamp optimizer frequency range to the measurement data range of the first curve
    let data_min_freq = curves[0].freq[0];
    let data_max_freq = curves[0].freq[curves[0].freq.len() - 1];
    let effective_min_freq = config.min_freq.max(data_min_freq);
    let effective_max_freq = config.max_freq.min(data_max_freq);

    if effective_max_freq < config.max_freq || effective_min_freq > config.min_freq {
        log::warn!(
            "  Clamping optimizer freq range [{:.1}, {:.1}] to measurement data range [{:.1}, {:.1}]",
            config.min_freq,
            config.max_freq,
            effective_min_freq,
            effective_max_freq
        );
    }

    // Parse PEQ model
    let peq_model = PeqModel::from_str(&config.peq_model, true)
        .map_err(|e| format!("Invalid PEQ model '{}': {}", config.peq_model, e))?;

    // Parse loss type
    let loss_type = match config.loss_type.as_str() {
        "flat" => {
            if config.asymmetric_loss {
                log::info!("  Using asymmetric loss (peaks penalized 2x more than dips)");
                LossType::SpeakerFlatAsymmetric
            } else {
                LossType::SpeakerFlat
            }
        }
        "score" => LossType::SpeakerScore,
        "epa" => LossType::Epa,
        _ => return Err(format!("Unknown loss type: {}", config.loss_type).into()),
    };

    // Build one ObjectiveData per curve
    let mut objectives = Vec::with_capacity(curves.len());
    // We'll use the first curve to build Args and as the "primary"
    let mut primary_objective = None;

    for (i, curve) in curves.iter().enumerate() {
        // Normalize each curve independently
        let mut sum = 0.0;
        let mut count = 0;
        for j in 0..curve.freq.len() {
            if curve.freq[j] >= effective_min_freq && curve.freq[j] <= effective_max_freq {
                sum += curve.spl[j];
                count += 1;
            }
        }
        let mean_spl = if count > 0 { sum / count as f64 } else { 0.0 };
        let mut normalized_curve = Curve {
            freq: curve.freq.clone(),
            spl: &curve.spl - mean_spl,
            phase: curve.phase.clone(),
            ..Default::default()
        };

        // Apply psychoacoustic smoothing if enabled
        if config.psychoacoustic {
            if i == 0 {
                log::info!(
                    "  Applying psychoacoustic smoothing to {} curves",
                    curves.len()
                );
            }
            let smoothing_config = crate::read::PsychoacousticSmoothingConfig::default();
            normalized_curve =
                crate::read::smooth_psychoacoustic(&normalized_curve, &smoothing_config);
        }

        // Create target curve
        let target_curve = match target_config {
            Some(TargetCurveConfig::Path(path)) => {
                let target = crate::read::read_curve_from_csv(path)?;
                crate::read::normalize_and_interpolate_response(&normalized_curve.freq, &target)
            }
            Some(TargetCurveConfig::Predefined(name)) => {
                let dummy_args = Args::parse_from(["autoeq", "--curve-name", name]);
                match crate::workflow::build_target_curve(
                    &dummy_args,
                    &normalized_curve.freq,
                    &normalized_curve,
                ) {
                    Ok(curve) => curve,
                    Err(_) => {
                        let target =
                            crate::read::read_curve_from_csv(&std::path::PathBuf::from(name))?;
                        crate::read::normalize_and_interpolate_response(
                            &normalized_curve.freq,
                            &target,
                        )
                    }
                }
            }
            None => Curve {
                freq: normalized_curve.freq.clone(),
                spl: Array1::zeros(normalized_curve.freq.len()),
                phase: None,
                ..Default::default()
            },
        };

        let deviation_curve = Curve {
            freq: normalized_curve.freq.clone(),
            spl: &target_curve.spl - &normalized_curve.spl,
            phase: None,
            ..Default::default()
        };

        let (mut objective_data, _use_cea) = crate::workflow::setup_objective_data(
            &build_args(
                config,
                effective_min_freq,
                effective_max_freq,
                sample_rate,
                loss_type,
                peq_model,
            ),
            &normalized_curve,
            &target_curve,
            &deviation_curve,
            &None,
        )
        .expect("setup_objective_data should not fail without spin data");

        // Propagate EPA configuration from OptimizerConfig into the
        // ObjectiveData so `compute_base_fitness` uses the user-provided
        // weights when `loss_type == LossType::Epa`.
        objective_data.epa_config = config.epa_config.clone();

        if i == 0 {
            primary_objective = Some(objective_data.clone());
        }
        objectives.push(objective_data);
    }

    // Normalize weights
    let n = objectives.len();
    let weights = match &multi_config.weights {
        Some(w) if w.len() == n => {
            let sum: f64 = w.iter().sum();
            if sum > 0.0 {
                w.iter().map(|wi| wi / sum).collect()
            } else {
                vec![1.0 / n as f64; n]
            }
        }
        _ => vec![1.0 / n as f64; n],
    };

    let multi_data = MultiObjectiveData {
        objectives,
        strategy: multi_config.strategy.clone(),
        weights,
        variance_lambda: multi_config.variance_lambda,
    };

    // Wrap multi-objective data into the primary ObjectiveData
    let mut primary = primary_objective.unwrap();
    primary.multi_objective = Some(multi_data);

    let args = build_args(
        config,
        effective_min_freq,
        effective_max_freq,
        sample_rate,
        loss_type,
        peq_model,
    );

    // Setup bounds and initial guess
    let (lower_bounds, upper_bounds) = crate::workflow::setup_bounds(&args);
    let mut x = crate::workflow::initial_guess(&args, &lower_bounds, &upper_bounds);

    // Clone objective data for potential local refinement
    let primary_for_refine = if config.refine {
        Some(primary.clone())
    } else {
        None
    };

    // Run global optimization
    let opt_result = if let Some(cb) = callback {
        crate::optim::optimize_filters_with_callback(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            primary,
            &args,
            cb,
        )
    } else {
        crate::optim::optimize_filters(&mut x, &lower_bounds, &upper_bounds, primary, &args)
    };

    let (_converged_msg, global_loss) = match opt_result {
        Ok((msg, loss)) => (msg, loss),
        Err((msg, loss)) => {
            log::warn!(
                "  Multi-measurement global optimization did not fully converge: {}",
                msg
            );
            (msg, loss)
        }
    };

    // Local refinement (COBYLA) to polish the global solution
    let final_loss = if let Some(refine_data) = primary_for_refine {
        log::info!(
            "  Running local refinement ({}) from global loss={:.6}",
            config.local_algo,
            global_loss
        );
        let local_result = crate::optim::optimize_filters_with_algo_override(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            refine_data,
            &args,
            Some(&config.local_algo),
        );
        match local_result {
            Ok((_msg, loss)) => {
                log::info!(
                    "  Local refinement: {:.6} -> {:.6} (improved {:.6})",
                    global_loss,
                    loss,
                    global_loss - loss
                );
                loss
            }
            Err((msg, loss)) => {
                log::warn!("  Local refinement did not converge: {}", msg);
                loss
            }
        }
    } else {
        global_loss
    };

    let peq = crate::x2peq::x2peq(&x, sample_rate, args.peq_model);
    let filters: Vec<Biquad> = peq
        .into_iter()
        .map(|(_weight, biquad)| biquad)
        .filter(|b| b.db_gain.abs() >= 0.05)
        .collect();

    log::info!(
        "Multi-measurement EQ optimization ({:?}): {} filters, final loss={:.6}",
        multi_config.strategy,
        filters.len(),
        final_loss
    );

    Ok((filters, final_loss))
}

/// Helper to build Args from OptimizerConfig for optimization
fn build_args(
    config: &OptimizerConfig,
    effective_min_freq: f64,
    effective_max_freq: f64,
    sample_rate: f64,
    loss_type: LossType,
    peq_model: PeqModel,
) -> Args {
    Args {
        num_filters: config.num_filters,
        curve: None,
        target: None,
        speaker: None,
        version: None,
        measurement: None,
        curve_name: "On Axis".to_string(),
        sample_rate,
        min_freq: effective_min_freq,
        max_freq: effective_max_freq,
        min_q: config.min_q,
        max_q: config.max_q,
        min_db: config.min_db,
        max_db: config.max_db,
        algo: config.algorithm.clone(),
        strategy: config.strategy.clone(),
        algo_list: false,
        strategy_list: false,
        peq_model,
        peq_model_list: false,
        population: config.population,
        maxeval: config.max_iter,
        refine: config.refine,
        local_algo: config.local_algo.clone(),
        min_spacing_oct: 0.2,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: config.smooth_n,
        loss: loss_type,
        tolerance: config.tolerance,
        atolerance: config.atolerance,
        recombination: 0.9,
        adaptive_weight_f: 0.9,
        adaptive_weight_cr: 0.9,
        no_parallel: false,
        output: None,
        driver1: None,
        driver2: None,
        driver3: None,
        driver4: None,
        crossover_type: "linkwitzriley4".to_string(),
        parallel_threads: num_cpus::get(),
        seed: config.seed,
        qa: None,
        preset: None,
    }
}

/// Spatial robustness optimization.
///
/// Instead of running multi-objective optimization across all curves, this:
/// 1. Computes RMS power average across all positions
/// 2. Computes per-frequency spatial variance
/// 3. Builds a correction depth mask (high correction where consistent, low where variable)
/// 4. Scales the target deviation by the mask before single-curve optimization
///
/// The mask ensures the optimizer focuses filter resources on spatially consistent
/// features (room modes) and avoids wasting filters on position-dependent effects
/// (comb filtering from reflections).
fn optimize_spatial_robustness(
    curves: &[Curve],
    config: &OptimizerConfig,
    multi_config: &MultiMeasurementConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
    callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
    // Build spatial robustness config from serde config or defaults
    let sr_config = match &multi_config.spatial_robustness {
        Some(sc) => SpatialRobustnessConfig {
            variance_threshold_db: sc.variance_threshold_db,
            transition_width_db: sc.transition_width_db,
            min_correction_depth: sc.min_correction_depth,
            mask_smoothing_octaves: sc.mask_smoothing_octaves,
        },
        None => SpatialRobustnessConfig::default(),
    };

    // Analyze spatial robustness
    let analysis = spatial_robustness::analyze_spatial_robustness(curves, &sr_config);

    log::info!(
        "  Spatial robustness: {} positions, variance range {:.1}-{:.1} dB",
        curves.len(),
        analysis
            .spatial_variance
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min),
        analysis
            .spatial_variance
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max),
    );

    let mean_depth =
        analysis.correction_depth.iter().sum::<f64>() / analysis.correction_depth.len() as f64;
    log::info!(
        "  Correction depth: mean={:.2}, min={:.2}, max={:.2}",
        mean_depth,
        analysis
            .correction_depth
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min),
        analysis
            .correction_depth
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max),
    );

    // Use the RMS-averaged curve as input to the single-curve optimizer.
    // The correction depth mask is applied by scaling the deviation curve:
    // where depth is low, the deviation appears small → optimizer won't place filters there.
    let averaged_curve = &analysis.averaged_curve;

    // Clamp frequency range
    let data_min_freq = averaged_curve.freq[0];
    let data_max_freq = averaged_curve.freq[averaged_curve.freq.len() - 1];
    let effective_min_freq = config.min_freq.max(data_min_freq);
    let effective_max_freq = config.max_freq.min(data_max_freq);

    // Normalize by subtracting mean SPL in optimization range
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..averaged_curve.freq.len() {
        if averaged_curve.freq[i] >= effective_min_freq
            && averaged_curve.freq[i] <= effective_max_freq
        {
            sum += averaged_curve.spl[i];
            count += 1;
        }
    }
    let mean_spl = if count > 0 { sum / count as f64 } else { 0.0 };
    let mut normalized_curve = Curve {
        freq: averaged_curve.freq.clone(),
        spl: &averaged_curve.spl - mean_spl,
        phase: averaged_curve.phase.clone(),
        ..Default::default()
    };

    // Apply psychoacoustic smoothing if enabled
    if config.psychoacoustic {
        log::info!("  Applying psychoacoustic smoothing to spatially averaged curve");
        let smoothing_config = crate::read::PsychoacousticSmoothingConfig::default();
        normalized_curve = crate::read::smooth_psychoacoustic(&normalized_curve, &smoothing_config);
    }

    // Parse PEQ model
    let peq_model = PeqModel::from_str(&config.peq_model, true)
        .map_err(|e| format!("Invalid PEQ model '{}': {}", config.peq_model, e))?;

    // Parse loss type
    let loss_type = match config.loss_type.as_str() {
        "flat" => {
            if config.asymmetric_loss {
                LossType::SpeakerFlatAsymmetric
            } else {
                LossType::SpeakerFlat
            }
        }
        "score" => LossType::SpeakerScore,
        "epa" => LossType::Epa,
        _ => return Err(format!("Unknown loss type: {}", config.loss_type).into()),
    };

    // Build target curve
    let target_curve = match target_config {
        Some(TargetCurveConfig::Path(path)) => {
            let target = crate::read::read_curve_from_csv(path)?;
            crate::read::normalize_and_interpolate_response(&normalized_curve.freq, &target)
        }
        Some(TargetCurveConfig::Predefined(name)) => {
            let dummy_args = Args::parse_from(["autoeq", "--curve-name", name]);
            match crate::workflow::build_target_curve(
                &dummy_args,
                &normalized_curve.freq,
                &normalized_curve,
            ) {
                Ok(curve) => curve,
                Err(_) => {
                    let target = crate::read::read_curve_from_csv(&std::path::PathBuf::from(name))?;
                    crate::read::normalize_and_interpolate_response(&normalized_curve.freq, &target)
                }
            }
        }
        None => Curve {
            freq: normalized_curve.freq.clone(),
            spl: Array1::zeros(normalized_curve.freq.len()),
            phase: None,
            ..Default::default()
        },
    };

    // Compute raw deviation
    let raw_deviation = &target_curve.spl - &normalized_curve.spl;

    // Apply correction depth mask to deviation.
    // This is the key spatial robustness step: the deviation at frequencies where the
    // spatial variance is high gets scaled down, so the optimizer doesn't try to correct
    // position-dependent features.
    let masked_deviation = &raw_deviation * &analysis.correction_depth;

    let deviation_curve = Curve {
        freq: normalized_curve.freq.clone(),
        spl: masked_deviation,
        phase: None,
        ..Default::default()
    };

    let args = build_args(
        config,
        effective_min_freq,
        effective_max_freq,
        sample_rate,
        loss_type,
        peq_model,
    );

    // Setup objective data with the masked deviation
    let (mut objective_data, _use_cea) = setup_objective_data(
        &args,
        &normalized_curve,
        &target_curve,
        &deviation_curve,
        &None,
    )
    .expect("setup_objective_data should not fail without spin data");

    // Propagate EPA config so compute_base_fitness uses user-provided
    // weights when loss_type == LossType::Epa.
    objective_data.epa_config = config.epa_config.clone();

    let (lower_bounds, upper_bounds) = crate::workflow::setup_bounds(&args);
    let mut x = crate::workflow::initial_guess(&args, &lower_bounds, &upper_bounds);

    let opt_result = if let Some(cb) = callback {
        crate::optim::optimize_filters_with_callback(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            objective_data,
            &args,
            cb,
        )
    } else {
        crate::optim::optimize_filters(&mut x, &lower_bounds, &upper_bounds, objective_data, &args)
    };

    let (_converged_msg, final_loss) = match opt_result {
        Ok((msg, loss)) => (msg, loss),
        Err((msg, loss)) => {
            eprintln!(
                "  Warning: spatial robustness optimization did not fully converge: {}",
                msg
            );
            (msg, loss)
        }
    };

    let peq = crate::x2peq::x2peq(&x, sample_rate, args.peq_model);
    let filters: Vec<Biquad> = peq
        .into_iter()
        .map(|(_weight, biquad)| biquad)
        .filter(|b| b.db_gain.abs() >= 0.05)
        .collect();

    log::info!(
        "Spatial robustness EQ: {} filters, final loss={:.6}",
        filters.len(),
        final_loss
    );

    Ok((filters, final_loss))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    fn make_synthetic_room_curve() -> Curve {
        // 500-point log-spaced curve 20-20kHz with room modes
        let n = 500;
        let log_min = 20.0_f64.ln();
        let log_max = 20000.0_f64.ln();
        let freqs: Vec<f64> = (0..n)
            .map(|i| (log_min + (log_max - log_min) * i as f64 / (n - 1) as f64).exp())
            .collect();
        let spl: Vec<f64> = freqs
            .iter()
            .map(|&f| {
                let mode1 = 10.0 * (-((f.log2() - 80.0_f64.log2()).powi(2)) / 0.3).exp();
                let mode2 = 8.0 * (-((f.log2() - 250.0_f64.log2()).powi(2)) / 0.2).exp();
                let dip = -6.0 * (-((f.log2() - 500.0_f64.log2()).powi(2)) / 0.4).exp();
                mode1 + mode2 + dip
            })
            .collect();
        Curve {
            freq: Array1::from_vec(freqs),
            spl: Array1::from_vec(spl),
            phase: None,
            ..Default::default()
        }
    }

    /// Regression test: refine step must run when config.refine=true.
    /// Bug: optimize_channel_eq_inner called optimize_filters directly,
    /// bypassing perform_optimization which contains the refine path.
    #[test]
    fn optimize_channel_eq_runs_refine_when_enabled() {
        let curve = make_synthetic_room_curve();
        let config_no_refine = OptimizerConfig {
            algorithm: "autoeq:de".to_string(),
            strategy: "lshade".to_string(),
            num_filters: 3,
            max_iter: 5000,
            population: 20,
            refine: false,
            seed: Some(42),
            tolerance: 1e-3,
            atolerance: 1e-3,
            min_filter_improvement: 0.0, // Use single-pass for this test
            ..OptimizerConfig::default()
        };
        let config_with_refine = OptimizerConfig {
            refine: true,
            ..config_no_refine.clone()
        };

        let (filters_no, loss_no) = optimize_channel_eq(&curve, &config_no_refine, None, 48000.0)
            .expect("optimization should succeed");
        let (filters_yes, loss_yes) =
            optimize_channel_eq(&curve, &config_with_refine, None, 48000.0)
                .expect("optimization should succeed");

        // Refine should produce equal or better loss.
        // Allow small tolerance for parallel DE floating-point non-determinism.
        assert!(
            loss_yes <= loss_no * 1.01,
            "refine should not significantly worsen loss: no_refine={:.6}, refine={:.6}",
            loss_no,
            loss_yes
        );
        // Both should produce non-empty filters
        assert!(!filters_no.is_empty(), "no_refine should produce filters");
        assert!(!filters_yes.is_empty(), "refine should produce filters");
    }

    /// Verify that LSHADE strategy is accepted and produces valid results.
    #[test]
    fn optimize_channel_eq_with_lshade_strategy() {
        let curve = make_synthetic_room_curve();
        let config = OptimizerConfig {
            algorithm: "autoeq:de".to_string(),
            strategy: "lshade".to_string(),
            num_filters: 5,
            max_iter: 5000,
            population: 20,
            seed: Some(42),
            tolerance: 1e-3,
            atolerance: 1e-3,
            ..OptimizerConfig::default()
        };

        let (filters, loss) = optimize_channel_eq(&curve, &config, None, 48000.0)
            .expect("LSHADE optimization should succeed");

        assert!(!filters.is_empty(), "should produce filters");
        assert!(loss < 5.0, "loss should be reasonable, got {:.4}", loss);
    }

    // ---------------------------------------------------------------
    // decide_schroeder_override: pure-logic helper for the
    // measurement-driven Schroeder frequency path. DSP-free so we
    // pass synthetic RT60 values directly and verify the four
    // branches: (1) accepted override in range, (2) rejected
    // implausible-high, (3) rejected implausible-low, (4) no
    // room_dimensions → fallback, (5) RT60 fit failed → fallback.
    // ---------------------------------------------------------------

    fn dc_config_with_dims(
        length: f64,
        width: f64,
        height: f64,
    ) -> DecomposedCorrectionSerdeConfig {
        DecomposedCorrectionSerdeConfig {
            room_dimensions: Some(crate::roomeq::RoomDimensions {
                length,
                width,
                height,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn decide_schroeder_override_accepts_plausible_measurement() {
        // 30 m³ room, RT60 = 0.4 s → Schroeder ≈ 2000·√(0.4/30) ≈ 231 Hz
        // (well inside [50, 800] Hz)
        let dc = dc_config_with_dims(4.0, 3.0, 2.5);
        let result = decide_schroeder_override(Some(0.4), &dc, 250.0);
        let measured = result.expect("should override with plausible value");
        assert!(
            (measured - 231.0).abs() < 2.0,
            "expected ~231 Hz, got {:.1} Hz",
            measured
        );
    }

    #[test]
    fn decide_schroeder_override_rejects_implausibly_high() {
        // Tiny room (1 m³) with a very short RT60 (0.1 s) →
        // Schroeder = 2000·√(0.1/1) = 632 Hz — still plausible.
        // Push it harder: 0.5 m³ volume, 0.1 s → 2000·√(0.1/0.5) ≈ 894 Hz
        // (outside the 800 Hz ceiling). This simulates a malformed
        // IR whose T20 slope came out steep enough to under-report
        // RT60 against a too-small user-supplied volume.
        let dc = dc_config_with_dims(1.0, 1.0, 0.5);
        let result = decide_schroeder_override(Some(0.1), &dc, 250.0);
        assert!(
            result.is_none(),
            "894 Hz is outside [50, 800], should reject, got {:?}",
            result
        );
    }

    #[test]
    fn decide_schroeder_override_rejects_implausibly_low() {
        // Huge room (1000 m³) with a very long contaminated RT60
        // (10 s) → Schroeder = 2000·√(10/1000) = 200 Hz — still
        // plausible. Push harder: 10000 m³ (impossible for a
        // listening room) with 10 s → 63 Hz, still just in range.
        // Use 10000 m³ + 1 s → 20 Hz which is below the 50 Hz floor.
        // This simulates either a pathologically large V or a
        // truncated RT60 fit that under-reports modal decay.
        let dc = dc_config_with_dims(100.0, 100.0, 1.0);
        let result = decide_schroeder_override(Some(1.0), &dc, 250.0);
        assert!(
            result.is_none(),
            "20 Hz is outside [50, 800], should reject, got {:?}",
            result
        );
    }

    #[test]
    fn decide_schroeder_override_falls_back_without_room_dimensions() {
        // RT60 was measurable but the user didn't provide room
        // dimensions → we can't plug into the formula, fall back.
        let dc = DecomposedCorrectionSerdeConfig::default();
        assert!(dc.room_dimensions.is_none());
        let result = decide_schroeder_override(Some(0.4), &dc, 250.0);
        assert!(result.is_none());
    }

    #[test]
    fn decide_schroeder_override_falls_back_on_rt60_fit_failure() {
        // compute_rt60_spectrum returned 0.0 (or all-zeros) for all
        // bass bands → `measure_bass_rt60` yields None → helper
        // sees None and falls back to config.
        let dc = dc_config_with_dims(4.0, 3.0, 2.5);
        let result = decide_schroeder_override(None, &dc, 250.0);
        assert!(result.is_none());
    }

    #[test]
    fn decide_schroeder_override_falls_back_on_zero_rt60() {
        // Defensive: filter also rejects explicitly-zero RT60 so the
        // caller doesn't have to pre-filter.
        let dc = dc_config_with_dims(4.0, 3.0, 2.5);
        let result = decide_schroeder_override(Some(0.0), &dc, 250.0);
        assert!(result.is_none());
    }

    // ---------------------------------------------------------------
    // trim_ir_length_to_noise_floor: single-pass Lundeby-lite. We
    // construct synthetic IRs (clean decay, clean + noise tail,
    // zero, sinusoid) at 48 kHz and assert the truncation length
    // matches the expected behaviour for each case.
    // ---------------------------------------------------------------

    /// Deterministic pseudo-random noise via a linear congruential
    /// generator. Same seed → same output, so tests stay
    /// reproducible without pulling `rand` into autoeq's
    /// dev-dependencies.
    fn lcg_noise(n: usize, seed: u32, amplitude: f32) -> Vec<f32> {
        let mut state = seed;
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                let normalised = (state as f32) / (u32::MAX as f32) - 0.5;
                normalised * 2.0 * amplitude
            })
            .collect()
    }

    /// Synthesise an exponentially decaying impulse response with a
    /// given RT60. Amplitude envelope is `exp(-k·t)` with
    /// `k = 3·ln(10)/RT60`, chosen so the squared envelope
    /// (Schroeder decay input) reaches −60 dB exactly at `t=RT60`.
    fn make_exponential_decay(num_samples: usize, sr: f32, rt60_seconds: f32) -> Vec<f32> {
        let k = 3.0 * std::f32::consts::LN_10 / rt60_seconds;
        (0..num_samples)
            .map(|i| {
                let t = i as f32 / sr;
                (-k * t).exp()
            })
            .collect()
    }

    #[test]
    fn trim_passes_short_ir_through_unchanged() {
        // 50 ms @ 48 kHz = 2400 samples, well below the 100 ms
        // minimum length. No trimming regardless of content.
        let sr = 48_000.0_f32;
        let ir = make_exponential_decay(2_400, sr, 0.2);
        assert_eq!(trim_ir_length_to_noise_floor(&ir, sr), ir.len());
    }

    #[test]
    fn trim_passes_all_zero_ir_through_unchanged() {
        // All-zero buffer → tail-noise estimate is 0 → early return,
        // keep full length. The DSP side will then fail the T20 fit
        // and return 0 RT60, which measure_bass_rt60 turns into None.
        let sr = 48_000.0_f32;
        let ir = vec![0.0_f32; 48_000];
        assert_eq!(trim_ir_length_to_noise_floor(&ir, sr), ir.len());
    }

    #[test]
    fn trim_passes_pure_noise_through_unchanged() {
        // Constant-amplitude LCG noise with near-uniform per-window
        // energy → no window exceeds the tail noise floor by +10 dB
        // → `find` returns None → full length kept. This matches
        // the doc guarantee that we don't mangle dead channels.
        let sr = 48_000.0_f32;
        let ir = lcg_noise(48_000, 0xDEADBEEF, 0.1);
        assert_eq!(trim_ir_length_to_noise_floor(&ir, sr), ir.len());
    }

    #[test]
    fn trim_cuts_clean_decay_with_strong_noise_tail() {
        // First 500 ms = exponential decay with RT60 = 0.5 s (so
        // amplitude hits ~1e-3 = −60 dB by the end of the decay
        // region). Next 500 ms = steady LCG noise at amplitude 0.01
        // (energy ≈ 1e-4, +10 dB threshold at energy 1e-3 which the
        // decay crosses around t ≈ 250 ms). The trim length must
        // therefore be well below the full 1 s buffer (confirming
        // the noise tail is cut) but still long enough that the
        // T20 fit (which only needs up to ~170 ms of decay for
        // RT60 = 0.5 s) has room to run.
        let sr = 48_000.0_f32;
        let full = 48_000_usize; // 1.0 s
        let decay_samples = 24_000_usize; // 500 ms
        let mut ir = make_exponential_decay(decay_samples, sr, 0.5);
        ir.extend(lcg_noise(full - decay_samples, 0x1234_5678, 0.01));
        assert_eq!(ir.len(), full);

        let kept = trim_ir_length_to_noise_floor(&ir, sr);
        // Must cut at least half the buffer (the noise tail).
        assert!(
            kept < full * 3 / 4,
            "expected trim below 75 % of buffer, got {} of {}",
            kept,
            full
        );
        // Must keep at least the first 170 ms so the T20 fit still
        // has its −5 → −25 dB span (~170 ms for RT60 = 500 ms).
        let min_keep = (sr * 0.170) as usize;
        assert!(
            kept >= min_keep,
            "expected trim above T20 span ({} samples), got {}",
            min_keep,
            kept
        );
    }

    #[test]
    fn trim_keeps_most_of_a_clean_decay_without_noise() {
        // Pure exponential decay with a digital-silence tail (the
        // decay has died below f32 subnormal range by the end of
        // the buffer). The tail-noise estimate is effectively 0, so
        // the early-return fires and the whole buffer is kept.
        let sr = 48_000.0_f32;
        let ir = make_exponential_decay(48_000, sr, 0.1);
        // RT60 = 100 ms so by 1 s we're at exp(-k*1) with
        // k = 3*ln(10)/0.1 ≈ 69.08 → exp(-69) ≈ 1e-30, which is
        // below f32 subnormal range and reads as 0. Tail is all
        // zero → noise_floor = 0 → early return.
        let kept = trim_ir_length_to_noise_floor(&ir, sr);
        assert_eq!(kept, ir.len(), "perfectly clean decay must be kept whole");
    }
}

// ============================================================================
// Processing Mode Integration Tests
// ============================================================================

#[cfg(test)]
mod processing_mode_tests {
    use super::*;
    use crate::roomeq::mixed_phase::MixedPhaseConfig;
    use crate::roomeq::types::{FirConfig, ProcessingMode};

    fn make_simple_room_curve() -> Curve {
        let n = 100;
        let log_min = 20.0_f64.ln();
        let log_max = 20000.0_f64.ln();
        let freqs: Vec<f64> = (0..n)
            .map(|i| (log_min + (log_max - log_min) * i as f64 / (n - 1) as f64).exp())
            .collect();
        let spl: Vec<f64> = freqs
            .iter()
            .map(|&f| 10.0 * (-((f.log2() - 80.0_f64.log2()).powi(2) / 0.3).exp()))
            .collect();
        Curve {
            freq: Array1::from_vec(freqs),
            spl: Array1::from_vec(spl),
            phase: None,
            ..Default::default()
        }
    }

    fn make_room_curve_with_phase() -> Curve {
        let n = 100;
        let log_min = 20.0_f64.ln();
        let log_max = 20000.0_f64.ln();
        let freqs: Vec<f64> = (0..n)
            .map(|i| (log_min + (log_max - log_min) * i as f64 / (n - 1) as f64).exp())
            .collect();
        let spl: Vec<f64> = freqs
            .iter()
            .map(|&f| 10.0 * (-((f.log2() - 80.0_f64.log2()).powi(2) / 0.3).exp()))
            .collect();
        // Add minimum phase (negative group delay = phase leading)
        let phase: Vec<f64> = freqs
            .iter()
            .map(|&f| -30.0 * (f / 1000.0).log10())
            .collect();
        Curve {
            freq: Array1::from_vec(freqs),
            spl: Array1::from_vec(spl),
            phase: Some(Array1::from_vec(phase)),
            ..Default::default()
        }
    }

    /// Test LowLatency mode (IIR only) - default processing mode
    #[test]
    fn test_processing_mode_lowlatency_config() {
        let config = OptimizerConfig {
            processing_mode: ProcessingMode::LowLatency,
            ..OptimizerConfig::default()
        };
        assert_eq!(config.processing_mode, ProcessingMode::LowLatency);
    }

    /// Test LowLatency mode produces valid IIR filters
    #[test]
    fn test_optimize_channel_eq_lowlatency() {
        let curve = make_simple_room_curve();
        let config = OptimizerConfig {
            processing_mode: ProcessingMode::LowLatency,
            algorithm: "autoeq:de".to_string(),
            strategy: "lshade".to_string(),
            num_filters: 3,
            max_iter: 1000,
            population: 10,
            seed: Some(42),
            ..OptimizerConfig::default()
        };

        let result = optimize_channel_eq(&curve, &config, None, 48000.0);
        assert!(result.is_ok(), "LowLatency optimization should succeed");
        let (filters, loss) = result.unwrap();
        assert!(!filters.is_empty(), "should produce IIR filters");
        assert!(loss.is_finite(), "loss should be finite, got {}", loss);
    }

    /// Test PhaseLinear mode configuration
    #[test]
    fn test_processing_mode_phaselinear_config() {
        let fir_config = FirConfig {
            taps: 4096,
            phase: "kirkeby".to_string(),
            correct_excess_phase: false,
            phase_smoothing: 0.167,
            pre_ringing: None,
        };
        let config = OptimizerConfig {
            processing_mode: ProcessingMode::PhaseLinear,
            fir: Some(fir_config),
            ..OptimizerConfig::default()
        };
        assert_eq!(config.processing_mode, ProcessingMode::PhaseLinear);
        assert!(config.fir.is_some());
    }

    /// Test Hybrid mode configuration
    #[test]
    fn test_processing_mode_hybrid_config() {
        let fir_config = FirConfig {
            taps: 4096,
            phase: "kirkeby".to_string(),
            correct_excess_phase: false,
            phase_smoothing: 0.167,
            pre_ringing: None,
        };
        let config = OptimizerConfig {
            processing_mode: ProcessingMode::Hybrid,
            fir: Some(fir_config),
            ..OptimizerConfig::default()
        };
        assert_eq!(config.processing_mode, ProcessingMode::Hybrid);
    }

    /// Test MixedPhase mode configuration
    #[test]
    fn test_processing_mode_mixedphase_config() {
        use crate::roomeq::types::MixedPhaseSerdeConfig;
        let mixed_phase_config = MixedPhaseSerdeConfig {
            max_fir_length_ms: 10.0,
            pre_ringing_threshold_db: -30.0,
            min_spatial_depth: 0.5,
            phase_smoothing_octaves: 1.0 / 6.0,
        };
        let config = OptimizerConfig {
            processing_mode: ProcessingMode::MixedPhase,
            mixed_phase: Some(mixed_phase_config),
            ..OptimizerConfig::default()
        };
        assert_eq!(config.processing_mode, ProcessingMode::MixedPhase);
        assert!(config.mixed_phase.is_some());
    }

    /// Test MixedPhase mode requires phase data
    #[test]
    fn test_mixedphase_requires_phase_data() {
        let curve_without_phase = make_simple_room_curve();
        assert!(curve_without_phase.phase.is_none());

        // MixedPhaseConfig should be used but decompose_phase will fail without phase
        let config = MixedPhaseConfig::default();
        let result = crate::roomeq::mixed_phase::decompose_phase(&curve_without_phase, &config);
        assert!(result.is_err(), "MixedPhase should fail without phase data");
    }

    /// Test MixedPhase mode with phase data succeeds
    #[test]
    fn test_mixedphase_with_phase_data() {
        let curve_with_phase = make_room_curve_with_phase();
        assert!(curve_with_phase.phase.is_some());

        let config = MixedPhaseConfig::default();
        let result = crate::roomeq::mixed_phase::decompose_phase(&curve_with_phase, &config);
        assert!(
            result.is_ok(),
            "MixedPhase should succeed with phase data: {:?}",
            result.err()
        );
    }

    /// Test that ProcessingMode enum has expected variants
    #[test]
    fn test_processing_mode_variants() {
        // Verify all variants exist and can be compared
        let modes = [
            ProcessingMode::LowLatency,
            ProcessingMode::PhaseLinear,
            ProcessingMode::Hybrid,
            ProcessingMode::MixedPhase,
        ];

        // Verify each variant is different from others
        assert_ne!(modes[0], modes[1]);
        assert_ne!(modes[0], modes[2]);
        assert_ne!(modes[0], modes[3]);
        assert_ne!(modes[1], modes[2]);
        assert_ne!(modes[1], modes[3]);
        assert_ne!(modes[2], modes[3]);
    }
}

// ============================================================================
// Regression Tests for Harman Target Curve
// ============================================================================

#[cfg(test)]
mod harman_regression_tests {
    use super::*;
    use crate::roomeq::target_tilt::build_complete_target_curve;
    use crate::roomeq::types::{TargetResponseConfig, TargetShape, UserPreference};

    fn make_curve_with_freqs(freqs: Vec<f64>, spl: Vec<f64>) -> Curve {
        Curve {
            freq: Array1::from_vec(freqs),
            spl: Array1::from_vec(spl),
            phase: None,
            ..Default::default()
        }
    }

    fn harman_curve(freqs: &[f64], bass_shelf_db: f64) -> Curve {
        let config = TargetResponseConfig {
            shape: TargetShape::Harman,
            preference: UserPreference {
                bass_shelf_db,
                bass_shelf_freq: 200.0,
                ..Default::default()
            },
            ..Default::default()
        };
        build_complete_target_curve(&Array1::from_vec(freqs.to_vec()), &config)
    }

    /// Regression test: optimization should not produce NaN or Inf loss with Harman target
    #[test]
    fn test_harman_target_no_nan_loss() {
        let freqs = vec![100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0];
        let spl = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let curve = make_curve_with_freqs(freqs, spl);

        let config = OptimizerConfig {
            algorithm: "autoeq:de".to_string(),
            strategy: "lshade".to_string(),
            num_filters: 3,
            max_iter: 1000,
            population: 10,
            seed: Some(42),
            tolerance: 1e-3,
            atolerance: 1e-3,
            ..OptimizerConfig::default()
        };

        let result = optimize_channel_eq(&curve, &config, None, 48000.0);
        assert!(
            result.is_ok(),
            "Optimization should succeed with Harman target"
        );

        let (_, loss) = result.unwrap();
        assert!(loss.is_finite(), "Loss should be finite, got {}", loss);
        assert!(loss >= 0.0, "Loss should be non-negative");
    }

    /// Regression test: Harman target curve at reference frequency should be ~0 dB
    #[test]
    fn test_harman_target_reference_frequency() {
        let freqs: Vec<f64> = (0..100)
            .map(|i| 20.0 * (1000.0 / 20.0_f64).powf(i as f64 / 99.0))
            .collect();
        let curve = harman_curve(&freqs, 0.0);

        let idx_ref = freqs
            .iter()
            .position(|f| (f - 1000.0).abs() < freqs[1] - freqs[0])
            .unwrap_or(freqs.len() / 2);

        assert!(
            curve.spl[idx_ref].abs() < 0.1,
            "At 1kHz reference, target should be ~0 dB, got {:.4}",
            curve.spl[idx_ref]
        );
    }

    /// Regression test: Harman target with bass boost adds bass below shelf freq
    #[test]
    fn test_harman_target_with_bass_boost() {
        let freqs: Vec<f64> = (0..100)
            .map(|i| 20.0 * (1000.0 / 20.0_f64).powf(i as f64 / 99.0))
            .collect();
        let curve = harman_curve(&freqs, 6.0);

        let freq_step = freqs[1] - freqs[0];

        let idx_bass = freqs
            .iter()
            .position(|f| (f - 100.0).abs() < freq_step * 2.0)
            .unwrap_or(5);
        assert!(
            curve.spl[idx_bass] > 4.0,
            "At 100Hz with +6dB bass boost, should have >4dB boost, got {:.2}",
            curve.spl[idx_bass]
        );

        let idx_ref = freqs
            .iter()
            .position(|f| (f - 1000.0).abs() < freq_step * 2.0)
            .unwrap_or(freqs.len() / 2);
        assert!(
            curve.spl[idx_ref].abs() < 0.5,
            "At 1kHz reference, should be ~0 dB, got {:.4}",
            curve.spl[idx_ref]
        );
    }

    /// Regression test: Harman target has downward tilt at high frequencies
    #[test]
    fn test_harman_target_high_frequency_tilt() {
        let freqs: Vec<f64> = (0..100)
            .map(|i| 20.0 * (1000.0 / 20.0_f64).powf(i as f64 / 99.0))
            .collect();
        let curve = harman_curve(&freqs, 0.0);

        let freq_step = freqs[1] - freqs[0];

        let idx_low = freqs
            .iter()
            .position(|f| (f - 200.0).abs() < freq_step * 2.0)
            .unwrap_or(10);
        let idx_high = freqs.len() - 1;

        assert!(
            curve.spl[idx_high] < curve.spl[idx_low] - 1.0,
            "High freq should be significantly below low freq (tilt), got low={:.2}, high={:.2}",
            curve.spl[idx_low],
            curve.spl[idx_high]
        );
    }
}

/// Load a measured impulse response WAV file, run SSIR analysis, and
/// return the analysis result alongside the mono IR and its sample
/// rate so downstream callers can also compute RT60 directly from
/// the decoded samples without re-reading the file from disk.
///
/// Returns `None` when the file can't be opened, the buffer is empty,
/// the IR is too short to be useful (< 10 ms), or SSIR didn't detect
/// any events.
fn try_ssir_analysis(
    wav_path: &std::path::Path,
    _sample_rate: f64,
) -> Option<(math_rir::SsirResult, Vec<f32>, f64)> {
    let reader = hound::WavReader::open(wav_path).ok()?;
    let spec = reader.spec();
    let wav_sr = spec.sample_rate;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|v| v as f32 * scale)
                .collect()
        }
    };

    if samples.is_empty() {
        return None;
    }

    // Use first channel for mono analysis (roomeq measurements are typically mono)
    let num_channels = spec.channels as usize;
    let num_frames = samples.len() / num_channels;
    let mono: Vec<f32> = if num_channels == 1 {
        samples
    } else {
        (0..num_frames).map(|i| samples[i * num_channels]).collect()
    };

    // Minimum useful RIR length: 10ms
    let min_samples = (0.010 * wav_sr as f64) as usize;
    if mono.len() < min_samples {
        return None;
    }

    let config = math_rir::SsirConfig::new(wav_sr as f64);
    let result = math_rir::analyze_rir(&mono, &config);

    // Only return if we actually detected something meaningful
    if result.num_events() >= 1 {
        Some((result, mono, wav_sr as f64))
    } else {
        None
    }
}
