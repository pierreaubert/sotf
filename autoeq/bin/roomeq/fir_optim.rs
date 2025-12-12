//! FIR filter optimization for room correction

use autoeq::Curve;
use autoeq::fir::{FirPhase, generate_fir_from_response};
use ndarray::Array1;
use std::error::Error;

use super::types::{OptimizerConfig, TargetCurveConfig};

/// Generate an FIR correction filter for a single channel
///
/// # Arguments
/// * `measurement` - Measured frequency response
/// * `config` - Optimizer configuration
/// * `target_config` - Optional target curve configuration
/// * `sample_rate` - Sample rate
///
/// # Returns
/// * Vector of FIR coefficients
pub fn generate_fir_correction(
    measurement: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
) -> Result<Vec<f64>, Box<dyn Error>> {
    // 1. Determine Target Curve
    let target_curve = match target_config {
        Some(TargetCurveConfig::Path(path)) => {
            let target = autoeq::read::read_curve_from_csv(path)?;
            autoeq::read::normalize_and_interpolate_response(&measurement.freq, &target)
        }
        Some(TargetCurveConfig::Predefined(name)) => {
            // Using dummy args for now, similar to eq_optim
            use autoeq::cli::Args;
            use clap::Parser;
            let dummy_args = Args::parse_from(["autoeq", "--curve-name", name]);
            autoeq::workflow::build_target_curve(&dummy_args, &measurement.freq, measurement)
        }
        None => {
            // Flat target
            Curve {
                freq: measurement.freq.clone(),
                spl: Array1::zeros(measurement.freq.len()),
                phase: None,
            }
        }
    };

    // 2. Compute Correction Curve (Target - Measurement)
    // We want the filter H such that H * Measurement = Target
    // In dB: H_db = Target_db - Measurement_db
    let correction_spl = &target_curve.spl - &measurement.spl;
    
    // Create correction curve object
    // Note: We ignore measurement phase here and assume minimum phase correction 
    // or linear phase correction based on magnitude only.
    // If we wanted to correct excess phase, we'd need complex division.
    // For now, magnitude-based FIR generation is standard.
    let correction_curve = Curve {
        freq: measurement.freq.clone(),
        spl: correction_spl,
        phase: None,
    };

    // 3. Get FIR settings
    let fir_config = config.fir.as_ref().ok_or("FIR configuration missing")?;
    let n_taps = fir_config.taps;
    let phase_type = match fir_config.phase.to_lowercase().as_str() {
        "linear" => FirPhase::Linear,
        "minimum" => FirPhase::Minimum,
        _ => return Err(format!("Unknown FIR phase type: {}", fir_config.phase).into()),
    };

    // 4. Generate FIR
    let coeffs = generate_fir_from_response(&correction_curve, sample_rate, n_taps, phase_type);

    Ok(coeffs)
}
