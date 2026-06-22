use super::load::load_measurement_as_driver;
use super::speaker_optimization_config::SpeakerOptimizationConfig;
use super::speaker_optimization_config_ext::SpeakerOptimizationConfigExt;
use super::speaker_optimization_progress::SpeakerOptimizationProgress;
use super::speaker_optimization_result::SpeakerOptimizationResult;
use super::types::MeasurementInput;
use super::types::SpeakerOptimizationCallback;
pub use autoeq::de::CallbackAction;
pub use autoeq::{ProgressCallbackConfig, ProgressUpdate};
use std::sync::{Arc, Mutex};

/// Optimize single-driver speaker using autoeq library functions
pub(super) fn optimize_single_driver(
    input: &MeasurementInput,
    config: &SpeakerOptimizationConfig,
    mut callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    // Create tokio runtime for async operations
    let rt =
        tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;

    rt.block_on(async {
        // Extract spinorama parameters from input
        let (speaker, version, measurement, curve_name) = match input {
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

        let input_config = autoeq::workflow::InputConfig {
            speaker: Some(speaker.to_string()),
            version: Some(version.to_string()),
            measurement: Some(measurement.to_string()),
            curve_name: curve_name.to_string(),
            curve_path: None,
        };
        let optim_params = autoeq::OptimParams::from(&config.args);

        let result =
            autoeq::optimize_speaker(&input_config, &optim_params, progress_config, lib_callback)
                .await
                .map_err(|e| e.to_string())?;

        Ok(SpeakerOptimizationResult::from(result))
    })
}

/// Optimize from CSV file (fallback for non-spinorama data)
pub(super) fn optimize_from_csv(
    path: &std::path::Path,
    config: &SpeakerOptimizationConfig,
    callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    let curve = autoeq::read::read_curve_from_csv(&path.to_path_buf())
        .map_err(|e| format!("Failed to read CSV: {}", e))?;
    optimize_from_curve(&curve, config, callback)
}

/// Optimize from pre-loaded curve
pub(super) fn optimize_from_curve(
    curve: &autoeq::Curve,
    config: &SpeakerOptimizationConfig,
    mut callback: Option<SpeakerOptimizationCallback>,
) -> Result<SpeakerOptimizationResult, String> {
    // Create standard frequency grid
    let standard_freq = autoeq::read::create_log_frequency_grid(2000, 20.0, 20000.0);
    let input_normalized = autoeq::normalize_and_interpolate_response(&standard_freq, curve);

    // Build target curve
    let target_config = autoeq::workflow::TargetConfig::from(&config.args);
    let target_curve =
        autoeq::workflow::build_target_curve(&target_config, &standard_freq, &input_normalized)
            .map_err(|e| e.to_string())?;

    // Create deviation curve
    let deviation_curve = autoeq::Curve {
        freq: target_curve.freq.clone(),
        spl: &target_curve.spl - &input_normalized.spl,
        phase: None,
        ..Default::default()
    };

    // Setup objective data (no spin data for CSV/curve input)
    let optim_params = autoeq::OptimParams::from(&config.args);
    let (objective_data, _) = autoeq::workflow::setup_objective_data(
        &optim_params,
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
        &optim_params,
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

/// Optimize multi-driver speaker using crossover optimization
pub(super) fn optimize_multidriver(
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
        config.args.population,
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
pub(super) fn optimize_multisub(
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
        config.args.population,
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
pub(super) fn optimize_dba(
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
        config.args.population,
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
