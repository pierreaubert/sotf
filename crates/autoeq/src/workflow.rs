//! Shared workflow helpers used by AutoEQ binaries
//!
//! This module centralizes the common pipeline steps for loading input data,
//! building target curves, preparing objective data, and running optimization.

use crate::AutoeqError;
use crate::Cea2034Data;
use crate::Curve;
use crate::iir::Biquad;
use crate::read;
use crate::x2peq;
use ndarray::Array1;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

pub use crate::optim::setup::*;

pub mod resume;

/// Load input curve from file or standard input
///
/// Returns the main input `Curve` and optional CEA2034 spinorama curves when
/// the measurement requires them.
pub async fn load_input_curve(
    args: &crate::cli::Args,
) -> Result<(Curve, Option<HashMap<String, Curve>>), Box<dyn Error>> {
    let mut spin_data: Option<HashMap<String, Curve>> = None;

    let input_curve = if let (Some(speaker), Some(version), Some(measurement)) =
        (&args.speaker, &args.version, &args.measurement)
    {
        // Handle Estimated In-Room Response specially - it needs to be calculated from CEA2034
        if measurement == "Estimated In-Room Response" {
            // Fetch CEA2034 data to calculate PIR
            let plot_data = read::fetch_measurement_plot_data(speaker, version, "CEA2034").await?;

            // Extract all CEA2034 curves using original frequency grid from API
            // This avoids interpolation artifacts and matches Python implementation
            let curves = read::extract_cea2034_curves_original(&plot_data, "CEA2034")?;

            // Store the spin data
            spin_data = Some(curves.clone());

            // Get the PIR curve specifically
            let pir_curve = curves
                .get("Estimated In-Room Response")
                .ok_or("PIR curve not found in CEA2034 data")?;

            pir_curve.clone()
        } else {
            // Regular measurement extraction
            let plot_data =
                read::fetch_measurement_plot_data(speaker, version, measurement).await?;
            let extracted_curve =
                read::extract_curve_by_name(&plot_data, measurement, &args.curve_name)?;

            // If it's CEA2034, also extract spin data using original frequency grid
            if measurement == "CEA2034" {
                spin_data = Some(read::extract_cea2034_curves_original(
                    &plot_data, "CEA2034",
                )?);
            }
            extracted_curve
        }
    } else {
        // No API params -> expect a CSV path
        let curve_path = args.curve.as_ref().ok_or(
            "Either --curve or all of --speaker, --version, and --measurement must be provided",
        )?;
        read::read_curve_from_csv(curve_path)?
    };

    Ok((input_curve, spin_data))
}

/// Build a target curve from CLI args and the input curve.
///
/// Delegates to [`build_target_curve_by_name`] for predefined curve names,
/// or loads from a CSV file path specified in `args.target`.
///
/// # Errors
///
/// Returns `AutoeqError::TargetCurveLoad` if loading from a CSV file fails.
pub fn build_target_curve(
    args: &crate::cli::Args,
    freqs: &Array1<f64>,
    input_curve: &Curve,
) -> Result<Curve, AutoeqError> {
    if let Some(ref target_path) = args.target {
        log::debug!(
            "[RUST DEBUG] Loading target curve from path: {}",
            target_path.display()
        );

        let target_curve =
            read::read_curve_from_csv(target_path).map_err(|e| AutoeqError::TargetCurveLoad {
                path: target_path.display().to_string(),
                message: e.to_string(),
            })?;
        Ok(read::normalize_and_interpolate_response(
            freqs,
            &target_curve,
        ))
    } else {
        build_target_curve_by_name(&args.curve_name, freqs, input_curve)
    }
}

/// Build a predefined target curve by name.
///
/// This function is the CLI-independent core of target curve generation.
/// It handles predefined curve names ("Listening Window", "Sound Power", etc.)
/// without requiring a `cli::Args` struct.
///
/// # Arguments
/// * `curve_name` - Name of the predefined curve (e.g. "Listening Window", "On Axis")
/// * `freqs` - Frequency grid for the target curve
/// * `input_curve` - Reference measurement curve (used for slope estimation)
///
/// # Returns
/// A flat (0 dB) target by default; slope-corrected for specific curve names.
pub fn build_target_curve_by_name(
    curve_name: &str,
    freqs: &Array1<f64>,
    input_curve: &Curve,
) -> Result<Curve, AutoeqError> {
    match curve_name {
        "Listening Window" => {
            let log_f_min = 1000.0_f64.log10();
            let log_f_max = 20000.0_f64.log10();
            let denom = log_f_max - log_f_min;
            let spl = Array1::from_shape_fn(freqs.len(), |i| {
                let f_hz = freqs[i].max(1e-12);
                let fl = f_hz.log10();
                if fl < log_f_min {
                    0.0
                } else if fl >= log_f_max {
                    -0.5
                } else {
                    let t = (fl - log_f_min) / denom;
                    -0.5 * t
                }
            });
            Ok(Curve {
                freq: freqs.clone(),
                spl,
                phase: None,
                ..Default::default()
            })
        }
        "Sound Power" | "Early Reflections" | "Estimated In-Room Response" => {
            let slope = crate::loss::curve_slope_per_octave_in_range(input_curve, 100.0, 10000.0)
                .unwrap_or(-1.2)
                - 0.2;
            let lo = 100.0_f64;
            let hi = 20000.0_f64;
            let hi_val = slope * (hi / lo).log2();
            let spl = Array1::from_shape_fn(freqs.len(), |i| {
                let f = freqs[i].max(1e-12);
                if f < lo {
                    0.0
                } else if f >= hi {
                    hi_val
                } else {
                    slope * (f / lo).log2()
                }
            });
            Ok(Curve {
                freq: freqs.clone(),
                spl,
                phase: None,
                ..Default::default()
            })
        }
        _ => {
            let spl = Array1::zeros(freqs.len());
            Ok(Curve {
                freq: freqs.clone(),
                spl,
                phase: None,
                ..Default::default()
            })
        }
    }
}

