// NLOPT-specific optimization code

use super::constraints::{
    CeilingConstraintData, CrossoverMonotonicityConstraintData, MinGainConstraintData,
    constraint_ceiling, constraint_crossover_monotonicity, constraint_min_gain,
};
use super::optim::ObjectiveData;
use super::optim::compute_fitness_penalties;
use crate::LossType;
use nlopt::{Algorithm, Nlopt, Target};

/// Optimize filter parameters using NLOPT algorithms
pub fn optimize_filters_nlopt(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    algo: Algorithm,
    population: usize,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {
    let num_params = x.len();

    // Decide whether to use penalties (for algorithms lacking inequality constraints)
    let use_penalties = matches!(
        algo,
        Algorithm::Crs2Lm
            | Algorithm::Direct
            | Algorithm::DirectL
            | Algorithm::GMlsl
            | Algorithm::GMlslLds
            | Algorithm::Sbplx
            | Algorithm::StoGo
            | Algorithm::StoGoRand
            | Algorithm::Neldermead
    );

    // Prepare constraint data BEFORE moving objective_data into NLopt
    let ceiling_data = CeilingConstraintData {
        freqs: objective_data.freqs.clone(),
        srate: objective_data.srate,
        max_db: objective_data.max_db,
        peq_model: objective_data.peq_model,
    };
    let min_gain_data = MinGainConstraintData {
        min_db: objective_data.min_db,
        peq_model: objective_data.peq_model,
    };

    // Prepare crossover monotonicity constraint for multi-driver optimization
    let crossover_monotonicity_data = if objective_data.loss_type == LossType::DriversFlat {
        objective_data.drivers_data.as_ref().map(|drivers_data| {
            CrossoverMonotonicityConstraintData {
                n_drivers: drivers_data.drivers.len(),
                // Require at least 0.15 in log10 space (about 40% frequency separation)
                // This ensures crossover frequencies don't converge to the same value
                min_log_separation: 0.15,
            }
        })
    } else {
        None
    };

    // Configure penalty weights when needed
    let mut objective_data = objective_data;
    if use_penalties {
        objective_data.penalty_w_ceiling = 1e4;
        objective_data.penalty_w_spacing = objective_data.spacing_weight.max(0.0) * 1e3;
        objective_data.penalty_w_mingain = 1e3;
    } else {
        objective_data.penalty_w_ceiling = 0.0;
        objective_data.penalty_w_spacing = 0.0;
        objective_data.penalty_w_mingain = 0.0;
    }

    // Now create optimizer and move objective_data
    let mut optimizer = Nlopt::new(
        algo,
        num_params,
        compute_fitness_penalties,
        Target::Minimize,
        objective_data,
    );

    // These unwraps are safe because lower_bounds and upper_bounds have the same
    // length as the optimizer dimension (num_params), which is validated by NLopt
    optimizer
        .set_lower_bounds(lower_bounds)
        .expect("lower bounds should have correct dimension");
    optimizer
        .set_upper_bounds(upper_bounds)
        .expect("upper bounds should have correct dimension");

    // Register inequality constraints when not using penalties.
    if !use_penalties {
        let _ = optimizer.add_inequality_constraint(constraint_ceiling, ceiling_data, 1e-6);
        // let _ = optimizer.add_inequality_constraint(constraint_spacing, spacing_data, 1e-9);
        let _ = optimizer.add_inequality_constraint(constraint_min_gain, min_gain_data, 1e-6);

        // Add crossover monotonicity constraint for multi-driver optimization
        if let Some(xover_data) = crossover_monotonicity_data {
            let _ = optimizer.add_inequality_constraint(
                constraint_crossover_monotonicity,
                xover_data,
                1e-6,
            );
        }
    }

    let _ = optimizer.set_population(population);
    let _ = optimizer.set_maxeval(maxeval as u32);
    // Stopping criteria - these should never fail with valid positive values
    optimizer
        .set_stopval(1e-4)
        .expect("stopval should be valid");
    optimizer
        .set_ftol_rel(1e-6)
        .expect("ftol_rel should be valid");
    optimizer
        .set_xtol_rel(1e-4)
        .expect("xtol_rel should be valid");

    let result = optimizer.optimize(x);

    match result {
        Ok((status, val)) => Ok((format!("{:?}", status), val)),
        Err((e, val)) => Err((format!("{:?}", e), val)),
    }
}
