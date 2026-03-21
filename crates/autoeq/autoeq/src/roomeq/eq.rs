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

use super::types::{MultiMeasurementConfig, OptimizerConfig, TargetCurveConfig};
use crate::optim::MultiObjectiveData;

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

fn optimize_channel_eq_inner(
    curve: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
    callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
    // Clamp optimizer frequency range to measurement data range.
    // Without this, filters get distributed into regions with no data and produce
    // nonsensical gains (e.g. -70 dB) that don't affect the loss function.
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
    // This is critical for room measurements which may have arbitrary absolute levels
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..curve.freq.len() {
        if curve.freq[i] >= effective_min_freq && curve.freq[i] <= effective_max_freq {
            sum += curve.spl[i];
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
    // This uses variable smoothing: fine resolution at low frequencies (preserve room modes)
    // and coarse resolution at high frequencies (ignore comb filtering)
    if config.psychoacoustic {
        log::info!("  Applying psychoacoustic smoothing (1/48 oct < 100 Hz, 1/6 oct > 1 kHz)");
        let smoothing_config = crate::read::PsychoacousticSmoothingConfig::default();
        normalized_curve = crate::read::smooth_psychoacoustic(&normalized_curve, &smoothing_config);
    }

    // Parse PEQ model
    let peq_model = PeqModel::from_str(&config.peq_model, true)
        .map_err(|e| format!("Invalid PEQ model '{}': {}", config.peq_model, e))?;

    // Create target curve (using normalized curve for consistency)
    let target_curve = match target_config {
        Some(TargetCurveConfig::Path(path)) => {
            // Load target from file
            let target = crate::read::read_curve_from_csv(path)?;
            crate::read::normalize_and_interpolate_response(&normalized_curve.freq, &target)
        }
        Some(TargetCurveConfig::Predefined(name)) => {
            // Generate predefined target
            let dummy_args = Args::parse_from(["autoeq", "--curve-name", name]);
            match crate::workflow::build_target_curve(
                &dummy_args,
                &normalized_curve.freq,
                &normalized_curve,
            ) {
                Ok(curve) => curve,
                Err(_) => {
                    // Fallback: If not a known predefined curve, treat name as a file path
                    debug!(
                        "  Target '{}' not a predefined curve, trying as file path...",
                        name
                    );
                    let target = crate::read::read_curve_from_csv(&std::path::PathBuf::from(name))?;
                    crate::read::normalize_and_interpolate_response(&normalized_curve.freq, &target)
                }
            }
        }
        None => {
            // Default flat target
            Curve {
                freq: normalized_curve.freq.clone(),
                spl: Array1::zeros(normalized_curve.freq.len()),
                phase: None,
            }
        }
    };

    // Parse loss type (with asymmetric option for room correction)
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

    // Create Args structure with optimization parameters
    let args = Args {
        // Number of filters
        num_filters: config.num_filters,

        // Input data (not used since we provide curve directly)
        curve: None,
        target: None,
        speaker: None,
        version: None,
        measurement: None,
        curve_name: "On Axis".to_string(),

        // Sample rate
        sample_rate,

        // Frequency constraints (clamped to measurement data range)
        min_freq: effective_min_freq,
        max_freq: effective_max_freq,

        // Q factor constraints
        min_q: config.min_q,
        max_q: config.max_q,

        // Gain constraints
        min_db: config.min_db,
        max_db: config.max_db,

        // Algorithm
        algo: config.algorithm.clone(),
        strategy: "currenttobest1bin".to_string(),
        algo_list: false,
        strategy_list: false,

        // PEQ model
        peq_model,
        peq_model_list: false,

        // Optimization parameters
        population: config.population,
        maxeval: config.max_iter,
        refine: config.refine, // Hybrid optimization: DE + local refinement
        local_algo: config.local_algo.clone(),

        // Spacing constraints
        min_spacing_oct: 0.2,
        spacing_weight: 20.0,

        // Smoothing
        smooth: true,
        smooth_n: 2,

        // Loss function
        loss: loss_type,

        // Optimization tuning
        tolerance: config.tolerance,
        atolerance: config.atolerance,
        recombination: 0.9,
        adaptive_weight_f: 0.9,
        adaptive_weight_cr: 0.9,
        no_parallel: false,

        // Output (not used)
        output: None,

        // Multi-driver (not used for single channel)
        driver1: None,
        driver2: None,
        driver3: None,
        driver4: None,
        crossover_type: "linkwitzriley4".to_string(),

        // Parallel threads
        parallel_threads: num_cpus::get(),

        // Random seed
        seed: config.seed,

        // QA mode (disabled)
        qa: None,
    };

    // Create deviation curve (target - normalized input measurement)
    // This tells the optimizer what correction is needed at each frequency
    let deviation_curve = Curve {
        freq: normalized_curve.freq.clone(),
        spl: &target_curve.spl - &normalized_curve.spl,
        phase: None,
    };

    // Setup objective data using autoeq's workflow
    let (objective_data, _use_cea) = setup_objective_data(
        &args,
        &normalized_curve,
        &target_curve,
        &deviation_curve,
        &None, // No spin data
    )
    .expect("setup_objective_data should not fail without spin data");

    // Setup bounds
    let (lower_bounds, upper_bounds) = crate::workflow::setup_bounds(&args);

    // Generate initial guess
    let mut x = crate::workflow::initial_guess(&args, &lower_bounds, &upper_bounds);

    // Perform optimization
    let opt_result = if let Some(cb) = callback {
        crate::optim::optimize_filters_with_callback(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            objective_data.clone(),
            &args,
            cb,
        )
    } else {
        crate::optim::optimize_filters(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            objective_data.clone(),
            &args,
        )
    };

    // Handle result - optimizer returns Result<(String, f64), (String, f64)>
    let (_converged_msg, final_loss) = match opt_result {
        Ok((msg, loss)) => (msg, loss),
        Err((msg, loss)) => {
            eprintln!("  Warning: optimization did not fully converge: {}", msg);
            (msg, loss)
        }
    };

    // Convert params to Biquad filters using autoeq's x2peq
    // x2peq returns Vec<(f64, Biquad)> where f64 is the weight
    let peq = crate::x2peq::x2peq(&x, sample_rate, args.peq_model);

    // Extract just the Biquad filters (ignore weights), pruning near-zero gain filters
    let filters: Vec<Biquad> = peq
        .into_iter()
        .map(|(_weight, biquad)| biquad)
        .filter(|b| b.db_gain.abs() >= 0.05)
        .collect();

    log::info!(
        "EQ optimization: {} filters, final loss={:.6}",
        filters.len(),
        final_loss
    );

    Ok((filters, final_loss))
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
    optimize_channel_eq_multi_inner(curves, config, multi_config, target_config, sample_rate, None)
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

    // Run optimization
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

    let (_converged_msg, final_loss) = match opt_result {
        Ok((msg, loss)) => (msg, loss),
        Err((msg, loss)) => {
            eprintln!(
                "  Warning: multi-measurement optimization did not fully converge: {}",
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
        strategy: "currenttobest1bin".to_string(),
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
        smooth_n: 2,
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
