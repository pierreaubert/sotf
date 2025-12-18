//! Speaker EQ optimization
//!
//! Provides the optimization logic and result types for speaker equalization.
//! This module contains the business logic that can be used by any frontend (GPUI, TUI, etc.)
//!
//! Supports:
//! - CSV and Spinorama API data sources
//! - Single-driver, multi-driver (crossover), multi-sub, and DBA configurations
//! - Real-time progress callbacks with configurable intervals

use super::params::OptimizationParams;
use super::types::{CrossoverType, SpeakerConfigType};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// Re-export CallbackAction for user convenience
pub use autoeq::de::CallbackAction;

// ============================================================================
// Progress and Callback Types
// ============================================================================

/// Data passed to the optimization callback at each interval
#[derive(Debug, Clone)]
pub struct SpeakerOptimizationProgress {
    /// Current iteration number
    pub iteration: usize,
    /// Current loss/objective value
    pub loss: f64,
    /// Convergence metric (population standard deviation)
    pub convergence: f64,
    /// Current best parameters (raw optimizer params)
    pub current_params: Vec<f64>,
    /// Current best biquad filters (decoded from params)
    pub current_biquads: Vec<autoeq_iir::Biquad>,
    /// Current filter response curve (dB)
    pub current_filter_response: Vec<f64>,
    /// Stage of optimization
    pub stage: OptimizationStage,
    /// Total iterations expected (maxeval)
    pub max_iterations: usize,
}

/// Stage of the optimization process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptimizationStage {
    /// Optimizing crossover frequencies and driver gains (multi-driver only)
    Crossover,
    /// Optimizing EQ filters
    #[default]
    Eq,
    /// Local refinement phase
    Refinement,
}

/// Callback function type for speaker optimization
pub type SpeakerOptimizationCallback =
    Box<dyn FnMut(&SpeakerOptimizationProgress) -> CallbackAction + Send>;

/// Configuration for the callback
#[derive(Debug, Clone)]
pub struct CallbackConfig {
    /// Report every N iterations (e.g., 10 or 25)
    pub interval: usize,
    /// Whether to include decoded biquads in each callback (more expensive)
    pub include_biquads: bool,
    /// Whether to include filter response curve (more expensive)
    pub include_filter_response: bool,
}

impl Default for CallbackConfig {
    fn default() -> Self {
        Self {
            interval: 25,
            include_biquads: true,
            include_filter_response: true,
        }
    }
}

// ============================================================================
// Input Configuration Types
// ============================================================================

/// Source of measurement data
#[derive(Debug, Clone)]
pub enum MeasurementInput {
    /// CSV file path
    CsvFile(PathBuf),
    /// Spinorama API reference
    Spinorama {
        speaker: String,
        version: String,
        measurement: String,
        curve_name: String,
    },
    /// Pre-loaded curve data
    Curve(autoeq::Curve),
}

/// Configuration for speaker optimization
#[derive(Debug, Clone)]
pub struct SpeakerOptimizationConfig {
    /// Speaker configuration type
    pub config_type: SpeakerConfigType,
    /// Main measurement (for single-driver)
    pub main_measurement: Option<MeasurementInput>,
    /// Driver measurements (for multi-driver, ordered low to high frequency)
    pub driver_measurements: Vec<MeasurementInput>,
    /// Crossover type (for multi-driver)
    pub crossover_type: Option<CrossoverType>,
    /// Initial crossover frequency hints (optional)
    pub crossover_freq_hints: Vec<f64>,
    /// Optimization parameters
    pub params: OptimizationParams,
    /// Callback configuration
    pub callback_config: Option<CallbackConfig>,
    /// Target curve (optional - defaults to flat or curve-name-specific)
    pub target: Option<MeasurementInput>,
}

impl Default for SpeakerOptimizationConfig {
    fn default() -> Self {
        Self {
            config_type: SpeakerConfigType::Single,
            main_measurement: None,
            driver_measurements: Vec::new(),
            crossover_type: None,
            crossover_freq_hints: Vec::new(),
            params: OptimizationParams::speaker_defaults(),
            callback_config: Some(CallbackConfig::default()),
            target: None,
        }
    }
}

/// Extended speaker config types including multi-sub and DBA
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeakerConfigTypeExt {
    /// Single measurement (simple speaker)
    #[default]
    Single,
    /// Multiple drivers with crossover
    MultiDriver,
    /// Multiple subwoofers (gain + delay optimization)
    MultiSub,
    /// Double Bass Array
    Dba,
}

/// Extended configuration for speaker optimization including multi-sub and DBA
#[derive(Debug, Clone)]
pub struct SpeakerOptimizationConfigExt {
    /// Speaker configuration type
    pub config_type: SpeakerConfigTypeExt,
    /// Main measurement (for single-driver)
    pub main_measurement: Option<MeasurementInput>,
    /// Driver measurements (for multi-driver/multi-sub, ordered low to high frequency)
    pub driver_measurements: Vec<MeasurementInput>,
    /// Front array measurements (for DBA)
    pub front_measurements: Vec<MeasurementInput>,
    /// Rear array measurements (for DBA)
    pub rear_measurements: Vec<MeasurementInput>,
    /// Crossover type (for multi-driver)
    pub crossover_type: Option<CrossoverType>,
    /// Initial crossover frequency hints (optional)
    pub crossover_freq_hints: Vec<f64>,
    /// Optimization parameters
    pub params: OptimizationParams,
    /// Callback configuration
    pub callback_config: Option<CallbackConfig>,
    /// Target curve (optional)
    pub target: Option<MeasurementInput>,
}

impl Default for SpeakerOptimizationConfigExt {
    fn default() -> Self {
        Self {
            config_type: SpeakerConfigTypeExt::Single,
            main_measurement: None,
            driver_measurements: Vec::new(),
            front_measurements: Vec::new(),
            rear_measurements: Vec::new(),
            crossover_type: None,
            crossover_freq_hints: Vec::new(),
            params: OptimizationParams::speaker_defaults(),
            callback_config: Some(CallbackConfig::default()),
            target: None,
        }
    }
}

// ============================================================================
// Result Types
// ============================================================================

