//! EQ optimization for individual channels

use autoeq::Curve;
use autoeq::cli::{Args, PeqModel};
use autoeq::loss::LossType;
use autoeq::workflow::setup_objective_data;
use autoeq_iir::Biquad;
use clap::{Parser, ValueEnum};
use ndarray::Array1;
use std::error::Error;

use super::types::{OptimizerConfig, TargetCurveConfig};

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
    // Parse PEQ model
    let peq_model = PeqModel::from_str(&config.peq_model, true)
        .map_err(|e| format!("Invalid PEQ model '{}': {}", config.peq_model, e))?;

    // Create target curve
    let target_curve = match target_config {
        Some(TargetCurveConfig::Path(path)) => {
            // Load target from file
            let target = autoeq::read::read_curve_from_csv(path)?;
            autoeq::read::normalize_and_interpolate_response(&curve.freq, &target)
        }
        Some(TargetCurveConfig::Predefined(name)) => {
            // Generate predefined target
            // Use dummy args to leverage existing target builder logic or re-implement
            // For now, simpler to re-implement common targets or map to Args
            // We can construct minimal Args with curve_name
            let dummy_args = Args::parse_from(["autoeq", "--curve-name", name]);
            autoeq::workflow::build_target_curve(&dummy_args, &curve.freq, curve)?
        }
        None => {
            // Default flat target
            Curve {
                freq: curve.freq.clone(),
                spl: Array1::zeros(curve.freq.len()),
                phase: None,
            }
        }
    };

    // Parse loss type
    let loss_type = match config.loss_type.as_str() {
        "flat" => LossType::SpeakerFlat,
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

        // Frequency constraints
        min_freq: config.min_freq,
        max_freq: config.max_freq,

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
        population: 300,
        maxeval: config.max_iter,
        refine: false,
        local_algo: "cobyla".to_string(),

        // Spacing constraints
        min_spacing_oct: 0.2,
        spacing_weight: 20.0,

        // Smoothing
        smooth: true,
        smooth_n: 2,

        // Loss function
        loss: loss_type,

        // Optimization tuning
        tolerance: 1e-3,
        atolerance: 1e-4,
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
        seed: None,

        // QA mode (disabled)
        qa: None,
    };

    // Create deviation curve (flat with zeros)
    let deviation_curve = Curve {
        freq: curve.freq.clone(),
        spl: Array1::zeros(curve.freq.len()),
        phase: None,
    };

    // Setup objective data using autoeq's workflow
    let (objective_data, _use_cea) = setup_objective_data(
        &args,
        curve,
        &target_curve,
        &deviation_curve,
        &None, // No spin data
    )
    .expect("setup_objective_data should not fail without spin data");

    // Setup bounds
    let (lower_bounds, upper_bounds) = autoeq::workflow::setup_bounds(&args);

    // Generate initial guess
    let mut x = autoeq::workflow::initial_guess(&args, &lower_bounds, &upper_bounds);

    // Perform optimization
    let opt_result = autoeq::optim::optimize_filters(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &args,
    );

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
    let peq = autoeq::x2peq::x2peq(&x, sample_rate, args.peq_model);

    // Extract just the Biquad filters (ignore weights)
    let filters: Vec<Biquad> = peq.into_iter().map(|(_weight, biquad)| biquad).collect();

    eprintln!(
        "  EQ optimization: {} filters, final loss={:.6}",
        filters.len(),
        final_loss
    );

    Ok((filters, final_loss))
}
