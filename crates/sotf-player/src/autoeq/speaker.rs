//! Speaker EQ optimization
//!
//! Provides thin wrappers around the autoeq library for speaker equalization.
//! Most functionality is delegated to the library.

use super::types::SpeakerConfigType;
pub use autoeq::CrossoverType;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// Re-export types from autoeq for convenience
pub use autoeq::de::CallbackAction;
pub use autoeq::{
    Cea2034Data, OptimizationOutput, ProgressCallbackConfig, ProgressUpdate, SpeakerOptResult,
    VisualizationCurves,
};

// ============================================================================
// Progress and Callback Types (thin wrappers for backward compatibility)
// ============================================================================

/// Data passed to the optimization callback at each interval
/// This wraps autoeq::ProgressUpdate with additional stage information
#[derive(Debug, Clone)]
pub struct SpeakerOptimizationProgress {
    /// Current iteration number
    pub iteration: usize,
    /// Current loss/objective value
    pub loss: f64,
    /// Optional score value (higher is better, e.g., Harman score)
    pub score: Option<f64>,
    /// Convergence metric (population standard deviation)
    pub convergence: f64,
    /// Current best parameters (raw optimizer params)
    pub current_params: Vec<f64>,
    /// Current best biquad filters (decoded from params)
    pub current_biquads: Vec<math_audio_iir_fir::Biquad>,
    /// Current filter response curve (dB)
    pub current_filter_response: Vec<f64>,
    /// Stage of optimization
    pub stage: OptimizationStage,
    /// Total iterations expected (maxeval)
    pub max_iterations: usize,
}

impl From<&ProgressUpdate> for SpeakerOptimizationProgress {
    fn from(update: &ProgressUpdate) -> Self {
        Self {
            iteration: update.iteration,
            loss: update.loss,
            score: update.score,
            convergence: update.convergence,
            current_params: update.params.clone(),
            current_biquads: update.biquads.clone(),
            current_filter_response: update.filter_response.clone(),
            stage: OptimizationStage::Eq,
            max_iterations: update.max_iterations,
        }
    }
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

impl From<&CallbackConfig> for ProgressCallbackConfig {
    fn from(cfg: &CallbackConfig) -> Self {
        Self {
            interval: cfg.interval,
            include_biquads: cfg.include_biquads,
            include_filter_response: cfg.include_filter_response,
            frequencies: Vec::new(),
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
    /// Optimization arguments (use Args::speaker_defaults() as base)
    pub args: autoeq::Args,
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
            args: autoeq::Args::speaker_defaults(),
            callback_config: Some(CallbackConfig::default()),
            target: None,
        }
    }
}

/// Extended speaker config types including multi-sub and DBA
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeakerConfigTypeExt {
    #[default]
    Single,
    MultiDriver,
    MultiSub,
    Dba,
}

/// Extended configuration for speaker optimization including multi-sub and DBA
#[derive(Debug, Clone)]
pub struct SpeakerOptimizationConfigExt {
    pub config_type: SpeakerConfigTypeExt,
    pub main_measurement: Option<MeasurementInput>,
    pub driver_measurements: Vec<MeasurementInput>,
    pub front_measurements: Vec<MeasurementInput>,
    pub rear_measurements: Vec<MeasurementInput>,
    pub crossover_type: Option<CrossoverType>,
    pub crossover_freq_hints: Vec<f64>,
    pub args: autoeq::Args,
    pub callback_config: Option<CallbackConfig>,
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
            args: autoeq::Args::speaker_defaults(),
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
    pub biquads: Vec<math_audio_iir_fir::Biquad>,
    pub frequencies: Vec<f64>,
    pub input_curve: Vec<f64>,
    pub target_curve: Vec<f64>,
    pub deviation_curve: Vec<f64>,
    pub filter_response: Vec<f64>,
    pub error_curve: Vec<f64>,
    pub corrected_curve: Vec<f64>,
    pub normalized_curve: Vec<f64>,
    pub individual_filter_responses: Vec<Vec<f64>>,
    pub output_path: String,

    // Spinorama specific curves (from CEA2034 data)
    pub on_axis_curve: Vec<f64>,
    pub lw_curve: Vec<f64>,
    pub er_curve: Vec<f64>,
    pub sp_curve: Vec<f64>,
    pub pir_curve: Vec<f64>,
    pub er_di_curve: Vec<f64>,
    pub sp_di_curve: Vec<f64>,

    pub optimization_history: Vec<(usize, f64)>,
    pub initial_loss: f64,
    pub final_loss: f64,