/// Result of a speaker optimization run
#[derive(Clone, Debug)]
pub struct SpeakerOptimizationResult {
    pub biquads: Vec<autoeq_iir::Biquad>,
    pub frequencies: Vec<f64>,
    pub input_curve: Vec<f64>,     // On-axis or listening window
    pub target_curve: Vec<f64>,    // Calculated target
    pub deviation_curve: Vec<f64>, // Input - Target
    pub filter_response: Vec<f64>, // Sum of biquads
    pub error_curve: Vec<f64>,     // Deviation + Filter
    pub corrected_curve: Vec<f64>, // Input + Filter
    pub individual_filter_responses: Vec<Vec<f64>>,
    pub output_path: String,

    // Spinorama specific curves
    pub er_curve: Vec<f64>,    // Early Reflections
    pub sp_curve: Vec<f64>,    // Sound Power
    pub er_di_curve: Vec<f64>, // Early Reflections Directivity Index
    pub sp_di_curve: Vec<f64>, // Sound Power Directivity Index

    pub optimization_history: Vec<(usize, f64)>,
    pub initial_loss: f64,
    pub final_loss: f64,

    // Multi-driver results (optional)
    pub crossover_freqs: Option<Vec<f64>>,
    pub driver_gains: Option<Vec<f64>>,
    pub driver_delays: Option<Vec<f64>>,
}

/// Internal result from multi-driver optimization
#[derive(Debug, Clone)]
struct MultiDriverResult {
    gains: Vec<f64>,
    delays: Vec<f64>,
    crossover_freqs: Vec<f64>,
    combined_curve: autoeq::Curve,
    biquads: Vec<autoeq_iir::Biquad>,
    history: Vec<(usize, f64)>,
    pre_score: f64,
    post_score: f64,
}

/// Internal result for curves computation
struct ResultCurves {
    frequencies: Vec<f64>,
    input_curve: Vec<f64>,
    target_curve: Vec<f64>,
    deviation_curve: Vec<f64>,
    filter_response: Vec<f64>,
    error_curve: Vec<f64>,
    corrected_curve: Vec<f64>,
    individual_filter_responses: Vec<Vec<f64>>,
    er_curve: Vec<f64>,
    sp_curve: Vec<f64>,
    er_di_curve: Vec<f64>,
    sp_di_curve: Vec<f64>,
}

// ============================================================================
// Data Loading Functions
// ============================================================================

/// Load measurement from any supported source
fn load_measurement(input: &MeasurementInput) -> Result<autoeq::Curve, String> {
    match input {
        MeasurementInput::CsvFile(path) => load_csv_measurement(path),
        MeasurementInput::Spinorama {
            speaker,
            version,
            measurement,
            curve_name,
        } => {
            let (curve, _) = load_spinorama_measurement(speaker, version, measurement, curve_name)?;
            Ok(curve)
        }
        MeasurementInput::Curve(curve) => Ok(curve.clone()),
    }
}

/// Load measurement from any supported source, including spin data
fn load_measurement_with_spin(
    input: &MeasurementInput,
) -> Result<(autoeq::Curve, Option<HashMap<String, autoeq::Curve>>), String> {
    match input {
        MeasurementInput::CsvFile(path) => {
            let curve = load_csv_measurement(path)?;
            Ok((curve, None))
        }
        MeasurementInput::Spinorama {
            speaker,
            version,
            measurement,
            curve_name,
        } => load_spinorama_measurement(speaker, version, measurement, curve_name),
        MeasurementInput::Curve(curve) => Ok((curve.clone(), None)),
    }
}

/// Load measurement from CSV file
fn load_csv_measurement(path: &std::path::Path) -> Result<autoeq::Curve, String> {
    autoeq::read::read_curve_from_csv(&path.to_path_buf())
        .map_err(|e| format!("Failed to read CSV: {}", e))
}

