//! EQ optimization for individual channels
//!
//! Provides per-channel PEQ optimization with intelligent initialization
//! based on peak detection in the deviation curve.

use crate::cli::{Args, PeqModel};
use crate::loss::LossType;
use crate::workflow::setup_objective_data;
use crate::Curve;
use clap::{Parser, ValueEnum};
use log::debug;
use math_audio_iir_fir::Biquad;
use ndarray::Array1;
use std::error::Error;

use super::types::{OptimizerConfig, TargetCurveConfig};

/// Configuration for peak-based initial guess seeding
#[derive(Debug, Clone)]
pub struct PeakSeedingConfig {
    /// Enable peak-based seeding
    pub enabled: bool,
    /// Minimum peak height to consider (dB)
    pub min_peak_height_db: f64,
    /// Minimum distance between peaks (octaves)
    pub min_peak_distance_oct: f64,
    /// Smoothing window for peak detection (1/N octave)
    pub smoothing_octave: f64,
}

impl Default for PeakSeedingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_peak_height_db: 1.5,
            min_peak_distance_oct: 0.5,
            smoothing_octave: 1.0 / 6.0,
        }
    }
}

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
                    debug!("  Target '{}' not a predefined curve, trying as file path...", name);
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

/// Optimize EQ filters with peak-based initial guess seeding.
///
/// This variant uses intelligent initialization by detecting peaks/dips
/// in the deviation curve and seeding filters at those frequencies.
pub fn optimize_channel_eq_with_seeding(
    curve: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
    seeding_config: PeakSeedingConfig,
) -> Result<(Vec<Biquad>, f64), Box<dyn Error>> {
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

    if config.psychoacoustic {
        log::info!("  Applying psychoacoustic smoothing (1/48 oct < 100 Hz, 1/6 oct > 1 kHz)");
        let smoothing_config = crate::read::PsychoacousticSmoothingConfig::default();
        normalized_curve = crate::read::smooth_psychoacoustic(&normalized_curve, &smoothing_config);
    }

    let peq_model = PeqModel::from_str(&config.peq_model, true)
        .map_err(|e| format!("Invalid PEQ model '{}': {}", config.peq_model, e))?;

    let target_curve = match target_config {
        Some(TargetCurveConfig::Path(path)) => {
            let target = crate::read::read_curve_from_csv(path)?;
            crate::read::normalize_and_interpolate_response(&normalized_curve.freq, &target)
        }
        Some(TargetCurveConfig::Predefined(name)) => {
            let dummy_args = Args::parse_from(["autoeq", "--curve-name", name]);
            crate::workflow::build_target_curve(
                &dummy_args,
                &normalized_curve.freq,
                &normalized_curve,
            )?
        }
        None => Curve {
            freq: normalized_curve.freq.clone(),
            spl: Array1::zeros(normalized_curve.freq.len()),
            phase: None,
        },
    };

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

    let args = Args {
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
        seed: config.seed,
        qa: None,
    };

    let deviation_curve = Curve {
        freq: normalized_curve.freq.clone(),
        spl: &target_curve.spl - &normalized_curve.spl,
        phase: None,
    };

    let (objective_data, _use_cea) = setup_objective_data(
        &args,
        &normalized_curve,
        &target_curve,
        &deviation_curve,
        &None,
    )
    .expect("setup_objective_data should not fail without spin data");

    let (lower_bounds, upper_bounds) = crate::workflow::setup_bounds(&args);

    // Generate initial guess with peak-based seeding
    let mut x = if seeding_config.enabled {
        generate_peak_based_guess(
            &deviation_curve,
            config.num_filters,
            &lower_bounds,
            &upper_bounds,
            effective_min_freq,
            effective_max_freq,
            &seeding_config,
            peq_model,
        )
    } else {
        crate::workflow::initial_guess(&args, &lower_bounds, &upper_bounds)
    };

    let opt_result = crate::optim::optimize_filters(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &args,
    );

    let (_converged_msg, final_loss) = match opt_result {
        Ok((msg, loss)) => (msg, loss),
        Err((msg, loss)) => {
            eprintln!("  Warning: optimization did not fully converge: {}", msg);
            (msg, loss)
        }
    };

    let peq = crate::x2peq::x2peq(&x, sample_rate, args.peq_model);
    let filters: Vec<Biquad> = peq.into_iter().map(|(_weight, biquad)| biquad).collect();

    eprintln!(
        "  EQ optimization: {} filters, final loss={:.6}",
        filters.len(),
        final_loss
    );

    Ok((filters, final_loss))
}