    // Multi-driver results (optional)
    pub crossover_freqs: Option<Vec<f64>>,
    pub driver_gains: Option<Vec<f64>>,
    pub driver_delays: Option<Vec<f64>>,
}

impl From<SpeakerOptResult> for SpeakerOptimizationResult {
    fn from(result: SpeakerOptResult) -> Self {
        let n = result.curves.frequencies.len();

        // Extract spin data curves if available
        let (on_axis, lw, er, sp, pir, er_di, sp_di) = if let Some(ref spin) = result.spin_data {
            (
                spin.on_axis.spl.iter().copied().collect(),
                spin.listening_window.spl.iter().copied().collect(),
                spin.early_reflections.spl.iter().copied().collect(),
                spin.sound_power.spl.iter().copied().collect(),
                spin.estimated_in_room.spl.iter().copied().collect(),
                spin.er_di.spl.iter().copied().collect(),
                spin.sp_di.spl.iter().copied().collect(),
            )
        } else {
            (
                vec![0.0; n],
                vec![0.0; n],
                vec![0.0; n],
                vec![0.0; n],
                vec![0.0; n],
                vec![0.0; n],
                vec![0.0; n],
            )
        };

        Self {
            biquads: result.biquads,
            frequencies: result.curves.frequencies,
            input_curve: result.curves.input_curve.clone(),
            target_curve: result.curves.target_curve,
            deviation_curve: result.curves.deviation_curve,
            filter_response: result.curves.filter_response,
            error_curve: result.curves.error_curve,
            corrected_curve: result.curves.corrected_curve,
            normalized_curve: result.curves.input_curve.clone(),
            individual_filter_responses: result.curves.individual_filter_responses,
            output_path: String::new(),
            on_axis_curve: on_axis,
            lw_curve: lw,
            er_curve: er,
            sp_curve: sp,
            pir_curve: pir,
            er_di_curve: er_di,
            sp_di_curve: sp_di,
            optimization_history: result.history,
            initial_loss: result.initial_loss,
            final_loss: result.final_loss,
            crossover_freqs: None,
            driver_gains: None,
            driver_delays: None,
        }
    }
}

// ============================================================================
// Main Entry Points
// ============================================================================

/// Run speaker optimization with callback support
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
            optimize_single_driver(input, config, callback)
        }
        SpeakerConfigType::MultiDriver => {
            if config.driver_measurements.is_empty() {
                return Err("Multi-driver config requires driver_measurements".to_string());
            }
            optimize_multidriver(&config.driver_measurements, config, callback)
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
                args: config.args.clone(),
                callback_config: config.callback_config.clone(),
                target: config.target.clone(),
            };
            optimize_single_driver(input, &simple_config, callback)
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
                args: config.args.clone(),
                callback_config: config.callback_config.clone(),
                target: config.target.clone(),
            };
            optimize_multidriver(&config.driver_measurements, &simple_config, callback)
        }
        SpeakerConfigTypeExt::MultiSub => {
            optimize_multisub(&config.driver_measurements, config, callback)
        }
        SpeakerConfigTypeExt::Dba => optimize_dba(config, callback),
    }
}

/// Backward-compatible entry point
pub fn run_speaker_optimization(
    speaker_model: &str,
    args: &autoeq::Args,
) -> Result<SpeakerOptimizationResult, String> {
    // Check for dummy speaker (for testing)
    if speaker_model == "Dummy Speaker" {
        return Ok(generate_dummy_result());
    }

    let config = SpeakerOptimizationConfig {
        config_type: SpeakerConfigType::Single,
        main_measurement: Some(MeasurementInput::Spinorama {
            speaker: speaker_model.to_string(),
            version: "asr".to_string(),
            measurement: "CEA2034".to_string(),
            curve_name: args.curve_name.clone(),
        }),
        driver_measurements: Vec::new(),
        crossover_type: None,
        crossover_freq_hints: Vec::new(),
        args: args.clone(),
        callback_config: Some(CallbackConfig::default()),
        target: None,
    };

    run_speaker_optimization_with_callback(&config, None)
}

// ============================================================================
// Internal Implementation
// ============================================================================