/// Load measurement from Spinorama API
fn load_spinorama_measurement(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<(autoeq::Curve, Option<HashMap<String, autoeq::Curve>>), String> {
    // Create a new runtime for blocking API call
    let rt =
        tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;

    rt.block_on(async {
        // Handle Estimated In-Room Response specially - it's computed from CEA2034 curves
        // This can be requested either as measurement="Estimated In-Room Response" or
        // as measurement="CEA2034" with curve_name="Estimated In-Room Response"
        if measurement == "Estimated In-Room Response"
            || (measurement == "CEA2034" && curve_name == "Estimated In-Room Response")
        {
            let plot_data = autoeq::read::fetch_measurement_plot_data(speaker, version, "CEA2034")
                .await
                .map_err(|e| format!("API error: {}", e))?;

            let curves = autoeq::read::extract_cea2034_curves_original(&plot_data, "CEA2034")
                .map_err(|e| format!("Spin data error: {}", e))?;

            let pir_curve = curves
                .get("Estimated In-Room Response")
                .ok_or("PIR curve not found in CEA2034 data")?
                .clone();

            Ok((pir_curve, Some(curves)))
        } else {
            let curve = autoeq::read::read_spinorama(speaker, version, measurement, curve_name)
                .await
                .map_err(|e| format!("API error: {}", e))?;

            // Extract spin data if CEA2034 (still need the full plot data for this)
            let spin_data = if measurement == "CEA2034" {
                let plot_data =
                    autoeq::read::fetch_measurement_plot_data(speaker, version, measurement)
                        .await
                        .map_err(|e| format!("API error: {}", e))?;
                Some(
                    autoeq::read::extract_cea2034_curves_original(&plot_data, "CEA2034")
                        .map_err(|e| format!("Spin data error: {}", e))?,
                )
            } else {
                None
            };

            Ok((curve, spin_data))
        }
    })
}

// ============================================================================
// Callback Infrastructure
// ============================================================================

/// Create an interval-based callback wrapper that converts DE callback to user callback
fn create_interval_callback(
    mut user_callback: SpeakerOptimizationCallback,
    interval: usize,
    stage: OptimizationStage,
    max_iterations: usize,
    sample_rate: f64,
    peq_model: autoeq::cli::PeqModel,
    include_biquads: bool,
    include_filter_response: bool,
    frequencies: Vec<f64>,
) -> Box<dyn FnMut(&autoeq::de::DEIntermediate) -> CallbackAction + Send> {
    let mut last_reported_iter = 0usize;

    Box::new(
        move |intermediate: &autoeq::de::DEIntermediate| -> CallbackAction {
            // Check if we should report
            if intermediate.iter == 0
                || intermediate.iter.saturating_sub(last_reported_iter) >= interval
            {
                last_reported_iter = intermediate.iter;

                // Decode current params to biquads if requested
                let current_biquads = if include_biquads {
                    decode_params_to_biquads(&intermediate.x.to_vec(), sample_rate, peq_model)
                } else {
                    Vec::new()
                };

                // Compute filter response if requested
                let current_filter_response =
                    if include_filter_response && !current_biquads.is_empty() {
                        compute_filter_response(&frequencies, &current_biquads)
                    } else {
                        Vec::new()
                    };

                let progress = SpeakerOptimizationProgress {
                    iteration: intermediate.iter,
                    loss: intermediate.fun,
                    convergence: intermediate.convergence,
                    current_params: intermediate.x.to_vec(),
                    current_biquads,
                    current_filter_response,
                    stage,
                    max_iterations,
                };

                user_callback(&progress)
            } else {
                CallbackAction::Continue
            }
        },
    )
}

/// Decode optimizer parameters to biquad filters
fn decode_params_to_biquads(
    params: &[f64],
    sample_rate: f64,
    peq_model: autoeq::cli::PeqModel,
) -> Vec<autoeq_iir::Biquad> {
    let peq = autoeq::x2peq::x2peq(params, sample_rate, peq_model);
    peq.into_iter().map(|(_, b)| b).collect()
}

/// Compute filter response at given frequencies
fn compute_filter_response(frequencies: &[f64], biquads: &[autoeq_iir::Biquad]) -> Vec<f64> {
    frequencies
        .iter()
        .map(|&freq| biquads.iter().map(|b| b.log_result(freq)).sum())
        .collect()
}

// ============================================================================
// Args Builder
// ============================================================================

/// Build autoeq::Args from OptimizationParams
fn build_autoeq_args(params: &OptimizationParams) -> autoeq::Args {
    autoeq::Args {
        num_filters: params.num_filters,
        sample_rate: params.sample_rate as f64,
        loss: match params.loss.as_str() {
            "speaker-flat" => autoeq::LossType::SpeakerFlat,
            "speaker-score" => autoeq::LossType::SpeakerScore,
            "headphone-flat" => autoeq::LossType::HeadphoneFlat,
            "headphone-score" => autoeq::LossType::HeadphoneScore,
            _ => autoeq::LossType::SpeakerFlat,
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
    }
}

// ============================================================================
// Single-Driver Optimization
// ============================================================================

/// Optimize a single-driver speaker
fn optimize_single_driver(
    curve: &autoeq::Curve,
    target: &autoeq::Curve,
    spin_data: &Option<HashMap<String, autoeq::Curve>>,
    params: &OptimizationParams,
    callback_config: &Option<CallbackConfig>,
    mut callback: Option<SpeakerOptimizationCallback>,
) -> Result<(Vec<autoeq_iir::Biquad>, Vec<(usize, f64)>, f64, f64), String> {
    // Build autoeq::Args from OptimizationParams
    let args = build_autoeq_args(params);

    // Create deviation curve (target - input)
    let deviation_curve = autoeq::Curve {
        freq: target.freq.clone(),
        spl: &target.spl - &curve.spl,
        phase: None,
    };

    // Setup objective data
    let (objective_data, _use_cea) =
        autoeq::workflow::setup_objective_data(&args, curve, target, &deviation_curve, spin_data)
            .map_err(|e| e.to_string())?;

    // Create callback if configured
    let frequencies: Vec<f64> = curve.freq.iter().copied().collect();
    let history = Arc::new(Mutex::new(Vec::new()));
    let history_ref = history.clone();

    let peq_model = args.peq_model;
    let sample_rate = args.sample_rate;
    let maxeval = args.maxeval;

    let de_callback: Box<dyn FnMut(&autoeq::de::DEIntermediate) -> CallbackAction + Send> =
        if let (Some(cfg), Some(user_cb)) = (callback_config, callback.take()) {
            // Wrap user callback with interval logic
            let mut interval_cb = create_interval_callback(
                user_cb,
                cfg.interval,
                OptimizationStage::Eq,
                maxeval,
                sample_rate,
                peq_model,
                cfg.include_biquads,
                cfg.include_filter_response,
                frequencies.clone(),
            );

            // Combine with history recording
            Box::new(move |intermediate| {
                if let Ok(mut h) = history_ref.lock() {
                    h.push((intermediate.iter, intermediate.fun));
                }
                interval_cb(intermediate)
            })
        } else {
            // Just record history
            Box::new(move |intermediate| {
                if let Ok(mut h) = history_ref.lock() {
                    h.push((intermediate.iter, intermediate.fun));
                }
                CallbackAction::Continue
            })
        };

    // Run optimization
    let filter_params =
        autoeq::workflow::perform_optimization_with_callback(&args, &objective_data, de_callback)
            .map_err(|e| format!("Optimization failed: {}", e))?;

    // Convert to biquads
    let peq = autoeq::x2peq::x2peq(&filter_params, args.sample_rate, args.peq_model);
    let biquads: Vec<autoeq_iir::Biquad> = peq.into_iter().map(|(_, b)| b).collect();

    // Get history
    let history_vec = history
        .lock()
        .map_err(|_| "Failed to lock history")?
        .clone();
    let initial_loss = history_vec.first().map(|x| x.1).unwrap_or(0.0);
    let final_loss = history_vec.last().map(|x| x.1).unwrap_or(0.0);

    Ok((biquads, history_vec, initial_loss, final_loss))
}

// ============================================================================
// Multi-Driver Optimization
// ============================================================================

/// Convert CrossoverType to autoeq's CrossoverType
fn convert_crossover_type(ct: &CrossoverType) -> autoeq::loss::CrossoverType {
    match ct {
        CrossoverType::Butterworth12 => autoeq::loss::CrossoverType::Butterworth2,
        CrossoverType::LR12 => autoeq::loss::CrossoverType::LinkwitzRiley2,
        CrossoverType::LR24 => autoeq::loss::CrossoverType::LinkwitzRiley4,
        CrossoverType::LR48 => autoeq::loss::CrossoverType::LinkwitzRiley4, // LR48 not available, fallback to LR24
    }
}

/// Optimize multi-driver speaker with crossover
fn optimize_multidriver(
    driver_curves: Vec<autoeq::Curve>,
    crossover_type: CrossoverType,
    params: &OptimizationParams,
    callback_config: &Option<CallbackConfig>,
    mut callback: Option<SpeakerOptimizationCallback>,
) -> Result<MultiDriverResult, String> {
    let n_drivers = driver_curves.len();
    if n_drivers < 2 {
        return Err("Multi-driver optimization requires at least 2 drivers".to_string());
    }

    // Create driver measurements
    let driver_measurements: Vec<autoeq::loss::DriverMeasurement> = driver_curves
        .iter()
        .map(|c| {
            autoeq::loss::DriverMeasurement::new(c.freq.clone(), c.spl.clone(), c.phase.clone())
        })
        .collect();

    // Create DriversLossData
    let autoeq_crossover_type = convert_crossover_type(&crossover_type);
    let drivers_data =
        autoeq::loss::DriversLossData::new(driver_measurements, autoeq_crossover_type);

    // Optimize crossover
    let crossover_result = autoeq::workflow::optimize_drivers_crossover(
        drivers_data.clone(),
        params.min_freq,
        params.max_freq,
        params.sample_rate as f64,
        &params.algo,
        params.maxeval,
        params.min_db,
        params.max_db,
    )
    .map_err(|e| format!("Crossover optimization failed: {}", e))?;

    // Compute combined curve using optimized parameters
    let combined_curve = compute_combined_driver_curve(
        &driver_curves,
        &crossover_result.gains,
        &crossover_result.delays,
        &crossover_result.crossover_freqs,
        &crossover_type,
        params.sample_rate as f64,
    );

    // Now optimize EQ on the combined curve
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);
    let combined_normalized =
        autoeq::normalize_and_interpolate_response(&standard_freq, &combined_curve);

    // Build target curve (flat)
    let target = autoeq::Curve {
        freq: combined_normalized.freq.clone(),
        spl: ndarray::Array1::zeros(combined_normalized.freq.len()),
        phase: None,
    };

    let (biquads, history, _initial, final_loss) = optimize_single_driver(
        &combined_normalized,
        &target,
        &None,
        params,
        callback_config,
        callback.take(),
    )?;

    Ok(MultiDriverResult {
        gains: crossover_result.gains,
        delays: crossover_result.delays,
        crossover_freqs: crossover_result.crossover_freqs,
        combined_curve: combined_normalized,
        biquads,
        history,
        pre_score: crossover_result.pre_objective,
        post_score: final_loss,
    })
}

/// Compute combined driver curve with gains, delays, and crossover
fn compute_combined_driver_curve(
    driver_curves: &[autoeq::Curve],
    gains: &[f64],
    _delays: &[f64],
    _crossover_freqs: &[f64],
    _crossover_type: &CrossoverType,
    _sample_rate: f64,
) -> autoeq::Curve {
    // Use the first driver's frequency grid
    let freq = driver_curves[0].freq.clone();
    let n = freq.len();

    // Simple approximation: sum the drivers with gains applied
    // In a full implementation, this would apply crossover filters and delays
    let mut combined_spl = ndarray::Array1::zeros(n);

    for (i, curve) in driver_curves.iter().enumerate() {
        let gain = gains.get(i).copied().unwrap_or(0.0);
        // Interpolate curve to common frequency grid if needed
        if curve.freq.len() == n {
            combined_spl += &(&curve.spl + gain);
        }
    }

    // Average the sum
    combined_spl /= driver_curves.len() as f64;

    autoeq::Curve {
        freq,
        spl: combined_spl,
        phase: None,
    }
}

// ============================================================================
// Multi-Sub Optimization
// ============================================================================

/// Optimize multiple subwoofers (gain + delay)
fn optimize_multisub(
    sub_curves: Vec<autoeq::Curve>,
    params: &OptimizationParams,
    callback_config: &Option<CallbackConfig>,
    mut callback: Option<SpeakerOptimizationCallback>,
) -> Result<MultiDriverResult, String> {
    let n_subs = sub_curves.len();
    if n_subs < 2 {
        return Err("Multi-sub optimization requires at least 2 subwoofers".to_string());
    }

    // Create driver measurements
    let driver_measurements: Vec<autoeq::loss::DriverMeasurement> = sub_curves
        .iter()
        .map(|c| {
            autoeq::loss::DriverMeasurement::new(c.freq.clone(), c.spl.clone(), c.phase.clone())
        })
        .collect();

    // Create DriversLossData (no crossover for multi-sub)
    let drivers_data = autoeq::loss::DriversLossData::new(
        driver_measurements,
        autoeq::loss::CrossoverType::LinkwitzRiley4, // Not used for multi-sub
    );

    // Optimize multi-sub
    let result = autoeq::workflow::optimize_multisub(
        drivers_data,
        params.min_freq,
        params.max_freq.min(500.0), // Multi-sub focuses on low frequencies
        params.sample_rate as f64,
        &params.algo,
        params.maxeval,
        params.min_db,
        params.max_db,
    )
    .map_err(|e| format!("Multi-sub optimization failed: {}", e))?;

    // Compute combined curve
    let combined_curve =
        compute_multisub_combined_curve(&sub_curves, &result.gains, &result.delays);

    // Now optimize EQ on the combined curve
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 500.0);
    let combined_normalized =
        autoeq::normalize_and_interpolate_response(&standard_freq, &combined_curve);

    let target = autoeq::Curve {
        freq: combined_normalized.freq.clone(),
        spl: ndarray::Array1::zeros(combined_normalized.freq.len()),
        phase: None,
    };

    let (biquads, history, _initial, final_loss) = optimize_single_driver(
        &combined_normalized,
        &target,
        &None,
        params,
        callback_config,
        callback.take(),
    )?;

    Ok(MultiDriverResult {
        gains: result.gains,
        delays: result.delays,
        crossover_freqs: vec![], // No crossovers for multi-sub
        combined_curve: combined_normalized,
        biquads,
        history,
        pre_score: result.pre_objective,
        post_score: final_loss,
    })
}