/// Generate initial guess based on peaks in the deviation curve.
///
/// Detects the most prominent peaks and dips in the deviation curve
/// and seeds filters at those frequencies with appropriate gains.
fn generate_peak_based_guess(
    deviation_curve: &Curve,
    num_filters: usize,
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    min_freq: f64,
    max_freq: f64,
    config: &PeakSeedingConfig,
    peq_model: PeqModel,
) -> Vec<f64> {
    let params_per_filter = crate::param_utils::params_per_filter(peq_model);

    // Find peaks in deviation curve
    let peaks = detect_deviation_peaks(
        &deviation_curve.freq,
        &deviation_curve.spl,
        min_freq,
        max_freq,
        config.min_peak_height_db,
        config.min_peak_distance_oct,
    );

    debug!("  Peak seeding: found {} peaks/dips", peaks.len());

    // Build initial guess from detected peaks
    let mut x = Vec::with_capacity(num_filters * params_per_filter);

    for i in 0..num_filters {
        let (freq, gain, q) = if i < peaks.len() {
            // Use detected peak
            peaks[i]
        } else {
            // Fill remaining with log-spaced frequencies
            let t = i as f64 / num_filters.max(1) as f64;
            let log_min = min_freq.log10();
            let log_max = max_freq.log10();
            let freq = 10.0_f64.powf(log_min + t * (log_max - log_min));
            (freq, 0.0, 1.0)
        };

        match peq_model {
            PeqModel::Pk
            | PeqModel::HpPk
            | PeqModel::HpPkLp
            | PeqModel::LsPk
            | PeqModel::LsPkHs => {
                let base_idx = i * 3;
                let log_freq = freq
                    .log10()
                    .max(lower_bounds[base_idx])
                    .min(upper_bounds[base_idx]);
                let q_clamped = q
                    .max(lower_bounds[base_idx + 1])
                    .min(upper_bounds[base_idx + 1]);
                let gain_clamped = gain
                    .max(lower_bounds[base_idx + 2])
                    .min(upper_bounds[base_idx + 2]);
                x.extend_from_slice(&[log_freq, q_clamped, gain_clamped]);
            }
            PeqModel::FreePkFree | PeqModel::Free => {
                let base_idx = i * 4;
                let filter_type = 0.0; // Peak filter
                let log_freq = freq
                    .log10()
                    .max(lower_bounds[base_idx + 1])
                    .min(upper_bounds[base_idx + 1]);
                let q_clamped = q
                    .max(lower_bounds[base_idx + 2])
                    .min(upper_bounds[base_idx + 2]);
                let gain_clamped = gain
                    .max(lower_bounds[base_idx + 3])
                    .min(upper_bounds[base_idx + 3]);
                x.extend_from_slice(&[filter_type, log_freq, q_clamped, gain_clamped]);
            }
        }
    }

    x
}

/// Detected peak/dip information: (frequency, gain_needed, q_estimate)
type PeakInfo = (f64, f64, f64);

/// Detect peaks and dips in a deviation curve.
///
/// Returns a sorted list of (frequency, gain, q) tuples where:
/// - frequency: Hz of the peak/dip
/// - gain: positive for boost (dip), negative for cut (peak)
/// - q: estimated Q factor based on peak width
fn detect_deviation_peaks(
    freq: &Array1<f64>,
    spl: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
    min_height: f64,
    min_distance_oct: f64,
) -> Vec<PeakInfo> {
    let mut peaks = Vec::new();
    let n = freq.len();

    if n < 5 {
        return peaks;
    }

    // Find range indices
    let start_idx = freq.iter().position(|&f| f >= min_freq).unwrap_or(0);
    let end_idx = freq.iter().rposition(|&f| f <= max_freq).unwrap_or(n - 1);

    if end_idx <= start_idx + 2 {
        return peaks;
    }

    // Detect local maxima (peaks in deviation = need cut)
    for i in start_idx + 1..end_idx {
        if spl[i] > spl[i - 1] && spl[i] > spl[i + 1] && spl[i] >= min_height {
            let peak_freq = freq[i];
            let peak_gain = -spl[i]; // Negative: cut the peak

            // Estimate Q from peak width
            let q = estimate_peak_q(freq, spl, i);

            // Check minimum distance from previous peaks
            let min_dist_ok = peaks.iter().all(|(f, _, _)| {
                let octaves = (peak_freq / f).abs().log2().abs();
                octaves >= min_distance_oct
            });

            if min_dist_ok {
                peaks.push((peak_freq, peak_gain, q));
            }
        }
    }

    // Detect local minima (dips in deviation = need boost)
    for i in start_idx + 1..end_idx {
        if spl[i] < spl[i - 1] && spl[i] < spl[i + 1] && spl[i] <= -min_height {
            let dip_freq = freq[i];
            let dip_gain = -spl[i]; // Positive: boost the dip

            let q = estimate_peak_q(freq, spl, i);

            let min_dist_ok = peaks.iter().all(|(f, _, _)| {
                let octaves = (dip_freq / f).abs().log2().abs();
                octaves >= min_distance_oct
            });

            if min_dist_ok {
                peaks.push((dip_freq, dip_gain, q));
            }
        }
    }

    // Sort by absolute gain (most significant first)
    peaks.sort_by(|a, b| {
        b.1.abs()
            .partial_cmp(&a.1.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    peaks
}

/// Estimate Q factor from peak width at -3dB points.
fn estimate_peak_q(freq: &Array1<f64>, spl: &Array1<f64>, peak_idx: usize) -> f64 {
    if peak_idx == 0 || peak_idx >= freq.len() - 1 {
        return 1.0;
    }

    let peak_spl = spl[peak_idx];
    let threshold = peak_spl - 3.0;

    // Find -3dB points on each side
    let mut left_idx = peak_idx;
    while left_idx > 0 && spl[left_idx] > threshold {
        left_idx -= 1;
    }

    let mut right_idx = peak_idx;
    while right_idx < freq.len() - 1 && spl[right_idx] > threshold {
        right_idx += 1;
    }

    let f1 = freq[left_idx];
    let f2 = freq[right_idx];
    let f0 = freq[peak_idx];

    // Q = f0 / (f2 - f1)
    let bandwidth = f2 - f1;
    if bandwidth > 0.0 {
        (f0 / bandwidth).clamp(0.5, 10.0)
    } else {
        1.0
    }
}