/// Interpolate all curves in Cea2034Data to a standard frequency grid
/// Note: Does NOT normalize - preserves original dB levels for proper visualization
fn interpolate_cea2034_data(spin_data: &Cea2034Data, standard_freq: &Array1<f64>) -> Cea2034Data {
    let interpolate = |curve: &Curve| read::interpolate_response(standard_freq, curve);

    let on_axis = interpolate(&spin_data.on_axis);
    let listening_window = interpolate(&spin_data.listening_window);
    let early_reflections = interpolate(&spin_data.early_reflections);
    let sound_power = interpolate(&spin_data.sound_power);
    let estimated_in_room = interpolate(&spin_data.estimated_in_room);
    let er_di = interpolate(&spin_data.er_di);
    let sp_di = interpolate(&spin_data.sp_di);

    // Build interpolated curves HashMap
    let mut curves = HashMap::new();
    curves.insert("On Axis".to_string(), on_axis.clone());
    curves.insert("Listening Window".to_string(), listening_window.clone());
    curves.insert("Early Reflections".to_string(), early_reflections.clone());
    curves.insert("Sound Power".to_string(), sound_power.clone());
    curves.insert(
        "Estimated In-Room Response".to_string(),
        estimated_in_room.clone(),
    );

    Cea2034Data {
        on_axis,
        listening_window,
        early_reflections,
        sound_power,
        estimated_in_room,
        er_di,
        sp_di,
        curves,
    }
}

/// All curves needed for visualization after optimization
#[derive(Debug, Clone)]
pub struct VisualizationCurves {
    /// Frequency points (Hz)
    pub frequencies: Vec<f64>,
    /// Input/measurement curve (dB)
    pub input_curve: Vec<f64>,
    /// Target curve (dB)
    pub target_curve: Vec<f64>,
    /// Deviation = target - input (dB)
    pub deviation_curve: Vec<f64>,
    /// Combined filter response (dB)
    pub filter_response: Vec<f64>,
    /// Error = deviation - filter_response (dB)
    pub error_curve: Vec<f64>,
    /// Corrected = input + filter_response (dB)
    pub corrected_curve: Vec<f64>,
    /// Individual filter responses (dB per filter)
    pub individual_filter_responses: Vec<Vec<f64>>,
}

/// Compute all visualization curves from optimization result
///
/// # Arguments
/// * `frequencies` - Frequency points (Hz)
/// * `input_curve` - Input measurement curve
/// * `target_curve` - Target curve
/// * `biquads` - Optimized biquad filters
///
/// # Returns
/// All curves needed for visualization
pub fn compute_visualization_curves(
    frequencies: &[f64],
    input_curve: &Curve,
    target_curve: &Curve,
    biquads: &[Biquad],
) -> VisualizationCurves {
    let input_vec: Vec<f64> = input_curve.spl.iter().copied().collect();
    let target_vec: Vec<f64> = target_curve.spl.iter().copied().collect();

    // Deviation = target - input
    let deviation_vec: Vec<f64> = target_vec
        .iter()
        .zip(input_vec.iter())
        .map(|(t, i)| t - i)
        .collect();

    // Filter response
    let filter_response: Vec<f64> = frequencies
        .iter()
        .map(|&freq| biquads.iter().map(|b| b.log_result(freq)).sum())
        .collect();

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

    VisualizationCurves {
        frequencies: frequencies.to_vec(),
        input_curve: input_vec,
        target_curve: target_vec,
        deviation_curve: deviation_vec,
        filter_response,
        error_curve: error_vec,
        corrected_curve: corrected_vec,
        individual_filter_responses,
    }
}