/// Compute combined multi-sub curve
fn compute_multisub_combined_curve(
    sub_curves: &[autoeq::Curve],
    gains: &[f64],
    _delays: &[f64],
) -> autoeq::Curve {
    let freq = sub_curves[0].freq.clone();
    let n = freq.len();
    let mut combined_spl = ndarray::Array1::zeros(n);

    for (i, curve) in sub_curves.iter().enumerate() {
        let gain = gains.get(i).copied().unwrap_or(0.0);
        if curve.freq.len() == n {
            combined_spl += &(&curve.spl + gain);
        }
    }

    combined_spl /= sub_curves.len() as f64;

    autoeq::Curve {
        freq,
        spl: combined_spl,
        phase: None,
    }
}

// ============================================================================
// DBA Optimization
// ============================================================================

/// Optimize Double Bass Array
fn optimize_dba(
    front_curves: Vec<autoeq::Curve>,
    rear_curves: Vec<autoeq::Curve>,
    params: &OptimizationParams,
    callback_config: &Option<CallbackConfig>,
    mut callback: Option<SpeakerOptimizationCallback>,
) -> Result<MultiDriverResult, String> {
    if front_curves.is_empty() || rear_curves.is_empty() {
        return Err("DBA optimization requires both front and rear arrays".to_string());
    }

    // For DBA, combine front and rear arrays separately, then optimize gains and delays
    // This is a simplified implementation

    // Combine front array
    let front_combined = compute_array_combined_curve(&front_curves);

    // Combine rear array
    let rear_combined = compute_array_combined_curve(&rear_curves);

    // Create driver measurements for front and rear
    let driver_measurements = vec![
        autoeq::loss::DriverMeasurement::new(
            front_combined.freq.clone(),
            front_combined.spl.clone(),
            front_combined.phase.clone(),
        ),
        autoeq::loss::DriverMeasurement::new(
            rear_combined.freq.clone(),
            rear_combined.spl.clone(),
            rear_combined.phase.clone(),
        ),
    ];

    // Create DriversLossData
    let drivers_data = autoeq::loss::DriversLossData::new(
        driver_measurements,
        autoeq::loss::CrossoverType::LinkwitzRiley4,
    );

    // Optimize as multi-sub (gains + delays)
    let result = autoeq::workflow::optimize_multisub(
        drivers_data,
        params.min_freq,
        params.max_freq.min(200.0), // DBA focuses on very low frequencies
        params.sample_rate as f64,
        &params.algo,
        params.maxeval,
        params.min_db,
        params.max_db,
    )
    .map_err(|e| format!("DBA optimization failed: {}", e))?;

    // Compute final combined curve
    let combined_curve = compute_dba_combined_curve(
        &front_combined,
        &rear_combined,
        result.gains.first().copied().unwrap_or(0.0),
        result.gains.get(1).copied().unwrap_or(0.0),
        result.delays.get(1).copied().unwrap_or(0.0),
    );

    // Optimize EQ on combined curve
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 200.0);
    let combined_normalized =
        autoeq::normalize_and_interpolate_response(&standard_freq, &combined_curve);

    let target = autoeq::Curve {
        freq: combined_normalized.freq.clone(),
        spl: ndarray::Array1::zeros(combined_normalized.freq.len()),
        phase: None,
    };

    let (biquads, history, _initial, final_loss) = optimize_single_driver(
        &combined_normalized,
        &target,
        &None,
        params,
        callback_config,
        callback.take(),
    )?;

    Ok(MultiDriverResult {
        gains: result.gains,
        delays: result.delays,
        crossover_freqs: vec![],
        combined_curve: combined_normalized,
        biquads,
        history,
        pre_score: result.pre_objective,
        post_score: final_loss,
    })
}

