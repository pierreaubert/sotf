//! Headphone EQ optimization
//!
//! Provides the optimization logic and result types for headphone equalization.
//! This module contains the business logic that can be used by any frontend (GPUI, TUI, etc.)

use super::params::OptimizationParams;
use std::path::PathBuf;

/// Bundled target curve data
pub mod target_curves {
    pub const HARMAN_OVER_EAR_2018: &str =
        include_str!("../../../data_tests/targets/harman-over-ear-2018.csv");
    pub const HARMAN_OVER_EAR_2015: &str =
        include_str!("../../../data_tests/targets/harman-over-ear-2015.csv");
    pub const HARMAN_OVER_EAR_2013: &str =
        include_str!("../../../data_tests/targets/harman-over-ear-2013.csv");
    pub const HARMAN_IN_EAR_2019: &str =
        include_str!("../../../data_tests/targets/harman-in-ear-2019.csv");
}

/// Result of headphone EQ optimization with all curves for visualization
#[derive(Clone, Debug)]
pub struct HeadphoneOptimizationResult {
    /// Optimized biquad filters
    pub biquads: Vec<autoeq_iir::Biquad>,
    /// Frequency points (Hz) - log-spaced
    pub frequencies: Vec<f64>,
    /// Input headphone measurement curve (dB)
    pub input_curve: Vec<f64>,
    /// Target curve (dB)
    pub target_curve: Vec<f64>,
    /// Deviation from target = target - input (dB)
    pub deviation_curve: Vec<f64>,
    /// Combined filter response (dB)
    pub filter_response: Vec<f64>,
    /// Error = deviation - filter_response (dB)
    pub error_curve: Vec<f64>,
    /// Corrected response = input + filter_response (dB)
    pub corrected_curve: Vec<f64>,
    /// Individual filter responses (each filter's dB response)
    pub individual_filter_responses: Vec<Vec<f64>>,
    /// Path where results were saved
    pub output_path: String,
    /// Optimization history (iteration, loss)
    pub optimization_history: Vec<(usize, f64)>,
    /// Initial loss value
    pub initial_loss: f64,
    /// Final loss value
    pub final_loss: f64,
}

