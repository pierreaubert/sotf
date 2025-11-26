//! Crossover optimization for multi-driver groups

use autoeq::Curve;
use autoeq::cli::{Args, PeqModel};
use autoeq::loss::{CrossoverType, DriverMeasurement, DriversLossData};
use autoeq::workflow::setup_drivers_objective_data;
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
    }
}

/// Optimize crossover for a group of driver measurements using autoeq's workflow
///
/// # Arguments
/// * `drivers` - Vector of driver measurements (will be sorted by frequency)
/// * `crossover_type` - Type of crossover to use
/// * `sample_rate` - Sample rate for filter design
/// * `min_freq` - Minimum frequency for evaluation
/// * `max_freq` - Maximum frequency for evaluation
/// * `min_db` - Minimum gain bound in dB
/// * `max_db` - Maximum gain bound in dB
///
/// # Returns
/// * Tuple of (optimal_gains, optimal_crossover_freqs, combined_curve)
pub fn optimize_crossover(
    drivers: Vec<Curve>,
    crossover_type: CrossoverType,
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
    min_db: f64,
    max_db: f64,
) -> Result<(Vec<f64>, Vec<f64>, Curve), Box<dyn Error>> {
    // Convert Curve to DriverMeasurement
    let driver_measurements: Vec<DriverMeasurement> = drivers
        .into_iter()
        .map(|curve| DriverMeasurement {
            freq: curve.freq,
            spl: curve.spl,
            phase: None, // Curve doesn't have phase data
        })
        .collect();

    let drivers_data = DriversLossData::new(driver_measurements, crossover_type);
    let n_drivers = drivers_data.drivers.len();

    eprintln!(
        "  Optimizing crossover for {} drivers ({:?})",
        n_drivers, crossover_type
    );

    // Create Args structure for optimization
    let args = Args {
        // Number of parameters = n_drivers (gains) + (n_drivers-1) (crossovers)
        num_filters: 0, // Not used for driver optimization

        // Input data (not used)
        curve: None,
        target: None,
        speaker: None,
        version: None,
        measurement: None,
        curve_name: "On Axis".to_string(),

        // Sample rate
        sample_rate,

        // Frequency constraints
        min_freq,
        max_freq,

        // Q and gain constraints
        min_q: 0.5,
        max_q: 10.0,
        min_db,
        max_db,

        // Algorithm - use cobyla for simplicity
        algo: "nlopt:cobyla".to_string(),
        strategy: "currenttobest1bin".to_string(),
        algo_list: false,
        strategy_list: false,

        // PEQ model
        peq_model: PeqModel::Pk,
        peq_model_list: false,

        // Optimization parameters
        population: 300,
        maxeval: 5000,
        refine: false,
        local_algo: "cobyla".to_string(),

        // Spacing and smoothing (not used)
        min_spacing_oct: 0.0,
        spacing_weight: 0.0,
        smooth: false,
        smooth_n: 1,

        // Loss function
        loss: autoeq::loss::LossType::DriversFlat,

        // Optimization tuning
        tolerance: 1e-3,
        atolerance: 1e-4,
        recombination: 0.9,
        adaptive_weight_f: 0.9,
        adaptive_weight_cr: 0.9,
        no_parallel: false,

        // Output (not used)
        output: None,

        // Multi-driver
        driver1: None,
        driver2: None,
        driver3: None,
        driver4: None,
        crossover_type: "linkwitzriley4".to_string(),

        // Parallel threads
        parallel_threads: num_cpus::get(),

        // Random seed
        seed: None,

        // QA mode (disabled)
        qa: None,
    };

    // Setup objective data using autoeq's workflow
    let objective_data = setup_drivers_objective_data(&args, drivers_data);

    // Get bounds
    let (lower_bounds, upper_bounds) = autoeq::workflow::setup_drivers_bounds(
        &args,
        objective_data.drivers_data.as_ref().unwrap(),
    );

    // Generate initial guess
    let mut x = autoeq::workflow::drivers_initial_guess(&lower_bounds, &upper_bounds, n_drivers);

    // Perform optimization
    let opt_result = autoeq::optim::optimize_filters(
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
            eprintln!(
                "  Warning: crossover optimization did not fully converge: {}",
                msg
            );
            (msg, loss)
        }
    };

    // Extract results from optimized parameters
    let gains = x[0..n_drivers].to_vec();
    let xover_freqs_log10 = &x[n_drivers..];
    let xover_freqs: Vec<f64> = xover_freqs_log10
        .iter()
        .map(|f| 10.0_f64.powf(*f))
        .collect();

    // Compute the combined response
    let combined_response = autoeq::loss::compute_drivers_combined_response(
        objective_data.drivers_data.as_ref().unwrap(),
        &gains,
        &xover_freqs,
        sample_rate,
    );

    let combined_curve = Curve {
        freq: objective_data
            .drivers_data
            .as_ref()
            .unwrap()
            .freq_grid
            .clone(),
        spl: combined_response,
    };

    eprintln!(
        "  Crossover optimization: gains={:?}, freqs={:?}, final loss={:.6}",
        gains
            .iter()
            .map(|g| format!("{:+.2}", g))
            .collect::<Vec<_>>(),
        xover_freqs
            .iter()
            .map(|f| format!("{:.0}", f))
            .collect::<Vec<_>>(),
        final_loss
    );

    Ok((gains, xover_freqs, combined_curve))
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
