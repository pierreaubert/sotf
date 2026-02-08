//! EQ optimization for individual channels

use crate::Curve;
use crate::cli::{Args, PeqModel};
use crate::loss::LossType;
use crate::workflow::setup_objective_data;
use clap::{Parser, ValueEnum};
use math_audio_iir_fir::Biquad;
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
            config.min_freq, config.max_freq, effective_min_freq, effective_max_freq
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
            // Use dummy args to leverage existing target builder logic or re-implement
            // For now, simpler to re-implement common targets or map to Args
            // We can construct minimal Args with curve_name
            let dummy_args = Args::parse_from(["autoeq", "--curve-name", name]);
            crate::workflow::build_target_curve(&dummy_args, &normalized_curve.freq, &normalized_curve)?
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
        refine: config.refine,  // Hybrid optimization: DE + local refinement
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
    let opt_result = crate::optim::optimize_filters(
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
    let peq = crate::x2peq::x2peq(&x, sample_rate, args.peq_model);

    // Extract just the Biquad filters (ignore weights)
    let filters: Vec<Biquad> = peq.into_iter().map(|(_weight, biquad)| biquad).collect();

    eprintln!(
        "  EQ optimization: {} filters, final loss={:.6}",
        filters.len(),
        final_loss
    );

    Ok((filters, final_loss))
}