/// Compute combined curve from an array of speakers
fn compute_array_combined_curve(curves: &[autoeq::Curve]) -> autoeq::Curve {
    if curves.is_empty() {
        return autoeq::Curve {
            freq: ndarray::Array1::zeros(0),
            spl: ndarray::Array1::zeros(0),
            phase: None,
        };
    }

    let freq = curves[0].freq.clone();
    let n = freq.len();
    let mut combined_spl = ndarray::Array1::zeros(n);

    for curve in curves {
        if curve.freq.len() == n {
            combined_spl += &curve.spl;
        }
    }

    combined_spl /= curves.len() as f64;

    autoeq::Curve {
        freq,
        spl: combined_spl,
        phase: None,
    }
}

/// Compute DBA combined curve (front + inverted rear)
fn compute_dba_combined_curve(
    front: &autoeq::Curve,
    rear: &autoeq::Curve,
    front_gain: f64,
    rear_gain: f64,
    _rear_delay: f64,
) -> autoeq::Curve {
    let freq = front.freq.clone();
    let n = freq.len();

    // Simple combination: front + rear with gains
    // In a full implementation, rear would be inverted and delayed
    let mut combined_spl = ndarray::Array1::zeros(n);

    if front.freq.len() == n {
        combined_spl += &(&front.spl + front_gain);
    }

    if rear.freq.len() == n {
        // Rear is typically inverted (phase flipped) in DBA
        combined_spl += &(&rear.spl + rear_gain);
    }

    combined_spl /= 2.0;

    autoeq::Curve {
        freq,
        spl: combined_spl,
        phase: None,
    }
}

// ============================================================================
// Result Curves Computation
// ============================================================================

