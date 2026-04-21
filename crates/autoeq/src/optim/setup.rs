//! Shared optimization setup helpers
//!
//! Functions for preparing objective data, computing parameter bounds, building
//! initial guesses, and orchestrating optimization runs. These are used by both
//! the CLI workflows in [`crate::workflow`] and by the room EQ subsystem.

use crate::AutoeqError;
use crate::Curve;
use crate::HeadphoneLossData;
use crate::PeqModel;
use crate::SpeakerLossData;
use crate::iir::Biquad;
use crate::loss::DriversLossData;
use super::{ObjectiveData, optimize_filters_with_algo_override};
use super::de::optimize_filters_autoeq_with_callback;
use crate::read;
use crate::x2peq;
use ndarray::Array1;
use std::collections::HashMap;
use std::error::Error;

/// Prepare the ObjectiveData and whether CEA2034-based scoring is active.
///
/// # Errors
///
/// Returns `AutoeqError::MissingCea2034Curve` if spin_data is provided but missing required curves.
/// Returns `AutoeqError::CurveLengthMismatch` if spin_data curves have inconsistent lengths.
pub fn setup_objective_data(
    params: &crate::OptimParams,
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
        Some(HeadphoneLossData::new(params.smooth, params.smooth_n))
    } else {
        None
    };

    let objective_data = ObjectiveData {
        freqs: input_curve.freq.clone(),
        target: target_curve.spl.clone(),
        deviation: deviation_curve.spl.clone(), // This is the deviation to be corrected
        srate: params.sample_rate,
        min_spacing_oct: params.min_spacing_oct,
        spacing_weight: params.spacing_weight,
        max_db: params.max_db,
        min_db: params.min_db,
        min_freq: params.min_freq,
        max_freq: params.max_freq,
        peq_model: params.peq_model,
        loss_type: params.loss,
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
        multi_objective: None,
        smooth: params.smooth,
        smooth_n: params.smooth_n,
        max_boost_envelope: None,
        min_cut_envelope: None,
        epa_config: None,
        detected_problems: Vec::new(),
        null_suppression: None,
    };

    Ok((objective_data, use_cea))
}

/// Set up objective data for multi-driver crossover optimization
///
/// # Arguments
/// * `params` - Optimization parameters
/// * `drivers_data` - Multi-driver measurement data
///
/// # Returns
/// * ObjectiveData configured for multi-driver optimization
pub fn setup_drivers_objective_data(
    params: &crate::OptimParams,
    drivers_data: DriversLossData,
) -> ObjectiveData {
    ObjectiveData {
        freqs: drivers_data.freq_grid.clone(),
        target: Array1::zeros(drivers_data.freq_grid.len()),
        deviation: Array1::zeros(drivers_data.freq_grid.len()),
        srate: params.sample_rate,
        min_spacing_oct: 0.0, // Not applicable for crossover optimization
        spacing_weight: 0.0,
        max_db: params.max_db,
        min_db: params.min_db,
        min_freq: params.min_freq,
        max_freq: params.max_freq,
        peq_model: params.peq_model,
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
        multi_objective: None,
        smooth: false, // Not applicable for crossover optimization
        smooth_n: 1,
        max_boost_envelope: None,
        min_cut_envelope: None,
        epa_config: None,
        detected_problems: Vec::new(),
        null_suppression: None,
    }
}