/// Optimize single-driver speaker using autoeq library functions
fn optimize_single_driver(
    input: &MeasurementInput,
    config: &SpeakerOptimizationConfig,
    mut callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    // Create tokio runtime for async operations
    let rt =
        tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;

    rt.block_on(async {
        // Extract spinorama parameters from input
        let (speaker, version, measurement, _curve_name) = match input {
            MeasurementInput::Spinorama {
                speaker,
                version,
                measurement,
                curve_name,
            } => (
                speaker.as_str(),
                version.as_str(),
                measurement.as_str(),
                curve_name.as_str(),
            ),
            MeasurementInput::CsvFile(path) => {
                // For CSV files, use the low-level API
                return optimize_from_csv(path, config, callback);
            }
            MeasurementInput::Curve(curve) => {
                // For pre-loaded curves, use the low-level API
                return optimize_from_curve(curve, config, callback);
            }
        };

        // Use high-level library function
        let progress_config = config
            .callback_config
            .as_ref()
            .map(ProgressCallbackConfig::from);

        // Wrap callback to convert ProgressUpdate -> SpeakerOptimizationProgress
        let lib_callback = callback.take().map(|mut cb| {
            move |update: &ProgressUpdate| -> CallbackAction {
                let progress = SpeakerOptimizationProgress::from(update);
                cb(&progress)
            }
        });

        let result = autoeq::optimize_speaker(
            speaker,
            version,
            measurement,
            &config.args,
            progress_config,
            lib_callback,
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(SpeakerOptimizationResult::from(result))
    })
}

/// Optimize from CSV file (fallback for non-spinorama data)
fn optimize_from_csv(
    path: &std::path::Path,
    config: &SpeakerOptimizationConfig,
    callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    let curve = autoeq::read::read_curve_from_csv(&path.to_path_buf())
        .map_err(|e| format!("Failed to read CSV: {}", e))?;
    optimize_from_curve(&curve, config, callback)
}

/// Optimize from pre-loaded curve
fn optimize_from_curve(
    curve: &autoeq::Curve,
    config: &SpeakerOptimizationConfig,
    mut callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    // Create standard frequency grid
    let standard_freq = autoeq::read::create_log_frequency_grid(2000, 20.0, 20000.0);
    let input_normalized = autoeq::normalize_and_interpolate_response(&standard_freq, curve);

    // Build target curve
    let target_curve =
        autoeq::workflow::build_target_curve(&config.args, &standard_freq, &input_normalized)
            .map_err(|e| e.to_string())?;

    // Create deviation curve
    let deviation_curve = autoeq::Curve {
        freq: target_curve.freq.clone(),
        spl: &target_curve.spl - &input_normalized.spl,
        phase: None,
    };

    // Setup objective data (no spin data for CSV/curve input)
    let (objective_data, _) = autoeq::workflow::setup_objective_data(
        &config.args,
        &input_normalized,
        &target_curve,
        &deviation_curve,
        &None,
    )
    .map_err(|e| e.to_string())?;

    // Run optimization with progress callback
    let progress_config = config
        .callback_config
        .as_ref()
        .map(ProgressCallbackConfig::from)
        .unwrap_or_default();

    let history = Arc::new(Mutex::new(Vec::new()));
    let history_clone = history.clone();

    let lib_callback = move |update: &ProgressUpdate| -> CallbackAction {
        if let Ok(mut h) = history_clone.lock() {
            h.push((update.iteration, update.loss));
        }
        if let Some(ref mut cb) = callback {
            let progress = SpeakerOptimizationProgress::from(update);
            cb(&progress)
        } else {
            CallbackAction::Continue
        }
    };

    let output = autoeq::perform_optimization_with_progress(
        &config.args,
        &objective_data,
        progress_config,
        lib_callback,
    )
    .map_err(|e| e.to_string())?;

    // Convert to biquads
    let biquads: Vec<math_audio_iir_fir::Biquad> = autoeq::x2peq(
        &output.params,
        config.args.sample_rate,
        config.args.peq_model,
    )
    .into_iter()
    .map(|(_, b)| b)
    .collect();

    // Compute visualization curves
    let frequencies: Vec<f64> = standard_freq.iter().copied().collect();
    let curves = autoeq::compute_visualization_curves(
        &frequencies,
        &input_normalized,
        &target_curve,
        &biquads,
    );

    let history_vec = history
        .lock()
        .map_err(|_| "Failed to lock history")?
        .clone();
    let initial_loss = history_vec.first().map(|x| x.1).unwrap_or(0.0);
    let final_loss = history_vec.last().map(|x| x.1).unwrap_or(0.0);

    Ok(SpeakerOptimizationResult {
        biquads,
        frequencies: curves.frequencies,
        input_curve: curves.input_curve.clone(),
        target_curve: curves.target_curve,
        deviation_curve: curves.deviation_curve,
        filter_response: curves.filter_response,
        error_curve: curves.error_curve,
        corrected_curve: curves.corrected_curve,
        normalized_curve: curves.input_curve,
        individual_filter_responses: curves.individual_filter_responses,
        output_path: String::new(),
        on_axis_curve: vec![0.0; frequencies.len()],
        lw_curve: vec![0.0; frequencies.len()],
        er_curve: vec![0.0; frequencies.len()],
        sp_curve: vec![0.0; frequencies.len()],
        pir_curve: vec![0.0; frequencies.len()],
        er_di_curve: vec![0.0; frequencies.len()],
        sp_di_curve: vec![0.0; frequencies.len()],
        optimization_history: history_vec,
        initial_loss,
        final_loss,
        crossover_freqs: None,
        driver_gains: None,
        driver_delays: None,
    })
}


/// Load a MeasurementInput into an autoeq DriverMeasurement
fn load_measurement_as_driver(
    input: &MeasurementInput,
) -> Result<autoeq::loss::DriverMeasurement, String> {
    match input {
        MeasurementInput::CsvFile(path) => {
            let paths = vec![path.clone()];
            let measurements = autoeq::workflow::load_driver_measurements_from_files(&paths)
                .map_err(|e| e.to_string())?;
            measurements
                .into_iter()
                .next()
                .ok_or_else(|| "No measurement loaded from CSV".to_string())
        }
        MeasurementInput::Curve(curve) => {
            let freq = ndarray::Array1::from_vec(curve.freq.iter().copied().collect());
            let spl = ndarray::Array1::from_vec(curve.spl.iter().copied().collect());
            let phase = curve
                .phase
                .as_ref()
                .map(|p| ndarray::Array1::from_vec(p.iter().copied().collect()));
            Ok(autoeq::loss::DriverMeasurement::new(freq, spl, phase))
        }
        MeasurementInput::Spinorama { .. } => {
            Err("Spinorama input not supported for multi-driver optimization".to_string())
        }
    }
}

/// Optimize multi-driver speaker using crossover optimization
fn optimize_multidriver(
    driver_inputs: &[MeasurementInput],
    config: &SpeakerOptimizationConfig,
    _callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    if driver_inputs.len() < 2 {
        return Err("Multi-driver optimization requires at least 2 drivers".to_string());
    }

    // Load all driver measurements
    let mut driver_measurements = Vec::new();
    for input in driver_inputs {
        let measurement = load_measurement_as_driver(input)?;
        driver_measurements.push(measurement);
    }

    // Create DriversLossData
    let crossover_type = config.crossover_type.unwrap_or_default();
    let drivers_data = autoeq::loss::DriversLossData::new(driver_measurements, crossover_type);

    // Extract optimization parameters from config
    let min_freq = config.args.min_freq;
    let max_freq = config.args.max_freq;
    let sample_rate = config.args.sample_rate;
    let algorithm = &config.args.algo;
    let max_iter = config.args.maxeval;
    let min_db = config.args.min_db;
    let max_db = config.args.max_db;

    // Run crossover optimization
    let result = autoeq::workflow::optimize_drivers_crossover(
        drivers_data.clone(),
        min_freq,
        max_freq,
        sample_rate,
        algorithm,
        max_iter,
        min_db,
        max_db,
        None, // Optional: initial crossover frequencies
        config.args.seed,
    )
    .map_err(|e| e.to_string())?;

    // Create visualization curves from the optimized result
    let n = 2000;
    let frequencies = autoeq::read::create_log_frequency_grid(n, min_freq, max_freq);
    let freq_vec: Vec<f64> = frequencies.iter().copied().collect();

    // Build result (no EQ filters for multi-driver, just crossover settings)
    Ok(SpeakerOptimizationResult {
        biquads: Vec::new(), // Multi-driver doesn't produce EQ filters
        frequencies: freq_vec.clone(),
        input_curve: vec![0.0; n],
        target_curve: vec![0.0; n],
        deviation_curve: vec![0.0; n],
        filter_response: vec![0.0; n],
        error_curve: vec![0.0; n],
        corrected_curve: vec![0.0; n],
        normalized_curve: vec![0.0; n],
        individual_filter_responses: Vec::new(),
        output_path: String::new(),
        on_axis_curve: vec![0.0; n],
        lw_curve: vec![0.0; n],
        er_curve: vec![0.0; n],
        sp_curve: vec![0.0; n],
        pir_curve: vec![0.0; n],
        er_di_curve: vec![0.0; n],
        sp_di_curve: vec![0.0; n],
        optimization_history: vec![(0, result.pre_objective), (max_iter, result.post_objective)],
        initial_loss: result.pre_objective,
        final_loss: result.post_objective,
        crossover_freqs: Some(result.crossover_freqs),
        driver_gains: Some(result.gains),
        driver_delays: Some(result.delays),
    })
}

/// Optimize multi-subwoofer configuration
fn optimize_multisub(
    driver_inputs: &[MeasurementInput],
    config: &SpeakerOptimizationConfigExt,
    _callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    if driver_inputs.is_empty() {
        return Err("Multi-sub optimization requires at least 1 subwoofer measurement".to_string());
    }

    // Load all driver measurements
    let mut driver_measurements = Vec::new();
    for input in driver_inputs {
        let measurement = load_measurement_as_driver(input)?;
        driver_measurements.push(measurement);
    }

    // Create DriversLossData with no crossover (subs don't use crossover between them)
    let drivers_data =
        autoeq::loss::DriversLossData::new(driver_measurements, autoeq::CrossoverType::None);

    // Extract optimization parameters from config
    let min_freq = config.args.min_freq.max(20.0); // Sub optimization typically 20-200 Hz
    let max_freq = config.args.max_freq.min(200.0);
    let sample_rate = config.args.sample_rate;
    let algorithm = &config.args.algo;
    let max_iter = config.args.maxeval;
    let min_db = config.args.min_db;
    let max_db = config.args.max_db;

    // Run multi-sub optimization
    let result = autoeq::workflow::optimize_multisub(
        drivers_data,
        min_freq,
        max_freq,
        sample_rate,
        algorithm,
        max_iter,
        min_db,
        max_db,
        config.args.seed,
    )
    .map_err(|e| e.to_string())?;

    // Create visualization curves
    let n = 2000;
    let frequencies = autoeq::read::create_log_frequency_grid(n, min_freq, max_freq);
    let freq_vec: Vec<f64> = frequencies.iter().copied().collect();

    // Build result (multi-sub produces gains and delays, not EQ)
    Ok(SpeakerOptimizationResult {
        biquads: Vec::new(),
        frequencies: freq_vec.clone(),
        input_curve: vec![0.0; n],
        target_curve: vec![0.0; n],
        deviation_curve: vec![0.0; n],
        filter_response: vec![0.0; n],
        error_curve: vec![0.0; n],
        corrected_curve: vec![0.0; n],
        normalized_curve: vec![0.0; n],
        individual_filter_responses: Vec::new(),
        output_path: String::new(),
        on_axis_curve: vec![0.0; n],
        lw_curve: vec![0.0; n],
        er_curve: vec![0.0; n],
        sp_curve: vec![0.0; n],
        pir_curve: vec![0.0; n],
        er_di_curve: vec![0.0; n],
        sp_di_curve: vec![0.0; n],
        optimization_history: vec![(0, result.pre_objective), (max_iter, result.post_objective)],
        initial_loss: result.pre_objective,
        final_loss: result.post_objective,
        crossover_freqs: None,
        driver_gains: Some(result.gains),
        driver_delays: Some(result.delays),
    })
}

/// Optimize DBA (Double Bass Array) configuration
///
/// DBA uses front and rear subwoofers to cancel room modes.
/// Front subs are time-aligned, rear subs are delayed and inverted.
fn optimize_dba(
    config: &SpeakerOptimizationConfigExt,
    _callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    if config.front_measurements.is_empty() || config.rear_measurements.is_empty() {
        return Err(
            "DBA optimization requires both front and rear subwoofer measurements".to_string(),
        );
    }

    // Load front subwoofer measurements
    let mut all_measurements = Vec::new();
    for input in &config.front_measurements {
        let measurement = load_measurement_as_driver(input)?;
        all_measurements.push(measurement);
    }
    let front_count = all_measurements.len();

    // Load rear subwoofer measurements
    for input in &config.rear_measurements {
        let measurement = load_measurement_as_driver(input)?;
        all_measurements.push(measurement);
    }

    // Create DriversLossData (using None crossover type for sub optimization)
    let drivers_data =
        autoeq::loss::DriversLossData::new(all_measurements, autoeq::CrossoverType::None);

    // Extract optimization parameters
    let min_freq = config.args.min_freq.max(20.0);
    let max_freq = config.args.max_freq.min(200.0);
    let sample_rate = config.args.sample_rate;
    let algorithm = &config.args.algo;
    let max_iter = config.args.maxeval;
    let min_db = config.args.min_db;
    let max_db = config.args.max_db;

    // Run multi-sub optimization (DBA is a specialized multi-sub configuration)
    let result = autoeq::workflow::optimize_multisub(
        drivers_data,
        min_freq,
        max_freq,
        sample_rate,
        algorithm,
        max_iter,
        min_db,
        max_db,
        config.args.seed,
    )
    .map_err(|e| e.to_string())?;

    // For DBA, rear subs should have inverted polarity
    // This is typically applied by the user in their DSP, but we note it in the result
    let mut gains = result.gains.clone();
    let delays = result.delays.clone();

    // Invert rear sub gains (negative = inverted polarity)
    for gain in gains.iter_mut().skip(front_count) {
        *gain = -*gain;
    }

    // Create visualization curves
    let n = 200;
    let frequencies = autoeq::read::create_log_frequency_grid(n, min_freq, max_freq);
    let freq_vec: Vec<f64> = frequencies.iter().copied().collect();

    Ok(SpeakerOptimizationResult {
        biquads: Vec::new(),
        frequencies: freq_vec,
        input_curve: vec![0.0; n],
        target_curve: vec![0.0; n],
        deviation_curve: vec![0.0; n],
        filter_response: vec![0.0; n],
        error_curve: vec![0.0; n],
        corrected_curve: vec![0.0; n],
        normalized_curve: vec![0.0; n],
        individual_filter_responses: Vec::new(),
        output_path: String::new(),
        on_axis_curve: vec![0.0; n],
        lw_curve: vec![0.0; n],
        er_curve: vec![0.0; n],
        sp_curve: vec![0.0; n],
        pir_curve: vec![0.0; n],
        er_di_curve: vec![0.0; n],
        sp_di_curve: vec![0.0; n],
        optimization_history: vec![(0, result.pre_objective), (max_iter, result.post_objective)],
        initial_loss: result.pre_objective,
        final_loss: result.post_objective,
        crossover_freqs: None,
        driver_gains: Some(gains),
        driver_delays: Some(delays),
    })
}

// ============================================================================
// Preview Curves (for displaying before optimization)
// ============================================================================

/// Result of loading preview curves
#[derive(Clone, Debug)]
pub struct PreviewCurves {
    pub frequencies: Vec<f64>,
    pub input_curve: Vec<f64>,
    pub target_curve: Vec<f64>,
    pub deviation_curve: Vec<f64>,
}

/// Load and compute preview curves for display before optimization
pub fn load_preview_curves(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<PreviewCurves, String> {
    let rt =
        tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;

    rt.block_on(load_preview_curves_async(
        speaker,
        version,
        measurement,
        curve_name,
    ))
}

/// Async version of load_preview_curves
pub async fn load_preview_curves_async(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<PreviewCurves, String> {
    // Load input curve using library function
    let (input_curve, _spin_data) =
        autoeq::load_spinorama_with_spin(speaker, version, measurement, curve_name)
            .await
            .map_err(|e| e.to_string())?;

    // Create standard frequency grid
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);

    // Normalize input curve
    let input_normalized = autoeq::normalize_and_interpolate_response(&standard_freq, &input_curve);

    // Build target curve using default args
    let args = autoeq::Args::speaker_defaults();
    let target_curve =
        autoeq::workflow::build_target_curve(&args, &standard_freq, &input_normalized)
            .map_err(|e| e.to_string())?;

    // Compute deviation
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
        normalized_curve: input_curve.clone(),
        individual_filter_responses: Vec::new(),
        output_path: "/tmp/speaker_eq.txt".to_string(),
        on_axis_curve: input_curve.clone(),
        lw_curve: input_curve.clone(),
        er_curve: input_curve.iter().map(|v| v - 3.0).collect(),
        sp_curve: input_curve.iter().map(|v| v - 5.0).collect(),
        pir_curve: input_curve.iter().map(|v| v - 2.0).collect(),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Configuration Type Tests
    // ============================================================================

    #[test]
    fn test_speaker_config_type_default() {
        let config = SpeakerOptimizationConfig::default();
        assert!(matches!(config.config_type, SpeakerConfigType::Single));
        assert!(config.main_measurement.is_none());
        assert!(config.driver_measurements.is_empty());
    }

    #[test]
    fn test_speaker_config_type_ext_default() {
        let config = SpeakerOptimizationConfigExt::default();
        assert!(matches!(config.config_type, SpeakerConfigTypeExt::Single));
        assert!(config.main_measurement.is_none());
        assert!(config.driver_measurements.is_empty());
        assert!(config.front_measurements.is_empty());
        assert!(config.rear_measurements.is_empty());
    }

    #[test]
    fn test_callback_config_default() {
        let config = CallbackConfig::default();
        assert_eq!(config.interval, 25);
        assert!(config.include_biquads);
        assert!(config.include_filter_response);
    }

    // ============================================================================
    // MeasurementInput Validation Tests
    // ============================================================================

    #[test]
    fn test_load_measurement_spinorama_not_supported() {
        let input = MeasurementInput::Spinorama {
            speaker: "Test".to_string(),
            version: "asr".to_string(),
            measurement: "CEA2034".to_string(),
            curve_name: "LW".to_string(),
        };
        let result = load_measurement_as_driver(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not supported"));
    }

    #[test]
    fn test_load_measurement_csv_file_not_found() {
        let input =
            MeasurementInput::CsvFile(std::path::PathBuf::from("/nonexistent/path/driver.csv"));
        let result = load_measurement_as_driver(&input);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_measurement_from_curve() {
        let curve = autoeq::Curve {
            freq: ndarray::Array1::from_vec(vec![20.0, 100.0, 1000.0, 10000.0]),
            spl: ndarray::Array1::from_vec(vec![80.0, 85.0, 90.0, 85.0]),
            phase: None,
        };
        let input = MeasurementInput::Curve(curve);
        let result = load_measurement_as_driver(&input);
        assert!(result.is_ok());
        let driver = result.unwrap();
        assert_eq!(driver.freq.len(), 4);
        assert_eq!(driver.spl.len(), 4);
    }

    #[test]
    fn test_load_measurement_from_curve_with_phase() {
        let curve = autoeq::Curve {
            freq: ndarray::Array1::from_vec(vec![20.0, 100.0, 1000.0]),
            spl: ndarray::Array1::from_vec(vec![80.0, 85.0, 90.0]),
            phase: Some(ndarray::Array1::from_vec(vec![0.0, 45.0, 90.0])),
        };
        let input = MeasurementInput::Curve(curve);
        let result = load_measurement_as_driver(&input);
        assert!(result.is_ok());
        let driver = result.unwrap();
        assert!(driver.phase.is_some());
    }

    // ============================================================================
    // Multi-driver Optimization Validation Tests
    // ============================================================================

    #[test]
    fn test_multidriver_requires_two_drivers() {
        let config = SpeakerOptimizationConfig {
            config_type: SpeakerConfigType::MultiDriver,
            driver_measurements: vec![MeasurementInput::Spinorama {
                speaker: "Test".to_string(),
                version: "asr".to_string(),
                measurement: "CEA2034".to_string(),
                curve_name: "LW".to_string(),
            }],
            ..Default::default()
        };
        let result = optimize_multidriver(&config.driver_measurements, &config, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 2 drivers"));
    }

    #[test]
    fn test_multidriver_empty_measurements() {
        let config = SpeakerOptimizationConfig {
            config_type: SpeakerConfigType::MultiDriver,
            driver_measurements: vec![],
            ..Default::default()
        };
        let result = optimize_multidriver(&config.driver_measurements, &config, None);
        assert!(result.is_err());
    }

    // ============================================================================
    // Multi-sub Optimization Validation Tests
    // ============================================================================

    #[test]
    fn test_multisub_requires_measurements() {
        let config = SpeakerOptimizationConfigExt {
            config_type: SpeakerConfigTypeExt::MultiSub,
            driver_measurements: vec![],
            ..Default::default()
        };
        let result = optimize_multisub(&config.driver_measurements, &config, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 1 subwoofer"));
    }

    // ============================================================================
    // DBA Optimization Validation Tests
    // ============================================================================

    #[test]
    fn test_dba_requires_front_and_rear() {
        let config = SpeakerOptimizationConfigExt {
            config_type: SpeakerConfigTypeExt::Dba,
            front_measurements: vec![],
            rear_measurements: vec![],
            ..Default::default()
        };
        let result = optimize_dba(&config, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("front and rear"));
    }

    #[test]
    fn test_dba_requires_front() {
        let curve = autoeq::Curve {
            freq: ndarray::Array1::from_vec(vec![20.0, 100.0]),
            spl: ndarray::Array1::from_vec(vec![80.0, 85.0]),
            phase: None,
        };
        let config = SpeakerOptimizationConfigExt {
            config_type: SpeakerConfigTypeExt::Dba,
            front_measurements: vec![],
            rear_measurements: vec![MeasurementInput::Curve(curve)],
            ..Default::default()
        };
        let result = optimize_dba(&config, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("front and rear"));
    }

    #[test]
    fn test_dba_requires_rear() {
        let curve = autoeq::Curve {
            freq: ndarray::Array1::from_vec(vec![20.0, 100.0]),
            spl: ndarray::Array1::from_vec(vec![80.0, 85.0]),
            phase: None,
        };
        let config = SpeakerOptimizationConfigExt {
            config_type: SpeakerConfigTypeExt::Dba,
            front_measurements: vec![MeasurementInput::Curve(curve)],
            rear_measurements: vec![],
            ..Default::default()
        };
        let result = optimize_dba(&config, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("front and rear"));
    }

    // ============================================================================
    // Optimization Progress Tests
    // ============================================================================

    #[test]
    fn test_optimization_progress_from_update() {
        let update = autoeq::ProgressUpdate {
            iteration: 100,
            loss: 0.5,
            score: Some(85.0),
            convergence: 0.001,
            params: vec![1.0, 2.0, 3.0],
            biquads: vec![],
            filter_response: vec![0.0; 10],
            max_iterations: 1000,
        };
        let progress = SpeakerOptimizationProgress::from(&update);
        assert_eq!(progress.iteration, 100);
        assert!((progress.loss - 0.5).abs() < 0.001);
        assert_eq!(progress.score, Some(85.0));
        assert!((progress.convergence - 0.001).abs() < 0.0001);
        assert_eq!(progress.current_params.len(), 3);
        assert_eq!(progress.max_iterations, 1000);
        assert!(matches!(progress.stage, OptimizationStage::Eq));
    }

    #[test]
    fn test_optimization_stage_default() {
        let stage = OptimizationStage::default();
        assert!(matches!(stage, OptimizationStage::Eq));
    }

    // ============================================================================
    // Result Structure Tests
    // ============================================================================

    #[test]
    fn test_dummy_result_generation() {
        let result = generate_dummy_result();
        assert_eq!(result.frequencies.len(), 200);
        assert_eq!(result.input_curve.len(), 200);
        assert_eq!(result.normalized_curve.len(), 200);
        assert_eq!(result.input_curve, result.normalized_curve);
        assert_eq!(result.target_curve.len(), 200);
        assert!(!result.optimization_history.is_empty());
        assert!(result.initial_loss > result.final_loss);
    }

    #[test]
    fn test_speaker_opt_result_from_autoeq() {
        // Create a minimal SpeakerOptResult to test conversion
        let curves = autoeq::VisualizationCurves {
            frequencies: vec![20.0, 100.0, 1000.0],
            input_curve: vec![80.0, 85.0, 90.0],
            target_curve: vec![85.0, 85.0, 85.0],
            deviation_curve: vec![5.0, 0.0, -5.0],
            filter_response: vec![0.0, 0.0, 0.0],
            error_curve: vec![5.0, 0.0, -5.0],
            corrected_curve: vec![85.0, 85.0, 85.0],
            individual_filter_responses: vec![],
        };
        let autoeq_result = autoeq::SpeakerOptResult {
            biquads: vec![],
            curves,
            spin_data: None,
            history: vec![(0, 1.0), (100, 0.1)],
            initial_loss: 1.0,
            final_loss: 0.1,
        };
        let result = SpeakerOptimizationResult::from(autoeq_result);
        assert_eq!(result.frequencies.len(), 3);
        assert_eq!(result.normalized_curve.len(), 3);
        assert_eq!(result.input_curve, result.normalized_curve);
        assert!((result.initial_loss - 1.0).abs() < 0.001);
        assert!((result.final_loss - 0.1).abs() < 0.001);
    }

    // ============================================================================
    // Preview Curves Tests
    // ============================================================================

    #[test]
    fn test_preview_curves_struct() {
        let preview = PreviewCurves {
            frequencies: vec![20.0, 100.0, 1000.0],
            input_curve: vec![80.0, 85.0, 90.0],
            target_curve: vec![85.0, 85.0, 85.0],
            deviation_curve: vec![5.0, 0.0, -5.0],
        };
        assert_eq!(preview.frequencies.len(), 3);
        assert_eq!(preview.input_curve.len(), 3);
        assert_eq!(preview.target_curve.len(), 3);
        assert_eq!(preview.deviation_curve.len(), 3);
    }

    // ============================================================================
    // Backward Compatibility Tests
    // ============================================================================

    #[test]
    fn test_run_speaker_optimization_dummy() {
        let args = autoeq::Args::speaker_defaults();
        let result = run_speaker_optimization("Dummy Speaker", &args);
        assert!(result.is_ok());
        let opt_result = result.unwrap();
        assert!(!opt_result.frequencies.is_empty());
    }
}