/// Compute all visualization curves from optimization result
fn compute_result_curves(
    frequencies: &[f64],
    input_curve: &autoeq::Curve,
    target_curve: &autoeq::Curve,
    biquads: &[autoeq_iir::Biquad],
    spin_data: &Option<HashMap<String, autoeq::Curve>>,
) -> ResultCurves {
    let n = frequencies.len();

    // Input and target as vectors
    let input_vec: Vec<f64> = input_curve.spl.iter().copied().collect();
    let target_vec: Vec<f64> = target_curve.spl.iter().copied().collect();

    // Deviation = target - input
    let deviation_vec: Vec<f64> = target_vec
        .iter()
        .zip(input_vec.iter())
        .map(|(t, i)| t - i)
        .collect();

    // Filter response
    let filter_response = compute_filter_response(frequencies, biquads);

    // Individual filter responses
    let individual_filter_responses: Vec<Vec<f64>> = biquads
        .iter()
        .map(|biquad| {
            frequencies
                .iter()
                .map(|&freq| biquad.log_result(freq))
                .collect()
        })
        .collect();

    // Error = deviation - filter_response
    let error_vec: Vec<f64> = deviation_vec
        .iter()
        .zip(filter_response.iter())
        .map(|(d, f)| d - f)
        .collect();

    // Corrected = input + filter_response
    let corrected_vec: Vec<f64> = input_vec
        .iter()
        .zip(filter_response.iter())
        .map(|(i, f)| i + f)
        .collect();

    // Spinorama curves
    let (er_curve, sp_curve, er_di_curve, sp_di_curve) = if let Some(spin) = spin_data {
        let er = spin
            .get("Early Reflections")
            .map(|c| c.spl.iter().copied().collect())
            .unwrap_or_else(|| vec![0.0; n]);
        let sp = spin
            .get("Sound Power")
            .map(|c| c.spl.iter().copied().collect())
            .unwrap_or_else(|| vec![0.0; n]);

        // Directivity indices
        let er_di: Vec<f64> = input_vec
            .iter()
            .zip(er.iter())
            .map(|(on, er_val)| on - er_val)
            .collect();
        let sp_di: Vec<f64> = input_vec
            .iter()
            .zip(sp.iter())
            .map(|(on, sp_val)| on - sp_val)
            .collect();

        (er, sp, er_di, sp_di)
    } else {
        (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n])
    };

    ResultCurves {
        frequencies: frequencies.to_vec(),
        input_curve: input_vec,
        target_curve: target_vec,
        deviation_curve: deviation_vec,
        filter_response,
        error_curve: error_vec,
        corrected_curve: corrected_vec,
        individual_filter_responses,
        er_curve,
        sp_curve,
        er_di_curve,
        sp_di_curve,
    }
}

// ============================================================================
// Main Entry Points
// ============================================================================

/// Run speaker optimization with full roomeq features and callback support
///
/// # Arguments
/// * `config` - Speaker optimization configuration
/// * `callback` - Optional progress callback (called every N iterations)
///
/// # Returns
/// The optimization result with all curves for visualization
pub fn run_speaker_optimization_with_callback(
    config: &SpeakerOptimizationConfig,
    callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    match config.config_type {
        SpeakerConfigType::Single => {
            let input = config
                .main_measurement
                .as_ref()
                .ok_or("Single-driver config requires main_measurement")?;
            optimize_single_driver_full(input, config, callback)
        }
        SpeakerConfigType::MultiDriver => {
            if config.driver_measurements.is_empty() {
                return Err("Multi-driver config requires driver_measurements".to_string());
            }
            optimize_multidriver_full(&config.driver_measurements, config, callback)
        }
    }
}

/// Run extended speaker optimization (includes multi-sub and DBA)
pub fn run_speaker_optimization_extended(
    config: &SpeakerOptimizationConfigExt,
    callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    match config.config_type {
        SpeakerConfigTypeExt::Single => {
            let input = config
                .main_measurement
                .as_ref()
                .ok_or("Single-driver config requires main_measurement")?;
            let simple_config = SpeakerOptimizationConfig {
                config_type: SpeakerConfigType::Single,
                main_measurement: Some(input.clone()),
                driver_measurements: Vec::new(),
                crossover_type: None,
                crossover_freq_hints: Vec::new(),
                params: config.params.clone(),
                callback_config: config.callback_config.clone(),
                target: config.target.clone(),
            };
            optimize_single_driver_full(input, &simple_config, callback)
        }
        SpeakerConfigTypeExt::MultiDriver => {
            if config.driver_measurements.is_empty() {
                return Err("Multi-driver config requires driver_measurements".to_string());
            }
            let simple_config = SpeakerOptimizationConfig {
                config_type: SpeakerConfigType::MultiDriver,
                main_measurement: None,
                driver_measurements: config.driver_measurements.clone(),
                crossover_type: config.crossover_type,
                crossover_freq_hints: config.crossover_freq_hints.clone(),
                params: config.params.clone(),
                callback_config: config.callback_config.clone(),
                target: config.target.clone(),
            };
            optimize_multidriver_full(&config.driver_measurements, &simple_config, callback)
        }
        SpeakerConfigTypeExt::MultiSub => {
            optimize_multisub_full(&config.driver_measurements, config, callback)
        }
        SpeakerConfigTypeExt::Dba => optimize_dba_full(
            &config.front_measurements,
            &config.rear_measurements,
            config,
            callback,
        ),
    }
}

/// Backward-compatible entry point
pub fn run_speaker_optimization(
    speaker_model: &str,
    params: &OptimizationParams,
) -> Result<SpeakerOptimizationResult, String> {
    // Check for dummy speaker (for testing)
    if speaker_model == "Dummy Speaker" {
        return Ok(generate_dummy_result());
    }

    // Try to load from Spinorama API
    let config = SpeakerOptimizationConfig {
        config_type: SpeakerConfigType::Single,
        main_measurement: Some(MeasurementInput::Spinorama {
            speaker: speaker_model.to_string(),
            version: "asr".to_string(),
            measurement: "CEA2034".to_string(),
            curve_name: params.curve_name.clone(),
        }),
        driver_measurements: Vec::new(),
        crossover_type: None,
        crossover_freq_hints: Vec::new(),
        params: params.clone(),
        callback_config: Some(CallbackConfig::default()),
        target: None,
    };

    run_speaker_optimization_with_callback(&config, None)
}

// ============================================================================
// Full Optimization Implementations
// ============================================================================