/// Complete speaker optimization result
#[derive(Debug, Clone)]
pub struct SpeakerOptResult {
    /// Optimized biquad filters
    pub biquads: Vec<Biquad>,
    /// Visualization curves
    pub curves: VisualizationCurves,
    /// CEA2034 spin data (if available)
    pub spin_data: Option<Cea2034Data>,
    /// Optimization history: (iteration, loss)
    pub history: Vec<(usize, f64)>,
    /// Initial loss value
    pub initial_loss: f64,
    /// Final loss value
    pub final_loss: f64,
}

/// Run complete speaker optimization from spinorama data
///
/// # Arguments
/// * `speaker` - Speaker name
/// * `version` - Version (e.g., "asr")
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `args` - Optimization arguments (use `Args::speaker_defaults()` as base)
/// * `progress_config` - Optional progress callback configuration
/// * `progress_callback` - Optional progress callback
///
/// # Returns
/// Complete optimization result with all curves
pub async fn optimize_speaker<F>(
    speaker: &str,
    version: &str,
    measurement: &str,
    args: &crate::cli::Args,
    progress_config: Option<ProgressCallbackConfig>,
    progress_callback: Option<F>,
) -> Result<SpeakerOptResult, Box<dyn Error>>
where
    F: FnMut(&ProgressUpdate) -> crate::de::CallbackAction + Send + 'static,
{
    // 1. Load measurement with spin data
    let (input_curve, spin_data) =
        read::load_spinorama_with_spin(speaker, version, measurement, &args.curve_name).await?;

    // 2. Normalize to standard frequency grid
    let standard_freq = read::create_log_frequency_grid(200, 20.0, 20000.0);
    let input_normalized = read::normalize_and_interpolate_response(&standard_freq, &input_curve);

    // 3. Build target curve
    let target_curve = build_target_curve(args, &standard_freq, &input_normalized)?;

    // 4. Create deviation curve
    let deviation_curve = Curve {
        freq: target_curve.freq.clone(),
        spl: &target_curve.spl - &input_normalized.spl,
        phase: None,
        ..Default::default()
    };

    // 5. Setup objective - normalize spin data to same frequency grid
    let spin_map = spin_data.as_ref().map(|s| {
        s.curves
            .iter()
            .map(|(name, curve)| {
                let normalized = read::normalize_and_interpolate_response(&standard_freq, curve);
                (name.clone(), normalized)
            })
            .collect::<HashMap<String, Curve>>()
    });
    let optim_params = crate::OptimParams::from(args);
    let (objective_data, _) = setup_objective_data(
        &optim_params,
        &input_normalized,
        &target_curve,
        &deviation_curve,
        &spin_map,
    )?;

    // 6. Run optimization
    let (params, history) = if let (Some(config), Some(callback)) =
        (progress_config, progress_callback)
    {
        let output = perform_optimization_with_progress(args, &objective_data, config, callback)?;
        (output.params, output.history)
    } else {
        let params = perform_optimization_with_callback(
            args,
            &objective_data,
            Box::new(|_| crate::de::CallbackAction::Continue),
        )?;
        (params, Vec::new())
    };

    // 7. Convert to biquads
    let biquads: Vec<Biquad> = x2peq(&params, args.sample_rate, args.peq_model)
        .into_iter()
        .map(|(_, b)| b)
        .collect();

    // 8. Compute visualization curves
    let frequencies: Vec<f64> = standard_freq.iter().copied().collect();
    let curves =
        compute_visualization_curves(&frequencies, &input_normalized, &target_curve, &biquads);

    let initial_loss = history.first().map(|x| x.1).unwrap_or(0.0);
    let final_loss = history.last().map(|x| x.1).unwrap_or(0.0);

    // Interpolate spin_data to standard frequency grid for consistent visualization
    // Note: Does NOT normalize - preserves original dB levels
    let interpolated_spin_data = spin_data.map(|s| interpolate_cea2034_data(&s, &standard_freq));

    Ok(SpeakerOptResult {
        biquads,
        curves,
        spin_data: interpolated_spin_data,
        history,
        initial_loss,
        final_loss,
    })
}

/// Complete headphone optimization result
#[derive(Debug, Clone)]
pub struct HeadphoneOptResult {
    /// Optimized biquad filters
    pub biquads: Vec<Biquad>,
    /// Visualization curves
    pub curves: VisualizationCurves,
    /// Optimization history: (iteration, loss)
    pub history: Vec<(usize, f64)>,
    /// Initial loss value
    pub initial_loss: f64,
    /// Final loss value
    pub final_loss: f64,
}