/// Run headphone EQ optimization
///
/// # Arguments
/// * `curve_path` - Path to the headphone measurement CSV file
/// * `target` - Target curve identifier (e.g., "harman-over-ear-2018", "custom")
/// * `target_custom_path` - Path to custom target curve (only used if target is "custom")
/// * `params` - Optimization parameters
/// * `export_format` - Export format for the resulting EQ file
///
/// # Returns
/// The optimization result with all curves for visualization
pub fn run_headphone_optimization(
    curve_path: &str,
    target: &str,
    target_custom_path: &str,
    params: &OptimizationParams,
    _export_format: &str,
) -> Result<HeadphoneOptimizationResult, String> {
    use std::sync::{Arc, Mutex};

    // Load headphone curve from CSV file
    let input_curve = autoeq::read_curve_from_csv(&PathBuf::from(curve_path))
        .map_err(|e| format!("Failed to read curve file: {}", e))?;

    // Load target curve
    let target_curve = load_target_curve(target, target_custom_path)?;

    // Create standard frequency grid (200 points, 20 Hz to 20 kHz)
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);

    // Normalize and interpolate curves
    let input_curve_norm =
        autoeq::normalize_and_interpolate_response(&standard_freq, &input_curve);
    let target_curve_norm =
        autoeq::normalize_and_interpolate_response(&standard_freq, &target_curve);

    // Create deviation curve
    let deviation_curve = autoeq::Curve {
        freq: target_curve_norm.freq.clone(),
        spl: &target_curve_norm.spl - &input_curve_norm.spl,
        phase: None,
    };

    // Setup optimization arguments from params
    let args = autoeq::Args {
        num_filters: params.num_filters,
        sample_rate: params.sample_rate as f64,
        loss: if params.loss == "headphone-flat" {
            autoeq::LossType::HeadphoneFlat
        } else {
            autoeq::LossType::HeadphoneScore
        },
        algo: params.algo.clone(),
        population: params.population,
        maxeval: params.maxeval,
        strategy: params.strategy.clone(),
        min_db: params.min_db,
        max_db: params.max_db,
        min_q: params.min_q,
        max_q: params.max_q,
        min_freq: params.min_freq,
        max_freq: params.max_freq,
        min_spacing_oct: params.min_spacing_oct,
        spacing_weight: params.spacing_weight,
        smooth: params.smooth,
        smooth_n: params.smooth_n,
        refine: params.refine,
        local_algo: params.local_algo.clone(),
        tolerance: params.tolerance,
        atolerance: params.abs_tolerance,
        recombination: params.de_cr,
        adaptive_weight_f: params.adaptive_weight_f,
        adaptive_weight_cr: params.adaptive_weight_cr,
        peq_model: match params.peq_model.as_str() {
            "hp-pk" => autoeq::cli::PeqModel::HpPk,
            "hp-pk-lp" => autoeq::cli::PeqModel::HpPkLp,
            "ls-pk" => autoeq::cli::PeqModel::LsPk,
            "ls-pk-hs" => autoeq::cli::PeqModel::LsPkHs,
            "free-pk-free" => autoeq::cli::PeqModel::FreePkFree,
            "free" => autoeq::cli::PeqModel::Free,
            _ => autoeq::cli::PeqModel::Pk,
        },
        curve: None,
        target: None,
        output: None,
        speaker: None,
        version: None,
        measurement: None,
        curve_name: params.curve_name.clone(),
        peq_model_list: false,
        algo_list: false,
        strategy_list: false,
        no_parallel: false,
        parallel_threads: 0,
        seed: None,
        qa: None,
        driver1: None,
        driver2: None,
        driver3: None,
        driver4: None,
        crossover_type: "linkwitzriley4".to_string(),
    };

    // Setup objective data
    let (objective_data, _use_cea) = autoeq::workflow::setup_objective_data(
        &args,
        &input_curve_norm,
        &target_curve_norm,
        &deviation_curve,
        &None,
    );

    // Run optimization with history tracking
    let history: Vec<(usize, f64)> = Vec::new();
    let history_ptr = Arc::new(Mutex::new(history));
    let history_callback = history_ptr.clone();

    let filter_params = autoeq::workflow::perform_optimization_with_callback(
        &args,
        &objective_data,
        Box::new(move |intermediate| {
            if let Ok(mut h) = history_callback.lock() {
                h.push((intermediate.iter, intermediate.fun));
            }
            autoeq::de::CallbackAction::Continue
        }),
    )
    .map_err(|e| format!("Optimization failed: {}", e))?;

    // Retrieve history
    let history = history_ptr
        .lock()
        .map_err(|_| "Failed to lock history")?
        .clone();
    let initial_loss = history.first().map(|x| x.1).unwrap_or(0.0);
    let final_loss = history.last().map(|x| x.1).unwrap_or(0.0);

    // Convert to biquad filters (x2peq returns Vec<(f64, Biquad)>)
    let peq = autoeq::x2peq::x2peq(&filter_params, args.sample_rate, args.peq_model);
    // Extract just the biquads from the (frequency, biquad) tuples
    let biquads: Vec<autoeq_iir::Biquad> = peq.into_iter().map(|(_, b)| b).collect();

    // Calculate all the curves for visualization
    let frequencies: Vec<f64> = standard_freq.iter().copied().collect();
    let input_curve_vec: Vec<f64> = input_curve_norm.spl.iter().copied().collect();
    let target_curve_vec: Vec<f64> = target_curve_norm.spl.iter().copied().collect();
    let deviation_curve_vec: Vec<f64> = deviation_curve.spl.iter().copied().collect();

    // Calculate combined filter response
    let filter_response: Vec<f64> = frequencies
        .iter()
        .map(|&freq| biquads.iter().map(|b| b.log_result(freq)).sum())
        .collect();

    // Calculate individual filter responses
    let individual_filter_responses: Vec<Vec<f64>> = biquads
        .iter()
        .map(|biquad| {
            frequencies
                .iter()
                .map(|&freq| biquad.log_result(freq))
                .collect()
        })
        .collect();

    // Error = deviation - filter_response (what we still need to correct)
    let error_curve: Vec<f64> = deviation_curve_vec
        .iter()
        .zip(filter_response.iter())
        .map(|(d, f)| d - f)
        .collect();

    // Corrected response = input + filter_response
    let corrected_curve: Vec<f64> = input_curve_vec
        .iter()
        .zip(filter_response.iter())
        .map(|(i, f)| i + f)
        .collect();

    Ok(HeadphoneOptimizationResult {
        biquads,
        frequencies,
        input_curve: input_curve_vec,
        target_curve: target_curve_vec,
        deviation_curve: deviation_curve_vec,
        filter_response,
        error_curve,
        corrected_curve,
        individual_filter_responses,
        output_path: String::new(), // Caller should save and set this
        optimization_history: history,
        initial_loss,
        final_loss,
    })
}

/// Load target curve from bundled data or custom file
pub fn load_target_curve(target: &str, custom_path: &str) -> Result<autoeq::Curve, String> {
    match target {
        "harman-over-ear-2018" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2018),
        "harman-over-ear-2015" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2015),
        "harman-over-ear-2013" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2013),
        "harman-in-ear-2019" => parse_csv_curve(target_curves::HARMAN_IN_EAR_2019),
        "custom" => {
            // Load from custom file path
            autoeq::read_curve_from_csv(&PathBuf::from(custom_path))
                .map_err(|e| format!("Failed to read custom target curve: {}", e))
        }
        _ => {
            // Load from custom file path
            autoeq::read_curve_from_csv(&PathBuf::from(custom_path))
                .map_err(|e| format!("A target curve is required for headphone: {}", e))
        }
    }
}

/// Parse a CSV string into a Curve
pub fn parse_csv_curve(csv_data: &str) -> Result<autoeq::Curve, String> {
    use ndarray::Array1;

    let mut freq = Vec::new();
    let mut spl = Vec::new();

    for (i, line) in csv_data.lines().enumerate() {
        // Skip header line
        if i == 0 && line.contains("frequency") {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            if let (Ok(f), Ok(s)) = (
                parts[0].trim().parse::<f64>(),
                parts[1].trim().parse::<f64>(),
            ) {
                freq.push(f);
                spl.push(s);
            }
        }
    }

    if freq.is_empty() {
        return Err("No valid data found in CSV".to_string());
    }

    Ok(autoeq::Curve {
        freq: Array1::from(freq),
        spl: Array1::from(spl),
        phase: None,
    })
}
