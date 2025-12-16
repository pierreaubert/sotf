//! Multi-subwoofer optimization

use autoeq::Curve;
use autoeq::loss::{CrossoverType, DriverMeasurement, DriversLossData};
use autoeq::workflow::DriverOptimizationResult;
use std::error::Error;

use super::types::{MeasurementSource, OptimizerConfig};
use autoeq::read as load;

/// Optimize multi-subwoofer configuration
///
/// # Arguments
/// * `measurements` - List of subwoofer measurements (sources)
/// * `config` - Optimizer configuration
/// * `sample_rate` - Sample rate
///
/// # Returns
/// * Tuple of (DriverOptimizationResult, Combined Curve)
pub fn optimize_multisub(
    measurements: &[MeasurementSource],
    config: &OptimizerConfig,
    sample_rate: f64,
) -> Result<(DriverOptimizationResult, Curve), Box<dyn Error>> {
    // Load all measurements
    let mut driver_measurements = Vec::new();
    for source in measurements {
        let curve = load::load_source(source)?;
        driver_measurements.push(DriverMeasurement {
            freq: curve.freq,
            spl: curve.spl,
            phase: curve.phase, // Critical: use phase
        });
    }

    // Create drivers data with NO crossover filtering
    let drivers_data = DriversLossData::new(driver_measurements, CrossoverType::None);

    let result = autoeq::workflow::optimize_multisub(
        drivers_data.clone(),
        config.min_freq,
        config.max_freq,
        sample_rate,
        &config.algorithm,
        config.max_iter,
        config.min_db,
        config.max_db,
    )?;

    // Compute combined response
    let combined_response = autoeq::loss::compute_drivers_combined_response(
        &drivers_data,
        &result.gains,
        &[], // no crossovers
        Some(&result.delays),
        sample_rate,
    );

    let combined_curve = Curve {
        freq: drivers_data.freq_grid.clone(),
        spl: combined_response,
        phase: None,
    };

    Ok((result, combined_curve))
}