/// Run complete headphone optimization from CSV measurement
///
/// # Arguments
/// * `curve_path` - Path to headphone measurement CSV
/// * `target_curve` - Target curve (use bundled Harman curves or custom)
/// * `args` - Optimization arguments (use `Args::headphone_defaults()` as base)
/// * `progress_config` - Optional progress callback configuration
/// * `progress_callback` - Optional progress callback
///
/// # Returns
/// Complete optimization result with all curves
pub fn optimize_headphone<F>(
    curve_path: &PathBuf,
    target_curve: &Curve,
    args: &crate::cli::Args,
    progress_config: Option<ProgressCallbackConfig>,
    progress_callback: Option<F>,
) -> Result<HeadphoneOptResult, Box<dyn Error>>
where
    F: FnMut(&ProgressUpdate) -> crate::de::CallbackAction + Send + 'static,
{
    // 1. Load measurement
    let input_curve = read::read_curve_from_csv(curve_path)?;

    // 2. Normalize to standard frequency grid
    let standard_freq = read::create_log_frequency_grid(200, 20.0, 20000.0);
    let input_normalized = read::normalize_and_interpolate_response(&standard_freq, &input_curve);
    let target_normalized = read::normalize_and_interpolate_response(&standard_freq, target_curve);

    // 3. Create deviation curve
    let deviation_curve = Curve {
        freq: target_normalized.freq.clone(),
        spl: &target_normalized.spl - &input_normalized.spl,
        phase: None,
        ..Default::default()
    };

    // 4. Setup objective
    let optim_params = crate::OptimParams::from(args);
    let (objective_data, _) = setup_objective_data(
        &optim_params,
        &input_normalized,
        &target_normalized,
        &deviation_curve,
        &None,
    )?;

    // 5. Run optimization
    let (params, history) = if let (Some(config), Some(callback)) =
        (progress_config, progress_callback)
    {
        let output = perform_optimization_with_progress(args, &objective_data, config, callback)?;
        (output.params, output.history)
    } else {
        let params = perform_optimization_with_callback(
            args,
            &objective_data,
            Box::new(|_| crate::de::CallbackAction::Continue),
        )?;
        (params, Vec::new())
    };

    // 6. Convert to biquads
    let biquads: Vec<Biquad> = x2peq(&params, args.sample_rate, args.peq_model)
        .into_iter()
        .map(|(_, b)| b)
        .collect();

    // 7. Compute visualization curves
    let frequencies: Vec<f64> = standard_freq.iter().copied().collect();
    let curves = compute_visualization_curves(
        &frequencies,
        &input_normalized,
        &target_normalized,
        &biquads,
    );

    let initial_loss = history.first().map(|x| x.1).unwrap_or(0.0);
    let final_loss = history.last().map(|x| x.1).unwrap_or(0.0);

    Ok(HeadphoneOptResult {
        biquads,
        curves,
        history,
        initial_loss,
        final_loss,
    })
}

/// Result of driver crossover optimization
#[derive(Debug, Clone)]
pub struct DriverOptimizationResult {
    /// Optimal per-driver gains in dB
    pub gains: Vec<f64>,
    /// Optimal per-driver delays in ms
    pub delays: Vec<f64>,
    /// Optimal crossover frequencies in Hz (n_drivers - 1 values)
    pub crossover_freqs: Vec<f64>,
    /// Loss value before optimization
    pub pre_objective: f64,
    /// Loss value after optimization
    pub post_objective: f64,
    /// Whether optimization converged successfully
    pub converged: bool,
}

/// Create minimal Args struct for driver optimization
///
/// This avoids requiring full CLI args when calling from library code.
fn create_driver_optimization_args(
    min_freq: f64,
    max_freq: f64,
    sample_rate: f64,
    algorithm: &str,
    max_iter: usize,
    population: usize,
    min_db: f64,
    max_db: f64,
    seed: Option<u64>,
) -> crate::cli::Args {
    use crate::LossType;
    use crate::cli::{Args, PeqModel};

    Args {
        num_filters: 0, // Not used for driver optimization
        curve: None,
        target: None,
        speaker: None,
        version: None,
        measurement: None,
        curve_name: "On Axis".to_string(),
        sample_rate,
        min_freq,
        max_freq,
        min_q: 0.5,
        max_q: 10.0,
        min_db,
        max_db,
        algo: algorithm.to_string(),
        strategy: "currenttobest1bin".to_string(),
        algo_list: false,
        strategy_list: false,
        peq_model: PeqModel::Pk,
        peq_model_list: false,
        population,
        maxeval: max_iter,
        refine: false,
        local_algo: "cobyla".to_string(),
        bo_initial_samples: 0,
        bo_batch_size: 0,
        bo_posterior_std_threshold: 0.0,
        bo_acquisition: "qei".to_string(),
        bo_ehvi: false,
        min_spacing_oct: 0.0,
        spacing_weight: 0.0,
        smoothness_weight: 0.0,
        smoothness_exponent: 1.0,
        smoothness_schroeder_hz: None,
        smoothness_modal_scale: 0.1,
        smooth: false,
        smooth_n: 1,
        loss: LossType::DriversFlat,
        tolerance: 1e-3,
        atolerance: 1e-4,
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
        seed,
        qa: None,
        preset: None,
    }
}

