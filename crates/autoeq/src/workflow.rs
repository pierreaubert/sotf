//! Shared workflow helpers used by AutoEQ binaries
//!
//! This module centralizes the common pipeline steps for loading input data,
//! building target curves, preparing objective data, and running optimization.

use crate::AutoeqError;
use crate::Cea2034Data;
use crate::Curve;
use crate::HeadphoneLossData;
use crate::PeqModel;
use crate::SpeakerLossData;
use crate::iir::Biquad;
use crate::loss::DriversLossData;
use crate::optim::{ObjectiveData, optimize_filters_with_algo_override};
use crate::optim_de::optimize_filters_autoeq_with_callback;
use crate::read;
use crate::x2peq;
use ndarray::Array1;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

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

/// Build a target curve (and optional smoothed version) from CLI args and the input curve.
/// Returns (inverted_curve, smoothed_curve_opt).
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
        crate::qa_println!(
            args,
            "[RUST DEBUG] Loading target curve from path: {}",
            target_path.display()
        );
        crate::qa_println!(
            args,
            "[RUST DEBUG] Current working directory: {:?}",
            std::env::current_dir()
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
        match args.curve_name.as_str() {
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
                })
            }
            "Sound Power" | "Early Reflections" | "Estimated In-Room Response" => {
                let slope =
                    crate::loss::curve_slope_per_octave_in_range(input_curve, 100.0, 10000.0)
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
                })
            }
            _ => {
                let spl = Array1::zeros(freqs.len());
                Ok(Curve {
                    freq: freqs.clone(),
                    spl,
                    phase: None,
                })
            }
        }
    }
}