/// Build optimization parameter bounds for multi-driver crossover optimization
///
/// # Arguments
/// * `params` - Optimization parameters
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
    params: &crate::OptimParams,
    drivers_data: &DriversLossData,
) -> (Vec<f64>, Vec<f64>) {
    let n_drivers = drivers_data.drivers.len();
    let n_params = n_drivers * 2 + (n_drivers - 1); // N gains + N delays + (N-1) crossovers

    let mut lower_bounds = Vec::with_capacity(n_params);
    let mut upper_bounds = Vec::with_capacity(n_params);

    // Bounds for gains: [-max_db, max_db]
    for _ in 0..n_drivers {
        lower_bounds.push(-params.max_db);
        upper_bounds.push(params.max_db);
    }

    // Bounds for delays: [-20.0, 20.0] ms
    for _ in 0..n_drivers {
        lower_bounds.push(-20.0);
        upper_bounds.push(20.0);
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
        let xover_min = (geometric_center * 0.5).max(params.min_freq).log10();
        let xover_max = (geometric_center * 2.0).min(params.max_freq).log10();

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
/// * `params` - Optimization parameters
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
    params: &crate::OptimParams,
    drivers_data: &DriversLossData,
) -> (Vec<f64>, Vec<f64>) {
    let n_drivers = drivers_data.drivers.len();
    let n_params = n_drivers * 2; // N gains + N delays (no crossovers)

    let mut lower_bounds = Vec::with_capacity(n_params);
    let mut upper_bounds = Vec::with_capacity(n_params);

    // Bounds for gains: [-max_db, max_db]
    for _ in 0..n_drivers {
        lower_bounds.push(-params.max_db);
        upper_bounds.push(params.max_db);
    }

    // Bounds for delays: [-20.0, 20.0] ms
    for _ in 0..n_drivers {
        lower_bounds.push(-20.0);
        upper_bounds.push(20.0);
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

/// Clamp the per-filter gain **upper** bound to 0 dB for any filter
/// whose maximum allowed center frequency is at or below the Schroeder
/// frequency.
///
/// Below Schroeder the room is modal: peaks caused by constructive
/// interference between the direct wave and its reflections *can* be
/// cut by EQ (reducing the input at f directly reduces the SPL at f),
/// but nulls caused by destructive interference *cannot* be filled by
/// EQ boost — the cancellation happens after the EQ, so adding more
/// input energy just raises the direct wave and its anti-phase
/// reflection by the same ratio and the null stays. Worse, the boost
/// burns amplifier headroom and excites woofer excursion for no
/// audible benefit.
///
/// Letting the DE optimizer place boost filters anywhere in the modal
/// region therefore wastes filter slots on physically impossible
/// corrections. This function enforces "below Schroeder is cuts-only"
/// as a hard constraint on the optimizer's parameter bounds.
///
/// Filters whose allowed frequency range *straddles* Schroeder (i.e.
/// their upper frequency bound is above `schroeder_hz`) keep their
/// original symmetric bounds — those filters can still be positioned
/// in the above-Schroeder part of their range where boosts are
/// physically meaningful.
pub fn restrict_boost_above_schroeder(
    upper_bounds: &mut [f64],
    params: &crate::OptimParams,
    schroeder_hz: f64,
) {
    use crate::cli::PeqModel;
    if schroeder_hz <= 0.0 {
        return;
    }
    let model = params.peq_model;
    let ppf = crate::param_utils::params_per_filter(model);
    let log_schroeder = schroeder_hz.log10();

    for i in 0..params.num_filters {
        let offset = i * ppf;
        // Parameter indices per filter depend on the model. Only these
        // two layouts exist today — `setup_bounds` uses the same
        // match and the indices here must stay in sync with it.
        let (freq_idx, gain_idx) = match model {
            PeqModel::Pk
            | PeqModel::HpPk
            | PeqModel::HpPkLp
            | PeqModel::LsPk
            | PeqModel::LsPkHs => (offset, offset + 2),
            PeqModel::FreePkFree | PeqModel::Free => (offset + 1, offset + 3),
        };
        if freq_idx >= upper_bounds.len() || gain_idx >= upper_bounds.len() {
            continue;
        }
        // If the filter's highest possible center frequency is at or
        // below Schroeder, every placement of this filter lives inside
        // the modal region — constrain it to cuts only.
        if upper_bounds[freq_idx] <= log_schroeder && upper_bounds[gain_idx] > 0.0 {
            upper_bounds[gain_idx] = 0.0;
        }
    }
}

/// Build optimization parameter bounds for the optimizer.
pub fn setup_bounds(params: &crate::OptimParams) -> (Vec<f64>, Vec<f64>) {
    use crate::cli::PeqModel;

    let model = params.peq_model;
    let ppf = crate::param_utils::params_per_filter(model);
    let num_params = params.num_filters * ppf;
    let mut lower_bounds = Vec::with_capacity(num_params);
    let mut upper_bounds = Vec::with_capacity(num_params);

    let spacing = 1.0; // Overlap factor - allows adjacent filters to overlap
    let gain_lower = -3.0 * params.max_db;
    let q_lower = params.min_q.max(0.1);
    let range = (params.max_freq.log10() - params.min_freq.log10()) / (params.num_filters as f64);

    for i in 0..params.num_filters {
        // Center frequency for this filter in log space
        let f_center = params.min_freq.log10() + (i as f64) * range;

        // Calculate bounds with overlap
        // Each filter can range from (center - spacing*range) to (center + spacing*range)
        let f_low = (f_center - spacing * range).max(params.min_freq.log10());
        let f_high = (f_center + spacing * range).min(params.max_freq.log10());

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

        // Ensure lower bound never exceeds upper bound (can happen when
        // progressive adjustment pushes f_low past f_high with many filters
        // in a narrow range).
        let f_high_adjusted = f_high_adjusted.max(f_low_adjusted);

        // Add bounds based on model type
        match model {
            PeqModel::Pk
            | PeqModel::HpPk
            | PeqModel::HpPkLp
            | PeqModel::LsPk
            | PeqModel::LsPkHs => {
                // Fixed filter types: [freq, Q, gain]
                lower_bounds.extend_from_slice(&[f_low_adjusted, q_lower, gain_lower]);
                upper_bounds.extend_from_slice(&[f_high_adjusted, params.max_q, params.max_db]);
            }
            PeqModel::FreePkFree | PeqModel::Free => {
                // Free filter types: [type, freq, Q, gain]
                let (type_low, type_high) = if model == PeqModel::Free
                    || (model == PeqModel::FreePkFree && (i == 0 || i == params.num_filters - 1))
                {
                    crate::param_utils::filter_type_bounds()
                } else {
                    (0.0, 0.999) // Peak filter only
                };
                lower_bounds.extend_from_slice(&[type_low, f_low_adjusted, q_lower, gain_lower]);
                upper_bounds.extend_from_slice(&[
                    type_high,
                    f_high_adjusted,
                    params.max_q,
                    params.max_db,
                ]);
            }
        }
    }

    // Apply model-specific constraints
    match model {
        PeqModel::HpPk | PeqModel::HpPkLp => {
            // First filter is highpass - fixed 3-param layout
            lower_bounds[0] = 20.0_f64.max(params.min_freq).log10();
            upper_bounds[0] = 120.0_f64.min(params.min_freq + 20.0).log10();
            lower_bounds[1] = 1.0;
            upper_bounds[1] = 1.5; // could be tuned as a function of max_db
            lower_bounds[2] = 0.0;
            upper_bounds[2] = 0.0;
        }
        PeqModel::LsPk | PeqModel::LsPkHs => {
            // First filter is low shelves - fixed 3-param layout
            lower_bounds[0] = 20.0_f64.max(params.min_freq).log10();
            upper_bounds[0] = 120.0_f64.min(params.min_freq + 20.0).log10();
            lower_bounds[1] = params.min_q;
            upper_bounds[1] = params.max_q;
            lower_bounds[2] = -params.max_db;
            upper_bounds[2] = params.max_db;
        }
        _ => {}
    }

    if params.num_filters > 1 {
        if matches!(model, PeqModel::HpPkLp) {
            // Last filter is lowpass - fixed 3-param layout
            let last_idx = (params.num_filters - 1) * ppf;
            if ppf == 3 {
                lower_bounds[last_idx] = (params.max_freq - 2000.0).max(5000.0).log10();
                upper_bounds[last_idx] = params.max_freq.log10();
                lower_bounds[last_idx + 1] = 1.0;
                upper_bounds[last_idx + 1] = 1.5;
                lower_bounds[last_idx + 2] = 0.0;
                upper_bounds[last_idx + 2] = 0.0;
            }
        }

        if matches!(model, PeqModel::LsPkHs) {
            // Last filter is lowpass - fixed 3-param layout
            let last_idx = (params.num_filters - 1) * ppf;
            if ppf == 3 {
                lower_bounds[last_idx] = (params.max_freq - 2000.0).max(5000.0).log10();
                upper_bounds[last_idx] = params.max_freq.log10();
                lower_bounds[last_idx + 1] = params.min_q;
                upper_bounds[last_idx + 1] = params.max_q;
                lower_bounds[last_idx + 2] = -params.max_db;
                upper_bounds[last_idx + 2] = params.max_db;
            }
        }
    }

    // Debug: Display bounds for each filter (unless in QA mode)
    if !params.quiet {
        log::info!("\n📏 Parameter Bounds (Model: {}):", model);
        log::info!("+----+-------------------+---------------+-----------------+--------+");
        log::info!("|  # | Freq Range (Hz)   | Q Range       | Gain Range (dB) | Type   |");
        log::info!("+----+-------------------+---------------+-----------------+--------+");
        for i in 0..params.num_filters {
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
                PeqModel::HpPkLp if i == params.num_filters - 1 => "LP",
                PeqModel::HpPkLp => "PK",
                PeqModel::LsPk if i == 0 => "LS",
                PeqModel::LsPk => "PK",
                PeqModel::LsPkHs if i == 0 => "LS",
                PeqModel::LsPkHs if i == params.num_filters - 1 => "HS",
                PeqModel::LsPkHs => "PK",
                PeqModel::FreePkFree if i == 0 || i == params.num_filters - 1 => "??",
                PeqModel::FreePkFree => "PK",
                PeqModel::Free => "??",
            };

            log::info!(
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
        log::info!("+----+-------------------+---------------+-----------------+--------+\n");
    }

    (lower_bounds, upper_bounds)
}

/// Build an initial guess vector for each filter.
pub fn initial_guess(
    params: &crate::OptimParams,
    lower_bounds: &[f64],
    upper_bounds: &[f64],
) -> Vec<f64> {
    let model = params.peq_model;
    let ppf = crate::param_utils::params_per_filter(model);
    let mut x = vec![];

    for i in 0..params.num_filters {
        let offset = i * ppf;

        match model {
            PeqModel::Pk
            | PeqModel::HpPk
            | PeqModel::HpPkLp
            | PeqModel::LsPk
            | PeqModel::LsPkHs => {
                // Fixed filter types: [freq, Q, gain]
                let freq = lower_bounds[offset]
                    .min(params.max_freq.log10())
                    .clamp(lower_bounds[offset], upper_bounds[offset]);
                let q = (upper_bounds[offset + 1] * lower_bounds[offset + 1])
                    .sqrt()
                    .clamp(lower_bounds[offset + 1], upper_bounds[offset + 1]);
                let sign = if i % 2 == 0 { 0.5 } else { -0.5 };
                let gain = (sign * upper_bounds[offset + 2].max(params.min_db))
                    .clamp(lower_bounds[offset + 2], upper_bounds[offset + 2]);
                x.extend_from_slice(&[freq, q, gain]);
            }
            PeqModel::FreePkFree | PeqModel::Free => {
                // Free filter types: [type, freq, Q, gain]
                let filter_type = 0.0_f64.clamp(lower_bounds[offset], upper_bounds[offset]);
                let freq = lower_bounds[offset + 1]
                    .min(params.max_freq.log10())
                    .clamp(lower_bounds[offset + 1], upper_bounds[offset + 1]);
                let q = (upper_bounds[offset + 2] * lower_bounds[offset + 2])
                    .sqrt()
                    .clamp(lower_bounds[offset + 2], upper_bounds[offset + 2]);
                let sign = if i % 2 == 0 { 0.5 } else { -0.5 };
                let gain = (sign * upper_bounds[offset + 3].max(params.min_db))
                    .clamp(lower_bounds[offset + 3], upper_bounds[offset + 3]);
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
    // TODO: Change signature to accept &OptimParams directly once
    // callers in workflow.rs (optimize_speaker, optimize_headphone) are updated.
    let params = crate::OptimParams::from(args);
    let (lower_bounds, upper_bounds) = setup_bounds(&params);
    let mut x = initial_guess(&params, &lower_bounds, &upper_bounds);

    // Only AutoEQ algorithms currently support callbacks
    let result = optimize_filters_autoeq_with_callback(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &params.algo,
        &params,
        callback,
    );

    match result {
        Ok((_status, _val)) => {}
        Err((e, _final_value)) => {
            return Err(std::io::Error::other(e).into());
        }
    };

    if params.refine {
        let local_result = optimize_filters_with_algo_override(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            objective_data.clone(),
            &params,
            Some(&params.local_algo),
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
/// * `args` - CLI arguments (will be converted to OptimParams internally)
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

/// Set up objective data for multi-subwoofer optimization.
///
/// Creates the objective function configuration for optimizing gain and delay
/// parameters across multiple subwoofers to achieve a flat combined response.
///
/// # Arguments
///
/// * `params` - Optimization parameters with optimization parameters
/// * `drivers_data` - Multi-driver measurement and configuration data
pub fn setup_multisub_objective_data(
    params: &crate::OptimParams,
    drivers_data: DriversLossData,
) -> ObjectiveData {
    ObjectiveData {
        freqs: drivers_data.freq_grid.clone(),
        target: Array1::zeros(drivers_data.freq_grid.len()),
        deviation: Array1::zeros(drivers_data.freq_grid.len()),
        srate: params.sample_rate,
        min_spacing_oct: 0.0,
        spacing_weight: 0.0,
        max_db: params.max_db,
        min_db: params.min_db,
        min_freq: params.min_freq,
        max_freq: params.max_freq,
        peq_model: params.peq_model,
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
        multi_objective: None,
        smooth: false, // Not applicable for multi-sub optimization
        smooth_n: 1,
        max_boost_envelope: None,
        min_cut_envelope: None,
        epa_config: None,
        detected_problems: Vec::new(),
        null_suppression: None,
    }
}

/// Set up parameter bounds for multi-subwoofer optimization.
///
/// Creates lower and upper bounds for gain and delay parameters.
/// Gains are bounded by `[-max_db, max_db]` and delays by `[0, 20]` ms.
///
/// # Arguments
///
/// * `params` - Optimization parameters with `max_db` setting
/// * `n_drivers` - Number of subwoofers
///
/// # Returns
///
/// Tuple of (lower_bounds, upper_bounds) vectors.
pub fn setup_multisub_bounds(params: &crate::OptimParams, n_drivers: usize) -> (Vec<f64>, Vec<f64>) {
    let n_params = n_drivers * 2; // gains + delays
    let mut lower_bounds = Vec::with_capacity(n_params);
    let mut upper_bounds = Vec::with_capacity(n_params);

    // Gains
    for _ in 0..n_drivers {
        lower_bounds.push(-params.max_db);
        upper_bounds.push(params.max_db);
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