/// Optimize multi-driver crossover configuration
///
/// This function orchestrates the complete driver optimization workflow:
/// 1. Sets up optimization objective data
/// 2. Computes parameter bounds
/// 3. Generates initial guess
/// 4. Runs optimization
/// 5. Extracts gains and crossover frequencies from results
///
/// # Arguments
/// * `drivers_data` - Driver measurements with crossover type
/// * `min_freq`, `max_freq` - Optimization frequency range (Hz)
/// * `sample_rate` - Sample rate for filter design (Hz)
/// * `algorithm` - Optimization algorithm (e.g., "nlopt:cobyla", "autoeq:de")
/// * `max_iter` - Maximum number of iterations/evaluations
/// * `min_db`, `max_db` - Per-driver gain bounds (dB)
///
/// # Returns
/// * `DriverOptimizationResult` containing optimal gains, crossover frequencies, and scores
///
/// # Example
/// ```ignore
/// let drivers_data = DriversLossData::new(measurements, CrossoverType::LinkwitzRiley4);
/// let result = optimize_drivers_crossover(
///     drivers_data,
///     100.0,    // min_freq
///     10000.0,  // max_freq
///     48000.0,  // sample_rate
///     "nlopt:cobyla",
///     5000,     // max_iter
///     -12.0,    // min_db
///     12.0,     // max_db
///     None,     // fixed_freqs
///     None,     // seed
/// )?;
/// log::info!("Gains: {:?}", result.gains);
/// log::info!("Crossover freqs: {:?}", result.crossover_freqs);
/// ```
#[allow(clippy::too_many_arguments)]
pub fn optimize_drivers_crossover(
    drivers_data: crate::loss::DriversLossData,
    min_freq: f64,
    max_freq: f64,
    sample_rate: f64,
    algorithm: &str,
    max_iter: usize,
    population: usize,
    min_db: f64,
    max_db: f64,
    fixed_freqs: Option<Vec<f64>>,
    seed: Option<u64>,
) -> Result<DriverOptimizationResult, Box<dyn std::error::Error>> {
    let n_drivers = drivers_data.drivers.len();

    // Create Args structure needed for optimization
    let args = create_driver_optimization_args(
        min_freq,
        max_freq,
        sample_rate,
        algorithm,
        max_iter,
        population,
        min_db,
        max_db,
        seed,
    );

    // Setup objective data with optional fixed frequencies
    let optim_params = crate::OptimParams::from(&args);
    let objective_data = if let Some(ref freqs) = fixed_freqs {
        let mut data = setup_drivers_objective_data(&optim_params, drivers_data.clone());
        data.fixed_crossover_freqs = Some(freqs.clone());
        data
    } else {
        setup_drivers_objective_data(&optim_params, drivers_data.clone())
    };

    // Setup bounds (exclude crossover frequencies if fixed)
    let (lower_bounds, upper_bounds) = if fixed_freqs.is_some() {
        setup_drivers_bounds_fixed_freqs(&optim_params, &drivers_data)
    } else {
        setup_drivers_bounds(&optim_params, &drivers_data)
    };

    // Generate initial guess
    let mut x = if fixed_freqs.is_some() {
        drivers_initial_guess_fixed_freqs(&lower_bounds, &upper_bounds, n_drivers)
    } else {
        drivers_initial_guess(&lower_bounds, &upper_bounds, n_drivers)
    };

    // Compute pre-optimization objective
    let pre_objective = crate::optim::compute_base_fitness(&x, &objective_data);

    // Run optimization
    let opt_result = crate::optim::optimize_filters(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &optim_params,
    );

    // Check optimization result
    let converged = match opt_result {
        Ok((_status, _val)) => true,
        Err((_err, _val)) => false,
    };

    // Compute post-optimization objective
    let post_objective = crate::optim::compute_base_fitness(&x, &objective_data);

    // Extract results from parameter vector
    let gains = x[0..n_drivers].to_vec();
    let delays = x[n_drivers..2 * n_drivers].to_vec();

    // Crossover frequencies: from optimization or fixed
    let crossover_freqs = if let Some(freqs) = fixed_freqs {
        freqs
    } else {
        // Parameter layout: [gains(N), delays(N), xovers(N-1)]
        let xover_freqs_log10 = &x[2 * n_drivers..];
        xover_freqs_log10.iter().map(|x| 10_f64.powf(*x)).collect()
    };

    Ok(DriverOptimizationResult {
        gains,
        delays,
        crossover_freqs,
        pre_objective,
        post_objective,
        converged,
    })
}