/// Prepare the ObjectiveData and whether CEA2034-based scoring is active.
///
/// # Errors
///
/// Returns `AutoeqError::MissingCea2034Curve` if spin_data is provided but missing required curves.
/// Returns `AutoeqError::CurveLengthMismatch` if spin_data curves have inconsistent lengths.
pub fn setup_objective_data(
    args: &crate::cli::Args,
    input_curve: &Curve,
    target_curve: &Curve,
    deviation_curve: &Curve,
    spin_data: &Option<HashMap<String, Curve>>,
) -> Result<(ObjectiveData, bool), AutoeqError> {
    // CEA2034 data is available if spin_data was provided.
    // This can happen either via:
    // 1. CLI path: args.measurement=CEA2034 and args.speaker/version are set
    // 2. Library path: spin_data passed directly from API fetch
    // The key requirement is just having spin_data available.
    let use_cea = spin_data.is_some();

    let speaker_score_data_opt = if let Some(spin) = spin_data {
        Some(SpeakerLossData::try_new(spin)?)
    } else {
        None
    };

    // Headphone score data is available when NOT using CEA2034 speaker data
    let headphone_score_data_opt = if !use_cea {
        Some(HeadphoneLossData::new(args.smooth, args.smooth_n))
    } else {
        None
    };

    let objective_data = ObjectiveData {
        freqs: input_curve.freq.clone(),
        target: target_curve.spl.clone(),
        deviation: deviation_curve.spl.clone(), // This is the deviation to be corrected
        srate: args.sample_rate,
        min_spacing_oct: args.min_spacing_oct,
        spacing_weight: args.spacing_weight,
        max_db: args.max_db,
        min_db: args.min_db,
        min_freq: args.min_freq,
        max_freq: args.max_freq,
        peq_model: args.effective_peq_model(),
        loss_type: args.loss,
        speaker_score_data: speaker_score_data_opt,
        headphone_score_data: headphone_score_data_opt,
        // Store input curve for headphone loss calculation
        input_curve: if !use_cea {
            Some(input_curve.clone())
        } else {
            None
        },
        // Multi-driver data will be set separately
        drivers_data: None,
        fixed_crossover_freqs: None,
        // Penalties default to zero; configured per algorithm in optimize_filters
        penalty_w_ceiling: 0.0,
        penalty_w_spacing: 0.0,
        penalty_w_mingain: 0.0,
        // Integrality constraints - none for continuous optimization
        integrality: None,
    };

    Ok((objective_data, use_cea))
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

/// Set up objective data for multi-driver crossover optimization
///
/// # Arguments
/// * `args` - CLI arguments
/// * `drivers_data` - Multi-driver measurement data
///
/// # Returns
/// * ObjectiveData configured for multi-driver optimization
pub fn setup_drivers_objective_data(
    args: &crate::cli::Args,
    drivers_data: DriversLossData,
) -> ObjectiveData {
    ObjectiveData {
        freqs: drivers_data.freq_grid.clone(),
        target: Array1::zeros(drivers_data.freq_grid.len()),
        deviation: Array1::zeros(drivers_data.freq_grid.len()),
        srate: args.sample_rate,
        min_spacing_oct: 0.0, // Not applicable for crossover optimization
        spacing_weight: 0.0,
        max_db: args.max_db,
        min_db: args.min_db,
        min_freq: args.min_freq,
        max_freq: args.max_freq,
        peq_model: args.effective_peq_model(),
        loss_type: crate::LossType::DriversFlat,
        speaker_score_data: None,
        headphone_score_data: None,
        input_curve: None,
        drivers_data: Some(drivers_data),
        fixed_crossover_freqs: None,
        penalty_w_ceiling: 0.0,
        penalty_w_spacing: 0.0,
        penalty_w_mingain: 0.0,
        integrality: None,
    }
}

/// Build optimization parameter bounds for multi-driver crossover optimization
///
/// # Arguments
/// * `args` - CLI arguments
/// * `drivers_data` - Multi-driver measurement data
///
/// # Returns
/// * Tuple of (lower_bounds, upper_bounds)
///
/// # Parameter Vector Layout
/// For N drivers: [gain1, gain2, ..., gainN, xover_freq1, xover_freq2, ..., xover_freq(N-1)]
/// - Gains are in dB, bounded by [-max_db, max_db]
/// - Crossover frequencies are in Hz (log10 space), bounded by driver frequency ranges
pub fn setup_drivers_bounds(
    args: &crate::cli::Args,
    drivers_data: &DriversLossData,
) -> (Vec<f64>, Vec<f64>) {
    let n_drivers = drivers_data.drivers.len();
    let n_params = n_drivers * 2 + (n_drivers - 1); // N gains + N delays + (N-1) crossovers

    let mut lower_bounds = Vec::with_capacity(n_params);
    let mut upper_bounds = Vec::with_capacity(n_params);

    // Bounds for gains: [-max_db, max_db]
    for _ in 0..n_drivers {
        lower_bounds.push(-args.max_db);
        upper_bounds.push(args.max_db);
    }

    // Bounds for delays: [-5.0, 5.0] ms
    for _ in 0..n_drivers {
        lower_bounds.push(-5.0);
        upper_bounds.push(5.0);
    }

    // Bounds for crossover frequencies
    // Each crossover should be between the mean frequencies of adjacent drivers
    for i in 0..(n_drivers - 1) {
        let driver_low = &drivers_data.drivers[i];
        let driver_high = &drivers_data.drivers[i + 1];

        // Use geometric mean frequencies as characteristic frequencies of each driver
        let mean_low = driver_low.mean_freq();
        let mean_high = driver_high.mean_freq();

        // Crossover should be between the geometric means with reasonable margin
        // Use log10 space for better optimization
        // Allow range from 0.5x mean_low to 2x mean_high, centered on geometric mean of the two means
        let geometric_center = (mean_low * mean_high).sqrt();
        let xover_min = (geometric_center * 0.5).max(args.min_freq).log10();
        let xover_max = (geometric_center * 2.0).min(args.max_freq).log10();

        // Ensure bounds are valid
        let xover_min = xover_min.min(xover_max - 0.1);

        lower_bounds.push(xover_min);
        upper_bounds.push(xover_max);
    }

    (lower_bounds, upper_bounds)
}

/// Generate initial guess for multi-driver crossover optimization
///
/// # Arguments
/// * `lower_bounds` - Lower bounds for parameters
/// * `upper_bounds` - Upper bounds for parameters
/// * `n_drivers` - Number of drivers
///
/// # Returns
/// * Initial guess vector: [gains, crossover_freqs_log10]
pub fn drivers_initial_guess(
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    n_drivers: usize,
) -> Vec<f64> {
    let mut x = Vec::new();

    // Initial gains: start with 0 dB for all drivers
    x.extend(vec![0.0; n_drivers]);

    // Initial delays: start with 0 ms
    x.extend(vec![0.0; n_drivers]);

    // Initial crossover frequencies: use geometric mean of bounds (in log space)
    // Crossovers start at index 2*n_drivers
    for i in (2 * n_drivers)..lower_bounds.len() {
        let xover_log10 = (lower_bounds[i] + upper_bounds[i]) / 2.0;
        x.push(xover_log10);
    }

    x
}

/// Build optimization parameter bounds for multi-driver optimization with fixed crossover frequencies
///
/// When crossover frequencies are fixed, we only optimize gains and delays.
///
/// # Arguments
/// * `args` - CLI arguments
/// * `drivers_data` - Multi-driver measurement data
///
/// # Returns
/// * Tuple of (lower_bounds, upper_bounds)
///
/// # Parameter Vector Layout
/// For N drivers: [gain1, gain2, ..., gainN, delay1, delay2, ..., delayN]
/// - Gains are in dB, bounded by [-max_db, max_db]
/// - Delays are in ms, bounded by [-5, 5]
pub fn setup_drivers_bounds_fixed_freqs(
    args: &crate::cli::Args,
    drivers_data: &DriversLossData,
) -> (Vec<f64>, Vec<f64>) {
    let n_drivers = drivers_data.drivers.len();
    let n_params = n_drivers * 2; // N gains + N delays (no crossovers)

    let mut lower_bounds = Vec::with_capacity(n_params);
    let mut upper_bounds = Vec::with_capacity(n_params);

    // Bounds for gains: [-max_db, max_db]
    for _ in 0..n_drivers {
        lower_bounds.push(-args.max_db);
        upper_bounds.push(args.max_db);
    }

    // Bounds for delays: [-5.0, 5.0] ms
    for _ in 0..n_drivers {
        lower_bounds.push(-5.0);
        upper_bounds.push(5.0);
    }

    (lower_bounds, upper_bounds)
}

/// Generate initial guess for multi-driver optimization with fixed crossover frequencies
///
/// # Arguments
/// * `_lower_bounds` - Lower bounds for parameters (unused, for API consistency)
/// * `_upper_bounds` - Upper bounds for parameters (unused, for API consistency)
/// * `n_drivers` - Number of drivers
///
/// # Returns
/// * Initial guess vector: [gains, delays]
pub fn drivers_initial_guess_fixed_freqs(
    _lower_bounds: &[f64],
    _upper_bounds: &[f64],
    n_drivers: usize,
) -> Vec<f64> {
    let mut x = Vec::new();

    // Initial gains: start with 0 dB for all drivers
    x.extend(vec![0.0; n_drivers]);

    // Initial delays: start with 0 ms
    x.extend(vec![0.0; n_drivers]);

    x
}

/// Build optimization parameter bounds for the optimizer.
pub fn setup_bounds(args: &crate::cli::Args) -> (Vec<f64>, Vec<f64>) {
    use crate::cli::PeqModel;

    let model = args.effective_peq_model();
    let ppf = crate::param_utils::params_per_filter(model);
    let num_params = args.num_filters * ppf;
    let mut lower_bounds = Vec::with_capacity(num_params);
    let mut upper_bounds = Vec::with_capacity(num_params);

    let spacing = 1.0; // Overlap factor - allows adjacent filters to overlap
    let gain_lower = -6.0 * args.max_db;
    let q_lower = args.min_q.max(0.1);
    let range = (args.max_freq.log10() - args.min_freq.log10()) / (args.num_filters as f64);

    for i in 0..args.num_filters {
        // Center frequency for this filter in log space
        let f_center = args.min_freq.log10() + (i as f64) * range;

        // Calculate bounds with overlap
        // Each filter can range from (center - spacing*range) to (center + spacing*range)
        let f_low = (f_center - spacing * range).max(args.min_freq.log10());
        let f_high = (f_center + spacing * range).min(args.max_freq.log10());

        // Ensure progressive increase: each filter's lower bound should be >= previous filter's lower bound
        let f_low_adjusted = if i > 0 {
            // Get the frequency lower bound of the previous filter
            let prev_freq_idx = if ppf == 3 {
                (i - 1) * 3
            } else {
                (i - 1) * 4 + 1
            };
            f_low.max(lower_bounds[prev_freq_idx])
        } else {
            f_low
        };

        // Ensure upper bound is also progressive (but can overlap)
        let f_high_adjusted = if i > 0 {
            let prev_freq_idx = if ppf == 3 {
                (i - 1) * 3
            } else {
                (i - 1) * 4 + 1
            };
            f_high.max(upper_bounds[prev_freq_idx])
        } else {
            f_high
        };

        // Add bounds based on model type
        match model {
            PeqModel::Pk
            | PeqModel::HpPk
            | PeqModel::HpPkLp
            | PeqModel::LsPk
            | PeqModel::LsPkHs => {
                // Fixed filter types: [freq, Q, gain]
                lower_bounds.extend_from_slice(&[f_low_adjusted, q_lower, gain_lower]);
                upper_bounds.extend_from_slice(&[f_high_adjusted, args.max_q, args.max_db]);
            }
            PeqModel::FreePkFree | PeqModel::Free => {
                // Free filter types: [type, freq, Q, gain]
                let (type_low, type_high) = if model == PeqModel::Free
                    || (model == PeqModel::FreePkFree && (i == 0 || i == args.num_filters - 1))
                {
                    crate::param_utils::filter_type_bounds()
                } else {
                    (0.0, 0.999) // Peak filter only
                };
                lower_bounds.extend_from_slice(&[type_low, f_low_adjusted, q_lower, gain_lower]);
                upper_bounds.extend_from_slice(&[
                    type_high,
                    f_high_adjusted,
                    args.max_q,
                    args.max_db,
                ]);
            }
        }
    }

    // Apply model-specific constraints
    match model {
        PeqModel::HpPk | PeqModel::HpPkLp => {
            // First filter is highpass - fixed 3-param layout
            lower_bounds[0] = 20.0_f64.max(args.min_freq).log10();
            upper_bounds[0] = 120.0_f64.min(args.min_freq + 20.0).log10();
            lower_bounds[1] = 1.0;
            upper_bounds[1] = 1.5; // could be tuned as a function of max_db
            lower_bounds[2] = 0.0;
            upper_bounds[2] = 0.0;
        }
        PeqModel::LsPk | PeqModel::LsPkHs => {
            // First filter is low shelves - fixed 3-param layout
            lower_bounds[0] = 20.0_f64.max(args.min_freq).log10();
            upper_bounds[0] = 120.0_f64.min(args.min_freq + 20.0).log10();
            lower_bounds[1] = args.min_q;
            upper_bounds[1] = args.max_q;
            lower_bounds[2] = -args.max_db;
            upper_bounds[2] = args.max_db;
        }
        _ => {}
    }

    if args.num_filters > 1 {
        if matches!(model, PeqModel::HpPkLp) {
            // Last filter is lowpass - fixed 3-param layout
            let last_idx = (args.num_filters - 1) * ppf;
            if ppf == 3 {
                lower_bounds[last_idx] = (args.max_freq - 2000.0).max(5000.0).log10();
                upper_bounds[last_idx] = args.max_freq.log10();
                lower_bounds[last_idx + 1] = 1.0;
                upper_bounds[last_idx + 1] = 1.5;
                lower_bounds[last_idx + 2] = 0.0;
                upper_bounds[last_idx + 2] = 0.0;
            }
        }

        if matches!(model, PeqModel::LsPkHs) {
            // Last filter is lowpass - fixed 3-param layout
            let last_idx = (args.num_filters - 1) * ppf;
            if ppf == 3 {
                lower_bounds[last_idx] = (args.max_freq - 2000.0).max(5000.0).log10();
                upper_bounds[last_idx] = args.max_freq.log10();
                lower_bounds[last_idx + 1] = args.min_q;
                upper_bounds[last_idx + 1] = args.max_q;
                lower_bounds[last_idx + 2] = -args.max_db;
                upper_bounds[last_idx + 2] = args.max_db;
            }
        }
    }

    // Debug: Display bounds for each filter (unless in QA mode)
    if args.qa.is_none() {
        println!("\n📏 Parameter Bounds (Model: {}):", model);
        println!("+-# -|---Freq Range (Hz)---|----Q Range----|---Gain Range (dB)---|--Type--+");
        for i in 0..args.num_filters {
            let offset = i * ppf;
            let (freq_idx, q_idx, gain_idx) = if ppf == 3 {
                (offset, offset + 1, offset + 2)
            } else {
                (offset + 1, offset + 2, offset + 3)
            };
            let freq_low_hz = 10f64.powf(lower_bounds[freq_idx]);
            let freq_high_hz = 10f64.powf(upper_bounds[freq_idx]);
            let q_low = lower_bounds[q_idx];
            let q_high = upper_bounds[q_idx];
            let gain_low = lower_bounds[gain_idx];
            let gain_high = upper_bounds[gain_idx];

            let filter_type = match model {
                PeqModel::Pk => "PK",
                PeqModel::HpPk if i == 0 => "HP",
                PeqModel::HpPk => "PK",
                PeqModel::HpPkLp if i == 0 => "HP",
                PeqModel::HpPkLp if i == args.num_filters - 1 => "LP",
                PeqModel::HpPkLp => "PK",
                PeqModel::LsPk if i == 0 => "LS",
                PeqModel::LsPk => "PK",
                PeqModel::LsPkHs if i == 0 => "LS",
                PeqModel::LsPkHs if i == args.num_filters - 1 => "HS",
                PeqModel::LsPkHs => "PK",
                PeqModel::FreePkFree if i == 0 || i == args.num_filters - 1 => "??",
                PeqModel::FreePkFree => "PK",
                PeqModel::Free => "??",
            };

            println!(
                "| {:2} | {:7.1} - {:7.1} | {:5.2} - {:5.2} | {:+6.2} - {:+6.2} | {:6} |",
                i + 1,
                freq_low_hz,
                freq_high_hz,
                q_low,
                q_high,
                gain_low,
                gain_high,
                filter_type
            );
        }
        println!("+----|--------------------|---------------|---------------------|---------+\n");
    }

    (lower_bounds, upper_bounds)
}

/// Build an initial guess vector for each filter.
pub fn initial_guess(
    args: &crate::cli::Args,
    lower_bounds: &[f64],
    upper_bounds: &[f64],
) -> Vec<f64> {
    let model = args.effective_peq_model();
    let ppf = crate::param_utils::params_per_filter(model);
    let mut x = vec![];

    for i in 0..args.num_filters {
        let offset = i * ppf;

        match model {
            PeqModel::Pk
            | PeqModel::HpPk
            | PeqModel::HpPkLp
            | PeqModel::LsPk
            | PeqModel::LsPkHs => {
                // Fixed filter types: [freq, Q, gain]
                let freq = lower_bounds[offset].min(args.max_freq.log10());
                let q = (upper_bounds[offset + 1] * lower_bounds[offset + 1]).sqrt();
                let sign = if i % 2 == 0 { 0.5 } else { -0.5 };
                let gain = sign * upper_bounds[offset + 2].max(args.min_db);
                x.extend_from_slice(&[freq, q, gain]);
            }
            PeqModel::FreePkFree | PeqModel::Free => {
                // Free filter types: [type, freq, Q, gain]
                let filter_type = 0.0;
                let freq = lower_bounds[offset + 1].min(args.max_freq.log10());
                let q = (upper_bounds[offset + 2] * lower_bounds[offset + 2]).sqrt();
                let sign = if i % 2 == 0 { 0.5 } else { -0.5 };
                let gain = sign * upper_bounds[offset + 3].max(args.min_db);
                x.extend_from_slice(&[filter_type, freq, q, gain]);
            }
        }
    }
    x
}

/// Run global (and optional local refine) optimization and return the parameter vector.
pub fn perform_optimization(
    args: &crate::cli::Args,
    objective_data: &ObjectiveData,
) -> Result<Vec<f64>, Box<dyn Error>> {
    perform_optimization_with_callback(
        args,
        objective_data,
        Box::new(|_intermediate| crate::de::CallbackAction::Continue),
    )
}

/// Run optimization with a DE progress callback (only used for AutoEQ DE).
pub fn perform_optimization_with_callback(
    args: &crate::cli::Args,
    objective_data: &ObjectiveData,
    callback: Box<dyn FnMut(&crate::de::DEIntermediate) -> crate::de::CallbackAction + Send>,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let (lower_bounds, upper_bounds) = setup_bounds(args);
    let mut x = initial_guess(args, &lower_bounds, &upper_bounds);

    // Only AutoEQ algorithms currently support callbacks
    let result = optimize_filters_autoeq_with_callback(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &args.algo,
        args,
        callback,
    );

    match result {
        Ok((_status, _val)) => {}
        Err((e, _final_value)) => {
            return Err(std::io::Error::other(e).into());
        }
    };

    if args.refine {
        let local_result = optimize_filters_with_algo_override(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            objective_data.clone(),
            args,
            Some(&args.local_algo),
        );
        match local_result {
            Ok((_local_status, _local_val)) => {}
            Err((e, _final_value)) => {
                return Err(std::io::Error::other(e).into());
            }
        }
    }

    Ok(x)
}

/// Progress update sent to callback during optimization
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// Current iteration number
    pub iteration: usize,
    /// Total expected iterations (maxeval)
    pub max_iterations: usize,
    /// Current loss/objective value (lower is better)
    pub loss: f64,
    /// Optional score value (higher is better, e.g., Harman speaker score)
    /// Available when speaker_score_data was provided
    pub score: Option<f64>,
    /// Convergence metric (population standard deviation)
    pub convergence: f64,
    /// Raw optimizer parameters
    pub params: Vec<f64>,
    /// Decoded biquad filters (if include_biquads=true)
    pub biquads: Vec<Biquad>,
    /// Filter response at standard frequencies (if include_filter_response=true)
    pub filter_response: Vec<f64>,
}