/// Full single-driver optimization
fn optimize_single_driver_full(
    input: &MeasurementInput,
    config: &SpeakerOptimizationConfig,
    callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    // Load measurement
    let (input_curve, spin_data) = load_measurement_with_spin(input)?;

    // Create standard frequency grid
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);

    // Normalize input curve
    let input_normalized = autoeq::normalize_and_interpolate_response(&standard_freq, &input_curve);

    // Interpolate spin_data curves to standard frequency grid
    let spin_data_interpolated = spin_data.as_ref().map(|spin| {
        spin.iter()
            .map(|(name, curve)| {
                let interpolated = autoeq::normalize_and_interpolate_response(&standard_freq, curve);
                (name.clone(), interpolated)
            })
            .collect::<HashMap<String, autoeq::Curve>>()
    });

    // Load or build target curve
    let target_curve = if let Some(ref target_input) = config.target {
        let target = load_measurement(target_input)?;
        autoeq::normalize_and_interpolate_response(&standard_freq, &target)
    } else {
        // Build target based on curve_name
        let args = build_autoeq_args(&config.params);
        autoeq::workflow::build_target_curve(&args, &standard_freq, &input_normalized)
            .map_err(|e| e.to_string())?
    };

    // Run optimization
    let (biquads, history, initial_loss, final_loss) = optimize_single_driver(
        &input_normalized,
        &target_curve,
        &spin_data_interpolated,
        &config.params,
        &config.callback_config,
        callback,
    )?;

    // Compute result curves
    let frequencies: Vec<f64> = standard_freq.iter().copied().collect();
    let curves = compute_result_curves(
        &frequencies,
        &input_normalized,
        &target_curve,
        &biquads,
        &spin_data_interpolated,
    );

    Ok(SpeakerOptimizationResult {
        biquads,
        frequencies: curves.frequencies,
        input_curve: curves.input_curve,
        target_curve: curves.target_curve,
        deviation_curve: curves.deviation_curve,
        filter_response: curves.filter_response,
        error_curve: curves.error_curve,
        corrected_curve: curves.corrected_curve,
        individual_filter_responses: curves.individual_filter_responses,
        output_path: String::new(),
        er_curve: curves.er_curve,
        sp_curve: curves.sp_curve,
        er_di_curve: curves.er_di_curve,
        sp_di_curve: curves.sp_di_curve,
        optimization_history: history,
        initial_loss,
        final_loss,
        crossover_freqs: None,
        driver_gains: None,
        driver_delays: None,
    })
}

/// Full multi-driver optimization
fn optimize_multidriver_full(
    driver_inputs: &[MeasurementInput],
    config: &SpeakerOptimizationConfig,
    callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    // Load all driver measurements
    let mut driver_curves = Vec::new();
    for input in driver_inputs {
        let curve = load_measurement(input)?;
        driver_curves.push(curve);
    }

    let crossover_type = config.crossover_type.unwrap_or(CrossoverType::LR24);

    // Run multi-driver optimization
    let result = optimize_multidriver(
        driver_curves,
        crossover_type,
        &config.params,
        &config.callback_config,
        callback,
    )?;

    // Compute result curves
    let frequencies: Vec<f64> = result.combined_curve.freq.iter().copied().collect();
    let target = autoeq::Curve {
        freq: result.combined_curve.freq.clone(),
        spl: ndarray::Array1::zeros(result.combined_curve.freq.len()),
        phase: None,
    };
    let curves = compute_result_curves(
        &frequencies,
        &result.combined_curve,
        &target,
        &result.biquads,
        &None,
    );

    Ok(SpeakerOptimizationResult {
        biquads: result.biquads,
        frequencies: curves.frequencies,
        input_curve: curves.input_curve,
        target_curve: curves.target_curve,
        deviation_curve: curves.deviation_curve,
        filter_response: curves.filter_response,
        error_curve: curves.error_curve,
        corrected_curve: curves.corrected_curve,
        individual_filter_responses: curves.individual_filter_responses,
        output_path: String::new(),
        er_curve: curves.er_curve,
        sp_curve: curves.sp_curve,
        er_di_curve: curves.er_di_curve,
        sp_di_curve: curves.sp_di_curve,
        optimization_history: result.history,
        initial_loss: result.pre_score,
        final_loss: result.post_score,
        crossover_freqs: Some(result.crossover_freqs),
        driver_gains: Some(result.gains),
        driver_delays: Some(result.delays),
    })
}

/// Full multi-sub optimization
fn optimize_multisub_full(
    sub_inputs: &[MeasurementInput],
    config: &SpeakerOptimizationConfigExt,
    callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    // Load all sub measurements
    let mut sub_curves = Vec::new();
    for input in sub_inputs {
        let curve = load_measurement(input)?;
        sub_curves.push(curve);
    }

    // Run multi-sub optimization
    let result = optimize_multisub(
        sub_curves,
        &config.params,
        &config.callback_config,
        callback,
    )?;

    // Compute result curves
    let frequencies: Vec<f64> = result.combined_curve.freq.iter().copied().collect();
    let target = autoeq::Curve {
        freq: result.combined_curve.freq.clone(),
        spl: ndarray::Array1::zeros(result.combined_curve.freq.len()),
        phase: None,
    };
    let curves = compute_result_curves(
        &frequencies,
        &result.combined_curve,
        &target,
        &result.biquads,
        &None,
    );

    Ok(SpeakerOptimizationResult {
        biquads: result.biquads,
        frequencies: curves.frequencies,
        input_curve: curves.input_curve,
        target_curve: curves.target_curve,
        deviation_curve: curves.deviation_curve,
        filter_response: curves.filter_response,
        error_curve: curves.error_curve,
        corrected_curve: curves.corrected_curve,
        individual_filter_responses: curves.individual_filter_responses,
        output_path: String::new(),
        er_curve: curves.er_curve,
        sp_curve: curves.sp_curve,
        er_di_curve: curves.er_di_curve,
        sp_di_curve: curves.sp_di_curve,
        optimization_history: result.history,
        initial_loss: result.pre_score,
        final_loss: result.post_score,
        crossover_freqs: None,
        driver_gains: Some(result.gains),
        driver_delays: Some(result.delays),
    })
}

/// Full DBA optimization
fn optimize_dba_full(
    front_inputs: &[MeasurementInput],
    rear_inputs: &[MeasurementInput],
    config: &SpeakerOptimizationConfigExt,
    callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    // Load front array measurements
    let mut front_curves = Vec::new();
    for input in front_inputs {
        let curve = load_measurement(input)?;
        front_curves.push(curve);
    }

    // Load rear array measurements
    let mut rear_curves = Vec::new();
    for input in rear_inputs {
        let curve = load_measurement(input)?;
        rear_curves.push(curve);
    }

    // Run DBA optimization
    let result = optimize_dba(
        front_curves,
        rear_curves,
        &config.params,
        &config.callback_config,
        callback,
    )?;

    // Compute result curves
    let frequencies: Vec<f64> = result.combined_curve.freq.iter().copied().collect();
    let target = autoeq::Curve {
        freq: result.combined_curve.freq.clone(),
        spl: ndarray::Array1::zeros(result.combined_curve.freq.len()),
        phase: None,
    };
    let curves = compute_result_curves(
        &frequencies,
        &result.combined_curve,
        &target,
        &result.biquads,
        &None,
    );

    Ok(SpeakerOptimizationResult {
        biquads: result.biquads,
        frequencies: curves.frequencies,
        input_curve: curves.input_curve,
        target_curve: curves.target_curve,
        deviation_curve: curves.deviation_curve,
        filter_response: curves.filter_response,
        error_curve: curves.error_curve,
        corrected_curve: curves.corrected_curve,
        individual_filter_responses: curves.individual_filter_responses,
        output_path: String::new(),
        er_curve: curves.er_curve,
        sp_curve: curves.sp_curve,
        er_di_curve: curves.er_di_curve,
        sp_di_curve: curves.sp_di_curve,
        optimization_history: result.history,
        initial_loss: result.pre_score,
        final_loss: result.post_score,
        crossover_freqs: None,
        driver_gains: Some(result.gains),
        driver_delays: Some(result.delays),
    })
}