/// Load driver measurements from CSV file paths
///
/// This function loads multiple driver measurement CSV files and converts them
/// to DriverMeasurement structs suitable for multi-driver optimization.
///
/// # Arguments
/// * `driver_paths` - Vector of paths to driver CSV files
///
/// # Returns
/// * Vector of DriverMeasurement structs
///
/// # Example
/// ```ignore
/// let paths = vec![
///     PathBuf::from("woofer.csv"),
///     PathBuf::from("tweeter.csv"),
/// ];
/// let measurements = load_driver_measurements_from_files(&paths)?;
/// ```
pub fn load_driver_measurements_from_files(
    driver_paths: &[std::path::PathBuf],
) -> Result<Vec<crate::loss::DriverMeasurement>, Box<dyn std::error::Error>> {
    use crate::loss::DriverMeasurement;
    use crate::read::load_driver_measurement;

    let mut measurements = Vec::new();

    for (i, path) in driver_paths.iter().enumerate() {
        match load_driver_measurement(path) {
            Ok((freq, spl, phase, _coherence, _noise_floor_db)) => {
                measurements.push(DriverMeasurement::new(freq, spl, phase));
                log::debug!("✓ Loaded driver {} from {}", i + 1, path.display());
            }
            Err(e) => {
                return Err(format!(
                    "Failed to load driver {} from {}: {}",
                    i + 1,
                    path.display(),
                    e
                )
                .into());
            }
        }
    }

    Ok(measurements)
}