/// Configuration for progress callbacks
#[derive(Debug, Clone)]
pub struct ProgressCallbackConfig {
    /// Report progress every N iterations (default: 25)
    pub interval: usize,
    /// Include decoded biquad filters in each update (default: true)
    pub include_biquads: bool,
    /// Include filter response curve in each update (default: true)
    pub include_filter_response: bool,
    /// Frequencies for filter response computation (if empty, uses standard 200-point grid)
    pub frequencies: Vec<f64>,
}

impl Default for ProgressCallbackConfig {
    fn default() -> Self {
        Self {
            interval: 25,
            include_biquads: true,
            include_filter_response: true,
            frequencies: Vec::new(), // Will use standard grid
        }
    }
}

/// Output from optimization with progress tracking
#[derive(Debug, Clone)]
pub struct OptimizationOutput {
    /// Raw filter parameters
    pub params: Vec<f64>,
    /// Optimization history: (iteration, loss)
    pub history: Vec<(usize, f64)>,
}

/// Run optimization with progress callback at configurable intervals
///
/// This wraps `perform_optimization_with_callback` with:
/// - Interval-based reporting (not every iteration)
/// - Automatic biquad decoding from raw params
/// - Filter response computation
/// - Score calculation when speaker_score_data is available
///
/// # Arguments
/// * `args` - Optimization arguments
/// * `objective_data` - Objective data from setup_objective_data
/// * `config` - Callback configuration (interval, what to include)
/// * `callback` - User callback receiving ProgressUpdate
///
/// # Returns
/// Optimization result with raw filter parameters and history
pub fn perform_optimization_with_progress<F>(
    args: &crate::cli::Args,
    objective_data: &ObjectiveData,
    config: ProgressCallbackConfig,
    mut callback: F,
) -> Result<OptimizationOutput, Box<dyn Error>>
where
    F: FnMut(&ProgressUpdate) -> crate::de::CallbackAction + Send + 'static,
{
    use std::sync::{Arc, Mutex};

    let frequencies: Vec<f64> = if config.frequencies.is_empty() {
        read::create_log_frequency_grid(200, 20.0, 20000.0)
            .iter()
            .copied()
            .collect()
    } else {
        config.frequencies.clone()
    };
    let freq_array = Array1::from(frequencies.clone());
    let speaker_score_data = objective_data.speaker_score_data.clone();
    let sample_rate = args.sample_rate;
    let peq_model = args.peq_model;
    let maxeval = args.maxeval;

    let last_reported = Arc::new(Mutex::new(0usize));
    let history = Arc::new(Mutex::new(Vec::new()));

    let last_reported_clone = Arc::clone(&last_reported);
    let history_clone = Arc::clone(&history);
    let freq_array_clone = freq_array.clone();
    let frequencies_clone = frequencies.clone();

    let de_callback = move |intermediate: &crate::de::DEIntermediate| -> crate::de::CallbackAction {
        // Always record history
        {
            let mut hist = history_clone.lock().unwrap();
            hist.push((intermediate.iter, intermediate.fun));
        }

        let mut last = last_reported_clone.lock().unwrap();

        // Check if we should report
        if intermediate.iter == 0 || intermediate.iter.saturating_sub(*last) >= config.interval {
            *last = intermediate.iter;

            // Decode biquads if requested
            let biquads: Vec<Biquad> = if config.include_biquads {
                x2peq(&intermediate.x.to_vec(), sample_rate, peq_model)
                    .into_iter()
                    .map(|(_, b)| b)
                    .collect()
            } else {
                Vec::new()
            };

            // Compute filter response if requested
            let filter_response: Vec<f64> = if config.include_filter_response && !biquads.is_empty()
            {
                frequencies_clone
                    .iter()
                    .map(|&f| biquads.iter().map(|b| b.log_result(f)).sum())
                    .collect()
            } else {
                Vec::new()
            };

            // Compute score if speaker_score_data available
            let score = speaker_score_data.as_ref().map(|sd| {
                let peq_response = if !filter_response.is_empty() {
                    Array1::from(filter_response.clone())
                } else {
                    let bs = x2peq(&intermediate.x.to_vec(), sample_rate, peq_model);
                    let resp: Vec<f64> = frequencies_clone
                        .iter()
                        .map(|&f| bs.iter().map(|(_, b)| b.log_result(f)).sum())
                        .collect();
                    Array1::from(resp)
                };
                crate::loss::speaker_score_loss(sd, &freq_array_clone, &peq_response)
            });

            let update = ProgressUpdate {
                iteration: intermediate.iter,
                max_iterations: maxeval,
                loss: intermediate.fun,
                score,
                convergence: intermediate.convergence,
                params: intermediate.x.to_vec(),
                biquads,
                filter_response,
            };

            callback(&update)
        } else {
            crate::de::CallbackAction::Continue
        }
    };

    let params = perform_optimization_with_callback(args, objective_data, Box::new(de_callback))?;

    let final_history = Arc::try_unwrap(history)
        .map(|m| m.into_inner().unwrap())
        .unwrap_or_default();

    Ok(OptimizationOutput {
        params,
        history: final_history,
    })
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
    let (objective_data, _) = setup_objective_data(
        args,
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
    };

    // 4. Setup objective
    let (objective_data, _) = setup_objective_data(
        args,
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
    min_db: f64,
    max_db: f64,
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
        population: 300,
        maxeval: max_iter,
        refine: false,
        local_algo: "cobyla".to_string(),
        min_spacing_oct: 0.0,
        spacing_weight: 0.0,
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
        seed: None,
        qa: None,
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
/// )?;
/// println!("Gains: {:?}", result.gains);
/// println!("Crossover freqs: {:?}", result.crossover_freqs);
/// ```
#[allow(clippy::too_many_arguments)]
pub fn optimize_drivers_crossover(
    drivers_data: crate::loss::DriversLossData,
    min_freq: f64,
    max_freq: f64,
    sample_rate: f64,
    algorithm: &str,
    max_iter: usize,
    min_db: f64,
    max_db: f64,
    fixed_freqs: Option<Vec<f64>>,
) -> Result<DriverOptimizationResult, Box<dyn std::error::Error>> {
    let n_drivers = drivers_data.drivers.len();

    // Create Args structure needed for optimization
    let args = create_driver_optimization_args(
        min_freq,
        max_freq,
        sample_rate,
        algorithm,
        max_iter,
        min_db,
        max_db,
    );

    // Setup objective data with optional fixed frequencies
    let objective_data = if let Some(ref freqs) = fixed_freqs {
        let mut data = setup_drivers_objective_data(&args, drivers_data.clone());
        data.fixed_crossover_freqs = Some(freqs.clone());
        data
    } else {
        setup_drivers_objective_data(&args, drivers_data.clone())
    };

    // Setup bounds (exclude crossover frequencies if fixed)
    let (lower_bounds, upper_bounds) = if fixed_freqs.is_some() {
        setup_drivers_bounds_fixed_freqs(&args, &drivers_data)
    } else {
        setup_drivers_bounds(&args, &drivers_data)
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
        &args,
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
            Ok((freq, spl, phase)) => {
                measurements.push(DriverMeasurement::new(freq, spl, phase));
                eprintln!("✓ Loaded driver {} from {}", i + 1, path.display());
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
    min_db: f64,
    max_db: f64,
) -> Result<DriverOptimizationResult, Box<dyn std::error::Error>> {
    let n_drivers = drivers_data.drivers.len();

    // Create Args
    let mut args = create_driver_optimization_args(
        min_freq,
        max_freq,
        sample_rate,
        algorithm,
        max_iter,
        min_db,
        max_db,
    );
    args.loss = crate::LossType::MultiSubFlat;

    // Setup objective data
    let objective_data = setup_multisub_objective_data(&args, drivers_data.clone());

    // Setup bounds (gains + delays)
    let (lower_bounds, upper_bounds) = setup_multisub_bounds(&args, n_drivers);

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
        &args,
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

/// Set up objective data for multi-subwoofer optimization.
///
/// Creates the objective function configuration for optimizing gain and delay
/// parameters across multiple subwoofers to achieve a flat combined response.
///
/// # Arguments
///
/// * `args` - CLI arguments with optimization parameters
/// * `drivers_data` - Multi-driver measurement and configuration data
pub fn setup_multisub_objective_data(
    args: &crate::cli::Args,
    drivers_data: DriversLossData,
) -> ObjectiveData {
    ObjectiveData {
        freqs: drivers_data.freq_grid.clone(),
        target: Array1::zeros(drivers_data.freq_grid.len()),
        deviation: Array1::zeros(drivers_data.freq_grid.len()),
        srate: args.sample_rate,
        min_spacing_oct: 0.0,
        spacing_weight: 0.0,
        max_db: args.max_db,
        min_db: args.min_db,
        min_freq: args.min_freq,
        max_freq: args.max_freq,
        peq_model: args.effective_peq_model(),
        loss_type: crate::LossType::MultiSubFlat,
        speaker_score_data: None,
        headphone_score_data: None,
        input_curve: None,
        drivers_data: Some(drivers_data),
        fixed_crossover_freqs: None,
        penalty_w_ceiling: 0.0,
        penalty_w_spacing: 0.0,
        penalty_w_mingain: 0.0,
        integrality: None,
    }
}

/// Set up parameter bounds for multi-subwoofer optimization.
///
/// Creates lower and upper bounds for gain and delay parameters.
/// Gains are bounded by `[-max_db, max_db]` and delays by `[0, 20]` ms.
///
/// # Arguments
///
/// * `args` - CLI arguments with `max_db` setting
/// * `n_drivers` - Number of subwoofers
///
/// # Returns
///
/// Tuple of (lower_bounds, upper_bounds) vectors.
pub fn setup_multisub_bounds(args: &crate::cli::Args, n_drivers: usize) -> (Vec<f64>, Vec<f64>) {
    let n_params = n_drivers * 2; // gains + delays
    let mut lower_bounds = Vec::with_capacity(n_params);
    let mut upper_bounds = Vec::with_capacity(n_params);

    // Gains
    for _ in 0..n_drivers {
        lower_bounds.push(-args.max_db);
        upper_bounds.push(args.max_db);
    }

    // Delays (0 to 20ms)
    for _ in 0..n_drivers {
        lower_bounds.push(0.0);
        upper_bounds.push(20.0);
    }

    (lower_bounds, upper_bounds)
}

/// Generate initial guess for multi-subwoofer optimization.
///
/// Returns a vector of zeros for all gain and delay parameters,
/// representing no gain adjustment and no delay for each driver.
///
/// # Arguments
///
/// * `n_drivers` - Number of subwoofers
///
/// # Returns
///
/// Vector of `n_drivers * 2` zeros (gains followed by delays).
pub fn multisub_initial_guess(n_drivers: usize) -> Vec<f64> {
    vec![0.0; n_drivers * 2]
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
        };
        let deviation = Curve {
            freq: input_curve.freq.clone(),
            spl: Array1::zeros(input_curve.freq.len()),
            phase: None,
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
        let (obj, use_cea) =
            super::setup_objective_data(&args, &input_curve, &target, &deviation, &spin_opt)
                .expect("setup_objective_data should succeed with valid spin data");
        assert!(use_cea);
        assert!(obj.speaker_score_data.is_some());

        // Case 2: Even if measurement is "On Axis", if spin_data is available,
        // we can still compute speaker score (the measurement just determines
        // which curve is being optimized, not what loss functions are available)
        let mut args2 = args.clone();
        args2.measurement = Some("On Axis".to_string());
        let (obj2, use_cea2) =
            super::setup_objective_data(&args2, &input_curve, &target, &deviation, &spin_opt)
                .expect("setup_objective_data should succeed with valid spin data");
        assert!(use_cea2); // Changed: spin_data available means speaker score is possible
        assert!(obj2.speaker_score_data.is_some());

        // Case 3: If spin_data is missing -> use_cea must be false
        let (obj3, use_cea3) =
            super::setup_objective_data(&args, &input_curve, &target, &deviation, &None)
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
        };
        let target_curve = Curve {
            freq: Array1::from(frequencies.clone()),
            spl: Array1::from(vec![80.0, 80.0, 80.0]),
            phase: None,
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
        };
        let target_curve = Curve {
            freq: Array1::from(frequencies.clone()),
            spl: Array1::from(vec![80.0, 80.0, 80.0]),
            phase: None,
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
}