// ============================================================================
// Dummy Data Generator (for testing)
// ============================================================================

// ============================================================================
// Preview Curves (for displaying before optimization)
// ============================================================================

/// Result of loading preview curves
#[derive(Clone, Debug)]
pub struct PreviewCurves {
    /// Frequencies (Hz)
    pub frequencies: Vec<f64>,
    /// Input curve (dB) - the raw measurement
    pub input_curve: Vec<f64>,
    /// Target curve (dB) - what we're optimizing towards
    pub target_curve: Vec<f64>,
    /// Deviation curve (dB) - target minus input (what needs to be corrected)
    pub deviation_curve: Vec<f64>,
}

/// Load and compute preview curves for display before optimization
///
/// This loads the input measurement from Spinorama API and builds the target
/// curve based on the curve_name, allowing users to see what will be optimized.
///
/// # Arguments
/// * `speaker` - Speaker name (e.g., "KEF R3")
/// * `version` - Version (e.g., "asr")
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `curve_name` - Curve to optimize (e.g., "Estimated In-Room Response", "Listening Window")
///
/// # Returns
/// Preview curves for display
pub fn load_preview_curves(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<PreviewCurves, String> {
    // Create runtime for blocking API call
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create runtime: {}", e))?;

    rt.block_on(async {
        load_preview_curves_async(speaker, version, measurement, curve_name).await
    })
}

/// Async version of load_preview_curves
pub async fn load_preview_curves_async(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<PreviewCurves, String> {
    // Load input curve from Spinorama API
    let input = MeasurementInput::Spinorama {
        speaker: speaker.to_string(),
        version: version.to_string(),
        measurement: measurement.to_string(),
        curve_name: curve_name.to_string(),
    };

    let (input_curve, _spin_data) = load_measurement_with_spin(&input)?;

    // Create standard frequency grid
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);

    // Normalize input curve to standard grid
    let input_normalized = autoeq::normalize_and_interpolate_response(&standard_freq, &input_curve);

    // Build target curve based on curve_name
    // Create minimal args for build_target_curve
    let args = autoeq::Args {
        num_filters: 7,
        sample_rate: 48000.0,
        loss: autoeq::LossType::SpeakerFlat,
        algo: "nlopt:cobyla".to_string(),
        population: 100,
        maxeval: 10000,
        strategy: "currenttobest1bin".to_string(),
        min_db: -4.0,
        max_db: 4.0,
        min_q: 0.5,
        max_q: 6.0,
        min_freq: 20.0,
        max_freq: 20000.0,
        min_spacing_oct: 0.0,
        spacing_weight: 0.0,
        smooth: false,
        smooth_n: 1,
        refine: false,
        local_algo: "cobyla".to_string(),
        tolerance: 1e-6,
        atolerance: 1e-6,
        recombination: 0.9,
        adaptive_weight_f: 0.5,
        adaptive_weight_cr: 0.5,
        peq_model: autoeq::cli::PeqModel::Pk,
        curve: None,
        target: None,
        output: None,
        speaker: None,
        version: None,
        measurement: None,
        curve_name: curve_name.to_string(),
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

    let target_curve = autoeq::workflow::build_target_curve(&args, &standard_freq, &input_normalized)
        .map_err(|e| e.to_string())?;

    // Compute deviation = target - input
    let frequencies: Vec<f64> = standard_freq.iter().copied().collect();
    let input_vec: Vec<f64> = input_normalized.spl.iter().copied().collect();
    let target_vec: Vec<f64> = target_curve.spl.iter().copied().collect();
    let deviation_vec: Vec<f64> = target_vec
        .iter()
        .zip(input_vec.iter())
        .map(|(t, i)| t - i)
        .collect();

    Ok(PreviewCurves {
        frequencies,
        input_curve: input_vec,
        target_curve: target_vec,
        deviation_curve: deviation_vec,
    })
}

// ============================================================================
// Dummy Data Generator (for testing)
// ============================================================================

fn generate_dummy_result() -> SpeakerOptimizationResult {
    let n = 200;
    let frequencies: Vec<f64> = (0..n)
        .map(|i| 20.0 * (1000.0f64).powf(i as f64 / n as f64))
        .collect();
    let input_curve: Vec<f64> = frequencies
        .iter()
        .map(|f| (f / 1000.0).sin() * 5.0)
        .collect();
    let target_curve: Vec<f64> = vec![0.0; n];

    SpeakerOptimizationResult {
        biquads: Vec::new(),
        frequencies: frequencies.clone(),
        input_curve: input_curve.clone(),
        target_curve: target_curve.clone(),
        deviation_curve: input_curve.clone(),
        filter_response: vec![0.0; n],
        error_curve: input_curve.clone(),
        corrected_curve: input_curve.clone(),
        individual_filter_responses: Vec::new(),
        output_path: "/tmp/speaker_eq.txt".to_string(),
        er_curve: input_curve.iter().map(|v| v - 3.0).collect(),
        sp_curve: input_curve.iter().map(|v| v - 5.0).collect(),
        er_di_curve: vec![3.0; n],
        sp_di_curve: vec![5.0; n],
        optimization_history: vec![(0, 1.0), (10, 0.5), (20, 0.1)],
        initial_loss: 1.0,
        final_loss: 0.1,
        crossover_freqs: None,
        driver_gains: None,
        driver_delays: None,
    }
}