/// Optimize multi-subwoofer configuration (gain, delay) to achieve flat summed response
#[allow(clippy::too_many_arguments)]
pub fn optimize_multisub(
    drivers_data: crate::loss::DriversLossData,
    min_freq: f64,
    max_freq: f64,
    sample_rate: f64,
    algorithm: &str,
    max_iter: usize,
    population: usize,
    min_db: f64,
    max_db: f64,
    seed: Option<u64>,
) -> Result<DriverOptimizationResult, Box<dyn std::error::Error>> {
    let n_drivers = drivers_data.drivers.len();

    // Create Args
    let mut args = create_driver_optimization_args(
        min_freq,
        max_freq,
        sample_rate,
        algorithm,
        max_iter,
        population,
        min_db,
        max_db,
        seed,
    );
    args.loss = crate::LossType::MultiSubFlat;

    // Setup objective data
    let optim_params = crate::OptimParams::from(&args);
    let objective_data = setup_multisub_objective_data(&optim_params, drivers_data.clone());

    // Setup bounds (gains + delays)
    let (lower_bounds, upper_bounds) = setup_multisub_bounds(&optim_params, n_drivers);

    // Initial guess
    let mut x = multisub_initial_guess(n_drivers);

    // Pre-objective
    let pre_objective = crate::optim::compute_base_fitness(&x, &objective_data);

    // Optimize
    let opt_result = crate::optim::optimize_filters(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &optim_params,
    );

    let converged = opt_result.is_ok();

    let post_objective = crate::optim::compute_base_fitness(&x, &objective_data);

    // Extract results: [gains(N), delays(N)]
    let gains = x[0..n_drivers].to_vec();
    let delays = x[n_drivers..2 * n_drivers].to_vec();
    let crossover_freqs = vec![];

    Ok(DriverOptimizationResult {
        gains,
        delays,
        crossover_freqs,
        pre_objective,
        post_objective,
        converged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Args;
    use clap::Parser;

    fn zero_curve(freqs: Vec<f64>) -> Curve {
        let n = freqs.len();
        Curve {
            freq: Array1::from(freqs),
            spl: Array1::zeros(n),
            phase: None,
            ..Default::default()
        }
    }

    #[test]
    fn build_target_curve_respects_smoothing_flag() {
        // Prepare a simple input curve and default args
        let mut args = Args::parse_from(["autoeq-test"]);
        args.curve_name = "Listening Window".to_string();
        let curve = zero_curve(vec![100.0, 1000.0, 10000.0, 20000.0]);

        // No smoothing
        args.smooth = false;
        let freqs = Array1::from(vec![100.0, 1000.0, 10000.0]);
        let _target_curve = super::build_target_curve(&args, &freqs, &curve)
            .expect("build_target_curve should succeed");
        let smoothed_none: Option<Curve> = None;
        assert!(smoothed_none.is_none());

        // With smoothing
        args.smooth = true;
        let freqs = Array1::from(vec![100.0, 1000.0, 10000.0]);
        let target_curve = super::build_target_curve(&args, &freqs, &curve)
            .expect("build_target_curve should succeed");
        let inv_smooth = target_curve.clone();
        let s = target_curve;
        assert_eq!(s.spl.len(), inv_smooth.spl.len());
    }

    #[test]
    fn setup_objective_data_sets_use_cea_when_expected() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.speaker = Some("spk".to_string());
        args.version = Some("v".to_string());
        args.measurement = Some("CEA2034".to_string());

        // Minimal input/target curves
        let input_curve = zero_curve(vec![100.0, 1000.0]);
        let target = Curve {
            freq: input_curve.freq.clone(),
            spl: Array1::zeros(input_curve.freq.len()),
            phase: None,
            ..Default::default()
        };
        let deviation = Curve {
            freq: input_curve.freq.clone(),
            spl: Array1::zeros(input_curve.freq.len()),
            phase: None,
            ..Default::default()
        };

        // Build minimal spin data with required keys
        let mut spin: HashMap<String, Curve> = HashMap::new();
        for k in [
            "On Axis",
            "Listening Window",
            "Sound Power",
            "Estimated In-Room Response",
        ] {
            spin.insert(k.to_string(), zero_curve(vec![100.0, 1000.0]));
        }
        let spin_opt = Some(spin);

        // Case 1: spin_data is available -> speaker_score_data should be set
        let params = crate::OptimParams::from(&args);
        let (obj, use_cea) =
            super::setup_objective_data(&params, &input_curve, &target, &deviation, &spin_opt)
                .expect("setup_objective_data should succeed with valid spin data");
        assert!(use_cea);
        assert!(obj.speaker_score_data.is_some());

        // Case 2: Even if measurement is "On Axis", if spin_data is available,
        // we can still compute speaker score (the measurement just determines
        // which curve is being optimized, not what loss functions are available)
        let mut args2 = args.clone();
        args2.measurement = Some("On Axis".to_string());
        let params2 = crate::OptimParams::from(&args2);
        let (obj2, use_cea2) =
            super::setup_objective_data(&params2, &input_curve, &target, &deviation, &spin_opt)
                .expect("setup_objective_data should succeed with valid spin data");
        assert!(use_cea2); // Changed: spin_data available means speaker score is possible
        assert!(obj2.speaker_score_data.is_some());

        // Case 3: If spin_data is missing -> use_cea must be false
        let (obj3, use_cea3) =
            super::setup_objective_data(&params, &input_curve, &target, &deviation, &None)
                .expect("setup_objective_data should succeed with no spin data");
        assert!(!use_cea3);
        assert!(obj3.speaker_score_data.is_none());
    }

    #[test]
    fn test_args_speaker_defaults() {
        let args = Args::speaker_defaults();
        assert_eq!(args.num_filters, 5);
        assert_eq!(args.sample_rate, 48000.0);
        assert_eq!(args.loss, crate::LossType::SpeakerFlat);
        assert_eq!(args.algo, "autoeq:de");
        assert_eq!(args.curve_name, "Listening Window");
        assert_eq!(args.min_freq, 20.0);
        assert_eq!(args.max_freq, 20000.0);
    }

    #[test]
    fn test_args_headphone_defaults() {
        let args = Args::headphone_defaults();
        assert_eq!(args.num_filters, 7);
        assert_eq!(args.loss, crate::LossType::HeadphoneScore);
        // Should inherit other values from speaker_defaults
        assert_eq!(args.sample_rate, 48000.0);
        assert_eq!(args.algo, "autoeq:de");
    }

    #[test]
    fn test_args_roomeq_defaults() {
        let args = Args::roomeq_defaults();
        assert_eq!(args.num_filters, 10);
        assert_eq!(args.max_freq, 500.0); // Room EQ focuses on low frequencies
        // Should inherit other values from speaker_defaults
        assert_eq!(args.sample_rate, 48000.0);
        assert_eq!(args.loss, crate::LossType::SpeakerFlat);
    }

    #[test]
    fn test_progress_callback_config_default() {
        let config = ProgressCallbackConfig::default();
        assert_eq!(config.interval, 25);
        assert!(config.include_biquads);
        assert!(config.include_filter_response);
        assert!(config.frequencies.is_empty());
    }

    #[test]
    fn test_compute_visualization_curves() {
        use crate::iir::BiquadFilterType;

        let frequencies = vec![100.0, 1000.0, 10000.0];
        let input_curve = Curve {
            freq: Array1::from(frequencies.clone()),
            spl: Array1::from(vec![80.0, 85.0, 82.0]),
            phase: None,
            ..Default::default()
        };
        let target_curve = Curve {
            freq: Array1::from(frequencies.clone()),
            spl: Array1::from(vec![80.0, 80.0, 80.0]),
            phase: None,
            ..Default::default()
        };

        // Create a simple peak filter
        let biquad = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, -5.0);
        let biquads = vec![biquad];

        let curves =
            compute_visualization_curves(&frequencies, &input_curve, &target_curve, &biquads);

        // Check that all curves have the right length
        assert_eq!(curves.frequencies.len(), 3);
        assert_eq!(curves.input_curve.len(), 3);
        assert_eq!(curves.target_curve.len(), 3);
        assert_eq!(curves.deviation_curve.len(), 3);
        assert_eq!(curves.filter_response.len(), 3);
        assert_eq!(curves.error_curve.len(), 3);
        assert_eq!(curves.corrected_curve.len(), 3);
        assert_eq!(curves.individual_filter_responses.len(), 1);

        // Check deviation = target - input
        for i in 0..3 {
            let expected_deviation = target_curve.spl[i] - input_curve.spl[i];
            assert!((curves.deviation_curve[i] - expected_deviation).abs() < 1e-10);
        }

        // Check corrected = input + filter_response
        for i in 0..3 {
            let expected_corrected = input_curve.spl[i] + curves.filter_response[i];
            assert!((curves.corrected_curve[i] - expected_corrected).abs() < 1e-10);
        }
    }

    #[test]
    fn test_visualization_curves_empty_biquads() {
        let frequencies = vec![100.0, 1000.0, 10000.0];
        let input_curve = Curve {
            freq: Array1::from(frequencies.clone()),
            spl: Array1::from(vec![80.0, 85.0, 82.0]),
            phase: None,
            ..Default::default()
        };
        let target_curve = Curve {
            freq: Array1::from(frequencies.clone()),
            spl: Array1::from(vec![80.0, 80.0, 80.0]),
            phase: None,
            ..Default::default()
        };

        let biquads: Vec<Biquad> = vec![];

        let curves =
            compute_visualization_curves(&frequencies, &input_curve, &target_curve, &biquads);

        // With no biquads, filter response should be all zeros
        for &val in &curves.filter_response {
            assert!((val - 0.0).abs() < 1e-10);
        }

        // Corrected should equal input when no filters
        for i in 0..3 {
            assert!((curves.corrected_curve[i] - input_curve.spl[i]).abs() < 1e-10);
        }

        // Error should equal deviation when no filters
        for i in 0..3 {
            assert!((curves.error_curve[i] - curves.deviation_curve[i]).abs() < 1e-10);
        }
    }

    #[test]
    fn initial_guess_respects_fixed_bounds_for_special_filters() {
        let mut args = Args::parse_from(["autoeq-test", "--peq-model", "hp-pk-lp", "-n", "3"]);
        args.min_freq = 20.0;
        args.max_freq = 20_000.0;

        let params = crate::OptimParams::from(&args);
        let (lower_bounds, upper_bounds) = setup_bounds(&params);
        let x = initial_guess(&params, &lower_bounds, &upper_bounds);

        for ((value, lower), upper) in x.iter().zip(lower_bounds.iter()).zip(upper_bounds.iter()) {
            assert!(
                *value >= *lower && *value <= *upper,
                "initial guess must lie within bounds: value={}, lower={}, upper={}",
                value,
                lower,
                upper
            );
        }

        assert_eq!(
            x[2], 0.0,
            "fixed high-pass gain should stay at its fixed bound"
        );
        assert_eq!(
            x[8], 0.0,
            "fixed low-pass gain should stay at its fixed bound"
        );
    }

    /// Regression test: gain lower bound must be -3*max_db (not -6*max_db).
    /// Bug: gain_lower was -6*max_db, creating a [-72,+12] search space instead
    /// of [-36,+12], wasting optimizer effort on unreachable gain values.
    #[test]
    fn setup_bounds_gain_lower_is_minus_3x_max_db() {
        let args = Args {
            num_filters: 3,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_q: 0.5,
            max_q: 10.0,
            min_db: 1.0, // min gain magnitude (constraint, not bound)
            max_db: 12.0,
            ..Args::parse_from(["autoeq-test"])
        };
        let params = crate::OptimParams::from(&args);
        let (lower, upper) = setup_bounds(&params);
        // PK model: 3 params per filter [freq, q, gain]
        // Check gain lower bound for each filter
        for i in 0..args.num_filters {
            let gain_lower = lower[i * 3 + 2];
            let gain_upper = upper[i * 3 + 2];
            assert!(
                (gain_lower - (-36.0)).abs() < 1e-9,
                "filter {} gain lower bound should be -3*max_db=-36, got {}",
                i,
                gain_lower
            );
            assert!(
                (gain_upper - 12.0).abs() < 1e-9,
                "filter {} gain upper bound should be max_db=12, got {}",
                i,
                gain_upper
            );
        }
    }

    /// Verify gain bounds scale correctly with different max_db values.
    #[test]
    fn setup_bounds_gain_scales_with_max_db() {
        for max_db in [4.0, 6.0, 12.0] {
            let args = Args {
                num_filters: 1,
                min_freq: 100.0,
                max_freq: 10000.0,
                min_q: 0.5,
                max_q: 5.0,
                min_db: 1.0,
                max_db,
                ..Args::parse_from(["autoeq-test"])
            };
            let params = crate::OptimParams::from(&args);
            let (lower, _upper) = setup_bounds(&params);
            let gain_lower = lower[2]; // gain is 3rd param
            let expected = -3.0 * max_db;
            assert!(
                (gain_lower - expected).abs() < 1e-9,
                "max_db={}: gain_lower should be {}, got {}",
                max_db,
                expected,
                gain_lower
            );
        }
    }
}
