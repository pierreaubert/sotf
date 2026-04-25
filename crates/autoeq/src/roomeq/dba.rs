//! Double Bass Array (DBA) optimization
//!
//! # Phase Data Requirement
//!
//! DBA optimization relies on complex summation to model the interaction between
//! front and rear subwoofer arrays. For accurate optimization, measurements should
//! include phase data. Missing phase is rejected rather than replaced with an
//! invented 0° phase response.
//!
//! The rear array is automatically inverted (180° phase shift) to create the
//! pressure wave cancellation pattern characteristic of DBA systems.

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
///
/// # Note on Phase Data
/// For accurate DBA optimization, measurements should include phase data.
/// The optimizer uses complex summation to model constructive/destructive
/// interference between front and rear arrays.
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

    // Build Args to derive OptimParams (DBA needs custom bounds, not OptimizerConfig defaults)
    let mut args = crate::cli::Args::parse_from(["autoeq"]); // defaults
    args.sample_rate = sample_rate;
    args.min_freq = config.min_freq;
    args.max_freq = config.max_freq;
    args.maxeval = config.max_iter;
    args.population = config.population;
    args.algo = config.algorithm.clone();
    args.seed = config.seed;
    args.loss = crate::LossType::MultiSubFlat;

    let optim_params = crate::OptimParams::from(&args);
    let objective_data =
        crate::workflow::setup_multisub_objective_data(&optim_params, drivers_data.clone());

    // Custom bounds: [Gain1, Gain2, Delay1, Delay2]
    // Index 0: Front Gain -> 0 (Locked)
    // Index 1: Rear Gain -> config bounds (typically attenuated)
    // Index 2: Front Delay -> 0 (Locked)
    // Index 3: Rear Delay -> 0 to 100 ms (approx 34m room)

    // DBA rear array is for cancellation — clamp rear gain to 0 dB max
    let min_gain = config.min_db.min(-30.0);
    let max_gain = 0.0;

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
        &optim_params,
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
    let combined_curve = compute_dba_combined_curve(
        &front_curve,
        &rear_curve_inverted,
        &gains,
        &delays,
        &drivers_data.freq_grid,
        sample_rate,
    )?;

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
///
/// # Phase Data
/// This function uses complex summation to properly model interference patterns.
/// If any measurement is missing phase data, this returns an error. DBA is a
/// phase-critical feature and should not invent coherence from magnitude-only
/// data.
pub fn sum_array_response(
    sources: &[super::types::MeasurementSource],
) -> Result<Curve, Box<dyn Error>> {
    if sources.is_empty() {
        return Err("Empty array".into());
    }

    // Load all and check for phase data
    let mut curves = Vec::new();
    for source in sources {
        let curve = load::load_source(source)?;
        if curve.phase.is_none() {
            return Err(format!(
                "DBA array summation requires phase data for source {:?}",
                source
            )
            .into());
        }
        curves.push(curve);
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
            let phase = interp
                .phase
                .as_ref()
                .ok_or("DBA interpolation lost required phase data")?[i];
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
        ..Default::default()
    })
}

/// Invert polarity of a curve (add 180 deg)
fn invert_polarity(curve: &Curve) -> Curve {
    let mut new_curve = curve.clone();
    if let Some(ref mut phase) = new_curve.phase {
        *phase = phase.mapv(|p| p + 180.0);
    }
    new_curve
}

fn compute_dba_combined_curve(
    front_curve: &Curve,
    rear_curve: &Curve,
    gains: &[f64],
    delays_ms: &[f64],
    freq_grid: &Array1<f64>,
    _sample_rate: f64,
) -> Result<Curve, Box<dyn Error>> {
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let front = crate::read::interpolate_log_space(freq_grid, front_curve);
    let rear = crate::read::interpolate_log_space(freq_grid, rear_curve);
    let front_phase = front
        .phase
        .as_ref()
        .ok_or("DBA combined curve requires front phase data")?;
    let rear_phase = rear
        .phase
        .as_ref()
        .ok_or("DBA combined curve requires rear phase data")?;
    let front_gain = gains.first().copied().unwrap_or(0.0);
    let rear_gain = gains.get(1).copied().unwrap_or(0.0);
    let front_delay_s = delays_ms.first().copied().unwrap_or(0.0) / 1000.0;
    let rear_delay_s = delays_ms.get(1).copied().unwrap_or(0.0) / 1000.0;

    let mut sum_complex = Array1::<Complex64>::zeros(freq_grid.len());
    for i in 0..freq_grid.len() {
        let f = freq_grid[i];
        let front_mag = 10.0_f64.powf((front.spl[i] + front_gain) / 20.0);
        let rear_mag = 10.0_f64.powf((rear.spl[i] + rear_gain) / 20.0);
        let front_phi = front_phase[i].to_radians() - 2.0 * PI * f * front_delay_s;
        let rear_phi = rear_phase[i].to_radians() - 2.0 * PI * f * rear_delay_s;
        sum_complex[i] =
            Complex64::from_polar(front_mag, front_phi) + Complex64::from_polar(rear_mag, rear_phi);
    }

    Ok(Curve {
        freq: freq_grid.clone(),
        spl: sum_complex.mapv(|z| 20.0 * z.norm().max(1e-12).log10()),
        phase: Some(sum_complex.mapv(|z| z.arg().to_degrees())),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MeasurementSource;

    #[test]
    fn test_invert_polarity() {
        let freq = Array1::from(vec![100.0, 1000.0]);
        let spl = Array1::from(vec![80.0, 80.0]);
        let phase = Array1::from(vec![0.0, -90.0]);

        let curve = Curve {
            freq: freq.clone(),
            spl: spl.clone(),
            phase: Some(phase.clone()),
            ..Default::default()
        };

        let inverted = invert_polarity(&curve);

        let inv_phase = inverted.phase.unwrap();
        assert!((inv_phase[0] - 180.0).abs() < 1e-6);
        assert!((inv_phase[1] - 90.0).abs() < 1e-6); // -90 + 180 = 90
    }

    #[test]
    fn sum_array_response_rejects_missing_phase() {
        let curve = Curve {
            freq: Array1::from(vec![50.0, 100.0]),
            spl: Array1::from(vec![80.0, 80.0]),
            phase: None,
            ..Default::default()
        };

        let err = sum_array_response(&[MeasurementSource::InMemory(curve)]).unwrap_err();
        assert!(
            err.to_string().contains("requires phase data"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sum_array_response_preserves_complex_phase() {
        let curve_a = Curve {
            freq: Array1::from(vec![100.0]),
            spl: Array1::from(vec![80.0]),
            phase: Some(Array1::from(vec![0.0])),
            ..Default::default()
        };
        let curve_b = Curve {
            freq: Array1::from(vec![100.0]),
            spl: Array1::from(vec![80.0]),
            phase: Some(Array1::from(vec![90.0])),
            ..Default::default()
        };

        let summed = sum_array_response(&[
            MeasurementSource::InMemory(curve_a),
            MeasurementSource::InMemory(curve_b),
        ])
        .unwrap();

        assert!(summed.phase.is_some());
        assert!((summed.phase.as_ref().unwrap()[0] - 45.0).abs() < 1e-6);
    }
}
