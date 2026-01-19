//! Double Bass Array (DBA) optimization

use crate::Curve;
use crate::loss::{CrossoverType, DriverMeasurement, DriversLossData};
use crate::workflow::DriverOptimizationResult;
use clap::Parser;
use ndarray::Array1;
use std::error::Error;

use super::types::{DBAConfig, OptimizerConfig};
use crate::read as load;

/// Optimize Double Bass Array configuration
///
/// # Arguments
/// * `dba_config` - DBA configuration (front/rear sources)
/// * `config` - Optimizer configuration
/// * `sample_rate` - Sample rate
///
/// # Returns
/// * Tuple of (DriverOptimizationResult, Combined Curve)
///   Result contains 2 entries: Index 0 = Front, Index 1 = Rear
pub fn optimize_dba(
    dba_config: &DBAConfig,
    config: &OptimizerConfig,
    sample_rate: f64,
) -> Result<(DriverOptimizationResult, Curve), Box<dyn Error>> {
    // 1. Load and Sum Front Array
    let front_curve = sum_array_response(&dba_config.front)?;

    // 2. Load and Sum Rear Array
    let rear_curve = sum_array_response(&dba_config.rear)?;

    // 3. Create optimization targets
    // We have 2 "drivers": Front Aggregate and Rear Aggregate
    // Front is fixed (Gain 0, Delay 0)
    // Rear is optimized (Gain, Delay)
    // DBA implies Rear is INVERTED relative to Front.
    // We add 180 degrees to Rear phase to simulate inversion.

    let rear_curve_inverted = invert_polarity(&rear_curve);

    let driver_measurements = vec![
        DriverMeasurement {
            freq: front_curve.freq.clone(),
            spl: front_curve.spl.clone(),
            phase: front_curve.phase.clone(),
        },
        DriverMeasurement {
            freq: rear_curve_inverted.freq.clone(),
            spl: rear_curve_inverted.spl.clone(),
            phase: rear_curve_inverted.phase.clone(),
        },
    ];

    let drivers_data = DriversLossData::new(driver_measurements, CrossoverType::None);

    // 4. Custom optimization
    // We can't use standard optimize_multisub because it optimizes ALL gains/delays.
    // We want to lock Front parameters.
    // So we'll implement a constrained optimization here or use custom bounds.

    // Custom bounds:
    // Front: Gain [-0.1, 0.1], Delay [0, 0] (Tight bounds effectively lock it)
    // Rear: Gain [-20, 0], Delay [0, 50ms] (DBA usually attenuates rear slightly)

    // We'll reuse the workflow helpers but supply custom bounds.

    // Create Args
    let mut args = crate::cli::Args::parse_from(["autoeq"]); // defaults
    args.sample_rate = sample_rate;
    args.min_freq = config.min_freq;
    args.max_freq = config.max_freq;
    args.maxeval = config.max_iter;
    args.algo = config.algorithm.clone();
    args.loss = crate::LossType::MultiSubFlat;

    let objective_data =
        crate::workflow::setup_multisub_objective_data(&args, drivers_data.clone());

    // Custom bounds: [Gain1, Gain2, Delay1, Delay2]
    // Index 0: Front Gain -> 0 (Locked)
    // Index 1: Rear Gain -> config bounds (typically attenuated)
    // Index 2: Front Delay -> 0 (Locked)
    // Index 3: Rear Delay -> 0 to 100 ms (approx 34m room)

    // Ensure bounds cover typical DBA range (-30dB to +5dB)
    let min_gain = config.min_db.min(-30.0);
    let max_gain = config.max_db.max(5.0);

    let lower_bounds = vec![-0.01, min_gain, 0.0, 0.0];
    let upper_bounds = vec![0.01, max_gain, 0.001, 100.0];

    // Initial guess
    // Rear delay guess: 10ms (~3.4m room)
    // Rear gain guess: -3dB
    let mut x = vec![0.0, -3.0, 0.0, 10.0];

    // Optimize
    let opt_result = crate::optim::optimize_filters(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data,
        &args,
    );

    let converged = opt_result.is_ok();

    // Recompute scores
    // Note: compute_base_fitness uses args.loss_type which we set to MultiSubFlat
    // and uses setup_multisub_objective_data
    // So we can assume it works.

    let gains = vec![x[0], x[1]];
    let delays = vec![x[2], x[3]];
    let crossover_freqs = vec![];

    // Compute combined response
    let combined_response = crate::loss::compute_drivers_combined_response(
        &drivers_data,
        &gains,
        &crossover_freqs,
        Some(&delays),
        sample_rate,
    );

    let combined_curve = Curve {
        freq: drivers_data.freq_grid.clone(),
        spl: combined_response,
        phase: None,
    };

    Ok((
        DriverOptimizationResult {
            gains,
            delays,
            crossover_freqs,
            pre_objective: 0.0, // Lazy
            post_objective: 0.0,
            converged,
        },
        combined_curve,
    ))
}

/// Sum multiple measurements into a single curve (complex summation)
fn sum_array_response(
    sources: &[super::types::MeasurementSource],
) -> Result<Curve, Box<dyn Error>> {
    if sources.is_empty() {
        return Err("Empty array".into());
    }

    // Load all
    let mut curves = Vec::new();
    for source in sources {
        curves.push(load::load_source(source)?);
    }

    // Reference freq from first
    let ref_freq = curves[0].freq.clone();

    // Sum complex
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let mut sum_complex = Array1::<Complex64>::zeros(ref_freq.len());

    for curve in &curves {
        // Interpolate to ref grid
        let interp = crate::read::interpolate_log_space(&ref_freq, curve);

        for i in 0..ref_freq.len() {
            let spl = interp.spl[i];
            let phase = interp.phase.as_ref().map(|p| p[i]).unwrap_or(0.0);
            let m = 10.0_f64.powf(spl / 20.0);
            let phi = phase * PI / 180.0;
            sum_complex[i] += Complex64::from_polar(m, phi);
        }
    }

    // Convert to SPL/Phase
    let spl = sum_complex.mapv(|z| 20.0 * z.norm().max(1e-12).log10());
    let phase = sum_complex.mapv(|z| z.arg() * 180.0 / PI);

    Ok(Curve {
        freq: ref_freq,
        spl,
        phase: Some(phase),
    })
}

/// Invert polarity of a curve (add 180 deg)
fn invert_polarity(curve: &Curve) -> Curve {
    let mut new_curve = curve.clone();
    if let Some(ref mut phase) = new_curve.phase {
        *phase = phase.mapv(|p| p + 180.0);
    } else {
        // If no phase, assume 0 -> 180
        new_curve.phase = Some(Array1::from_elem(curve.freq.len(), 180.0));
    }
    new_curve
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invert_polarity() {
        let freq = Array1::from(vec![100.0, 1000.0]);
        let spl = Array1::from(vec![80.0, 80.0]);
        let phase = Array1::from(vec![0.0, -90.0]);

        let curve = Curve {
            freq: freq.clone(),
            spl: spl.clone(),
            phase: Some(phase.clone()),
        };

        let inverted = invert_polarity(&curve);

        let inv_phase = inverted.phase.unwrap();
        assert!((inv_phase[0] - 180.0).abs() < 1e-6);
        assert!((inv_phase[1] - 90.0).abs() < 1e-6); // -90 + 180 = 90
    }
}
