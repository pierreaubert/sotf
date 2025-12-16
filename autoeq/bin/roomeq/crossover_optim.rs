//! Crossover optimization for multi-driver groups

use autoeq::Curve;
use autoeq::loss::{CrossoverType, DriverMeasurement, DriversLossData};
use std::error::Error;

/// Parse crossover type from string
pub fn parse_crossover_type(type_str: &str) -> Result<CrossoverType, Box<dyn Error>> {
    match type_str.to_lowercase().as_str() {
        "butterworth2" | "bw2" | "butterworth12" => Ok(CrossoverType::Butterworth2),
        "lr2" | "linkwitzriley2" | "linkwitzriley12" => Ok(CrossoverType::LinkwitzRiley2),
        "lr4" | "lr24" | "linkwitzriley4" | "linkwitzriley24" => Ok(CrossoverType::LinkwitzRiley4),
        _ => Err(format!("Unknown crossover type: {}", type_str).into()),
    }
}

/// Convert CrossoverType enum to plugin string format
pub fn crossover_type_to_string(ct: &CrossoverType) -> &'static str {
    match ct {
        CrossoverType::Butterworth2 => "Butterworth12",
        CrossoverType::LinkwitzRiley2 => "LR12",
        CrossoverType::LinkwitzRiley4 => "LR24",
        CrossoverType::None => "None",
    }
}

use super::types::OptimizerConfig;

/// Optimize crossover for a group of driver measurements using autoeq's workflow
///
/// # Arguments
/// * `drivers` - Vector of driver measurements (will be sorted by frequency)
/// * `crossover_type` - Type of crossover to use
/// * `sample_rate` - Sample rate for filter design
/// * `config` - Optimizer configuration
///
/// # Returns
/// * Tuple of (optimal_gains, optimal_delays, optimal_crossover_freqs, combined_curve)
#[allow(clippy::type_complexity)]
pub fn optimize_crossover(
    drivers: Vec<Curve>,
    crossover_type: CrossoverType,
    sample_rate: f64,
    config: &OptimizerConfig,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, Curve), Box<dyn Error>> {
    // Convert Curve to DriverMeasurement
    let driver_measurements: Vec<DriverMeasurement> = drivers
        .into_iter()
        .map(|curve| DriverMeasurement {
            freq: curve.freq,
            spl: curve.spl,
            phase: curve.phase,
        })
        .collect();

    let drivers_data = DriversLossData::new(driver_measurements, crossover_type);
    let n_drivers = drivers_data.drivers.len();

    eprintln!(
        "  Optimizing crossover for {} drivers ({:?})",
        n_drivers, crossover_type
    );

    // Call library workflow to perform optimization
    let result = autoeq::workflow::optimize_drivers_crossover(
        drivers_data.clone(),
        config.min_freq,
        config.max_freq,
        sample_rate,
        &config.algorithm,
        config.max_iter,
        config.min_db,
        config.max_db,
    )?;

    // Compute the combined response
    let combined_response = autoeq::loss::compute_drivers_combined_response(
        &drivers_data,
        &result.gains,
        &result.crossover_freqs,
        Some(&result.delays),
        sample_rate,
    );

    let combined_curve = Curve {
        freq: drivers_data.freq_grid.clone(),
        spl: combined_response,
        phase: None,
    };

    eprintln!(
        "  Crossover optimization: gains={:?}, delays={:?} ms, freqs={:?}, final loss={:.6}",
        result
            .gains
            .iter()
            .map(|g| format!("{:+.2}", g))
            .collect::<Vec<_>>(),
        result
            .delays
            .iter()
            .map(|d| format!("{:.2}", d))
            .collect::<Vec<_>>(),
        result
            .crossover_freqs
            .iter()
            .map(|f| format!("{:.0}", f))
            .collect::<Vec<_>>(),
        result.post_objective
    );

    Ok((
        result.gains,
        result.delays,
        result.crossover_freqs,
        combined_curve,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_crossover_type() {
        assert!(matches!(
            parse_crossover_type("lr24"),
            Ok(CrossoverType::LinkwitzRiley4)
        ));
        assert!(matches!(
            parse_crossover_type("LR4"),
            Ok(CrossoverType::LinkwitzRiley4)
        ));
        assert!(matches!(
            parse_crossover_type("butterworth2"),
            Ok(CrossoverType::Butterworth2)
        ));
        assert!(parse_crossover_type("invalid").is_err());
    }
}
