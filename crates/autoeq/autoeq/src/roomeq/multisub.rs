//! Multi-subwoofer optimization

use crate::Curve;
use crate::loss::{CrossoverType, DriverMeasurement, DriversLossData};
use crate::workflow::DriverOptimizationResult;
use log::warn;
use std::error::Error;

use super::types::{MeasurementSource, OptimizerConfig};
use crate::read as load;

/// Optimize multi-subwoofer configuration
///
/// # Arguments
/// * `measurements` - List of subwoofer measurements (sources)
/// * `config` - Optimizer configuration
/// * `sample_rate` - Sample rate
///
/// # Returns
/// * Tuple of (DriverOptimizationResult, Combined Curve)
///
/// # Note on Phase Data
/// For accurate optimization, measurements should include phase data.
/// The optimizer uses complex summation to model constructive/destructive
/// interference between subwoofers. Without phase data, the optimizer
/// assumes 0° phase for all measurements, which may result in suboptimal
/// delay settings.
pub fn optimize_multisub(
    measurements: &[MeasurementSource],
    config: &OptimizerConfig,
    sample_rate: f64,
) -> Result<(DriverOptimizationResult, Curve), Box<dyn Error>> {
    // Load all measurements and check for phase data
    let mut driver_measurements = Vec::new();
    let mut missing_phase_count = 0;

    for source in measurements {
        let curve = load::load_source(source)?;
        if curve.phase.is_none() {
            missing_phase_count += 1;
        }
        driver_measurements.push(DriverMeasurement {
            freq: curve.freq,
            spl: curve.spl,
            phase: curve.phase, // Critical: use phase for accurate summation
        });
    }

    // Warn if phase data is missing
    if missing_phase_count > 0 {
        warn!(
            "Multi-sub optimization: {} of {} measurements are missing phase data. \
            This may result in inaccurate delay optimization. \
            For best results, include phase data in your measurements (e.g., export from REW with phase).",
            missing_phase_count,
            measurements.len()
        );
    }

    // Create drivers data with NO crossover filtering
    let drivers_data = DriversLossData::new(driver_measurements, CrossoverType::None);

    let result = crate::workflow::optimize_multisub(
        drivers_data.clone(),
        config.min_freq,
        config.max_freq,
        sample_rate,
        &config.algorithm,
        config.max_iter,
        config.min_db,
        config.max_db,
        config.seed,
    )?;

    // Compute combined response
    let combined_response = crate::loss::compute_drivers_combined_response(
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
