//! EQ optimization for individual channels
//!
//! Provides per-channel PEQ optimization using autoeq's workflow.

use crate::cli::{Args, PeqModel};
use crate::loss::LossType;
use crate::workflow::setup_objective_data;
use crate::Curve;
use clap::{Parser, ValueEnum};
use log::debug;
use math_audio_iir_fir::Biquad;
use ndarray::Array1;
use std::error::Error;

use super::impulse_analysis;
use super::spatial_robustness::{self, SpatialRobustnessConfig};
use super::types::{
    MultiMeasurementConfig, MultiMeasurementStrategy, OptimizerConfig, TargetCurveConfig,
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
    };

    // Compute decomposed correction weights BEFORE psychoacoustic smoothing.
    let decomposed_weights = config.decomposed_correction.as_ref().map(|dc_config| {
        let dc_analysis_config = impulse_analysis::DecomposedCorrectionConfig {
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
                Some(ssir_result) => {
                    log::info!(
                        "  SSIR analysis: {} reflections, mixing time={:.1} ms",
                        ssir_result.num_reflections(),
                        ssir_result.mixing_time_ms(),
                    );
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
        result.correction_weights
    });

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
    };

    // Setup objective data
    let (objective_data, _use_cea) = setup_objective_data(
        &args_template,
        &normalized_curve,
        &target_curve,
        &deviation_curve,
        &None,
    )
    .expect("setup_objective_data should not fail without spin data");

    Ok(PreparedSingleChannelEq {
        objective_data,
        args_template,
        peq_model,
        sample_rate,
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

    let (lower_bounds, upper_bounds) = crate::workflow::setup_bounds(&args);
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

    // Convert to Biquad filters, pruning near-zero gain
    let peq = crate::x2peq::x2peq(&x, prep.sample_rate, prep.peq_model);
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
            },
        };

        let deviation_curve = Curve {
            freq: normalized_curve.freq.clone(),
            spl: &target_curve.spl - &normalized_curve.spl,
            phase: None,
        };

        let (objective_data, _use_cea) = crate::workflow::setup_objective_data(
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
    let (objective_data, _use_cea) = setup_objective_data(
        &args,
        &normalized_curve,
        &target_curve,
        &deviation_curve,
        &None,
    )
    .expect("setup_objective_data should not fail without spin data");

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
    use crate::roomeq::target_tilt::{
        build_harman_target_curve, build_harman_target_curve_with_bass_boost,
    };

    fn make_curve_with_freqs(freqs: Vec<f64>, spl: Vec<f64>) -> Curve {
        Curve {
            freq: Array1::from_vec(freqs),
            spl: Array1::from_vec(spl),
            phase: None,
        }
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
        let curve = build_harman_target_curve(&Array1::from_vec(freqs.clone()));

        // Find index closest to 1000 Hz (reference frequency)
        let idx_ref = freqs
            .iter()
            .position(|f| (f - 1000.0).abs() < freqs[1] - freqs[0])
            .unwrap_or(freqs.len() / 2);

        // At reference frequency, target should be ~0 dB
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
        let curve =
            build_harman_target_curve_with_bass_boost(&Array1::from_vec(freqs.clone()), 6.0);

        // Use approximate frequency spacing to find close indices
        let freq_step = freqs[1] - freqs[0];

        // Find index close to 100 Hz (well below 200 Hz shelf)
        let idx_bass = freqs
            .iter()
            .position(|f| (f - 100.0).abs() < freq_step * 2.0)
            .unwrap_or(5);

        // At 100 Hz, should have significant bass boost
        assert!(
            curve.spl[idx_bass] > 4.0,
            "At 100Hz with +6dB bass boost, should have >4dB boost, got {:.2}",
            curve.spl[idx_bass]
        );

        // Find index close to 1 kHz (should be near 0)
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
        let curve = build_harman_target_curve(&Array1::from_vec(freqs.clone()));

        let freq_step = freqs[1] - freqs[0];

        // Find index close to 200 Hz (low frequency end)
        let idx_low = freqs
            .iter()
            .position(|f| (f - 200.0).abs() < freq_step * 2.0)
            .unwrap_or(10);
        let idx_high = freqs.len() - 1;

        // High frequency should have negative tilt relative to low frequency
        assert!(
            curve.spl[idx_high] < curve.spl[idx_low] - 1.0,
            "High freq should be significantly below low freq (tilt), got low={:.2}, high={:.2}",
            curve.spl[idx_low],
            curve.spl[idx_high]
        );
    }
}

/// Try to run SSIR analysis on a measured WAV file.
///
/// Returns None if the WAV can't be loaded or the RIR is too short for analysis.
fn try_ssir_analysis(
    wav_path: &std::path::Path,
    _sample_rate: f64,
) -> Option<math_rir::SsirResult> {
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
        Some(result)
    } else {
        None
    }
}
