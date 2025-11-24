// Metaheuristics-specific optimization code

use super::optim::{ObjectiveData, compute_fitness_penalties};
use ndarray::Array1;

#[allow(unused_imports)]
use metaheuristics_nature as mh;
#[allow(unused_imports)]
use mh::methods::{De as MhDe, Fa as MhFa, Pso as MhPso, Rga as MhRga, Tlbo as MhTlbo};
#[allow(unused_imports)]
use mh::{Bounded as MhBounded, Fitness as MhFitness, ObjFunc as MhObjFunc, Solver as MhSolver};

/// Information passed to callback after each generation
/// Similar to DEIntermediate but for metaheuristics optimizers
pub struct MHIntermediate {
    pub x: Array1<f64>,
    pub fun: f64,
    pub iter: usize,
}

/// Callback action - shared with DE module for consistency
pub use crate::de::CallbackAction;

// ---------------- Metaheuristics objective and utilities ----------------
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MHObjective {
    pub data: ObjectiveData,
    pub bounds: Vec<[f64; 2]>,
    /// Optional callback state for tracking progress
    pub callback_state: Option<Arc<Mutex<CallbackState>>>,
}

/// State tracked across fitness evaluations for callback reporting
pub struct CallbackState {
    pub best_fitness: f64,
    pub best_params: Vec<f64>,
    pub eval_count: usize,
    pub last_report_eval: usize,
}

impl MhBounded for MHObjective {
    fn bound(&self) -> &[[f64; 2]] {
        self.bounds.as_slice()
    }
}

impl MhObjFunc for MHObjective {
    type Ys = f64;
    fn fitness(&self, xs: &[f64]) -> Self::Ys {
        // Create mutable copy of data for compute_fitness_penalties
        let mut data_copy = self.data.clone();
        let fitness_val = compute_fitness_penalties(xs, None, &mut data_copy);

        // Update callback state if present
        if let Some(ref state_arc) = self.callback_state
            && let Ok(mut state) = state_arc.lock()
        {
            state.eval_count += 1;

            // Track best solution
            if fitness_val < state.best_fitness {
                state.best_fitness = fitness_val;
                state.best_params = xs.to_vec();
            }
        }

        fitness_val
    }
}

/// Create a default callback for metaheuristics that prints progress
pub fn create_mh_callback(
    algo_name: &str,
) -> Box<dyn FnMut(&MHIntermediate) -> CallbackAction + Send> {
    let name = algo_name.to_string();
    let mut last_fitness = f64::INFINITY;
    let mut stall_count = 0;

    Box::new(move |intermediate: &MHIntermediate| -> CallbackAction {
        // Check for progress
        let improvement = if intermediate.fun < last_fitness {
            let delta = last_fitness - intermediate.fun;
            last_fitness = intermediate.fun;
            stall_count = 0;
            format!("(-{:.2e})", delta)
        } else {
            stall_count += 1;
            if stall_count >= 50 {
                format!("(STALL:{})", stall_count)
            } else {
                "(--) ".to_string()
            }
        };

        // Print when stalling or periodically
        if stall_count == 1 || stall_count % 25 == 0 || intermediate.iter.is_multiple_of(10) {
            let msg = format!(
                "{} iter {:4}  fitness={:.6e} {}",
                name, intermediate.iter, intermediate.fun, improvement
            );
            crate::qa_println!("{}", msg);
        }

        // Show parameter details every 50 iterations
        if intermediate.iter > 0 && intermediate.iter.is_multiple_of(50) {
            let param_summary: Vec<String> = (0..intermediate.x.len() / 3)
                .map(|i| {
                    let freq = 10f64.powf(intermediate.x[i * 3]);
                    let q = intermediate.x[i * 3 + 1];
                    let gain = intermediate.x[i * 3 + 2];
                    format!("[f{:.0}Hz Q{:.2} G{:.2}dB]", freq, q, gain)
                })
                .collect();
            crate::qa_println!("  --> Best params: {}", param_summary.join(" "));
        }

        CallbackAction::Continue
    })
}

/// Optimize filter parameters using metaheuristics algorithms
pub fn optimize_filters_mh(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    mh_name: &str,
    population: usize,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {
    // Create default callback for terminal output
    let callback = create_mh_callback(&format!("mh::{}", mh_name));

    // Delegate to callback version
    optimize_filters_mh_with_callback(
        x,
        lower_bounds,
        upper_bounds,
        objective_data,
        mh_name,
        population,
        maxeval,
        callback,
    )
}

/// Optimize filter parameters using metaheuristics algorithms with callback support
#[allow(clippy::too_many_arguments)]
pub fn optimize_filters_mh_with_callback(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    mh_name: &str,
    population: usize,
    maxeval: usize,
    mut callback: Box<dyn FnMut(&MHIntermediate) -> CallbackAction + Send>,
) -> Result<(String, f64), (String, f64)> {
    let num_params = x.len();

    // Build bounds for metaheuristics (as pairs)
    assert_eq!(lower_bounds.len(), num_params);
    assert_eq!(upper_bounds.len(), num_params);
    let mut bounds: Vec<[f64; 2]> = Vec::with_capacity(num_params);
    for i in 0..num_params {
        bounds.push([lower_bounds[i], upper_bounds[i]]);
    }

    // Create objective with penalties (metaheuristics don't support constraints)
    let mut penalty_data = objective_data.clone();
    // PSO needs balanced penalties - not too harsh to allow exploration,
    // but strong enough to guide toward feasible solutions
    let (ceiling_penalty, spacing_penalty, mingain_penalty) = if mh_name == "pso" {
        (5e2, objective_data.spacing_weight.max(0.0) * 5e2, 50.0) // Moderate penalties
    } else {
        (1e4, objective_data.spacing_weight.max(0.0) * 1e3, 1e3)
    };
    penalty_data.penalty_w_ceiling = ceiling_penalty;
    penalty_data.penalty_w_spacing = spacing_penalty;
    penalty_data.penalty_w_mingain = mingain_penalty;

    // Create callback state
    let callback_state = Arc::new(Mutex::new(CallbackState {
        best_fitness: f64::INFINITY,
        best_params: vec![],
        eval_count: 0,
        last_report_eval: 0,
    }));

    // Clone for the task closure
    let callback_state_task = Arc::clone(&callback_state);

    // Simple objective function wrapper for metaheuristics
    let mh_obj = MHObjective {
        data: penalty_data,
        bounds,
        callback_state: Some(Arc::clone(&callback_state)),
    };

    // Choose algorithm configuration
    // Use boxed builder to allow runtime selection with unified type
    let builder = match mh_name {
        "de" => MhSolver::build_boxed(MhDe::default(), mh_obj),
        "pso" => {
            // Tuned PSO parameters for this implementation
            // This PSO uses: v = velocity*x + cognition*r1*(pbest-x) + social*r2*(gbest-x)
            // where v becomes the new position (not standard PSO)
            // Balance exploration and exploitation
            let pso_tuned = MhPso::default()
                .cognition(1.0) // Equal personal best influence
                .social(1.5) // Stronger global best attraction
                .velocity(0.9); // Moderate inertia for gradual convergence
            MhSolver::build_boxed(pso_tuned, mh_obj)
        }
        "rga" => {
            // RGA works well for constrained optimization with default parameters
            // Note: RGA benefits from larger populations (recommended: 100+)
            MhSolver::build_boxed(MhRga::default(), mh_obj)
        }
        "tlbo" => MhSolver::build_boxed(MhTlbo, mh_obj),
        "fa" | "firefly" => {
            // Firefly works well for constrained optimization
            // alpha: randomization parameter (exploration)
            // beta_min: minimum attractiveness (exploitation)
            // gamma: light absorption coefficient (distance sensitivity)
            let fa_tuned = MhFa::default()
                .alpha(0.5) // Reduced randomization for more focused search
                .beta_min(1.0) // Keep default attractiveness
                .gamma(0.01); // Keep default absorption
            MhSolver::build_boxed(fa_tuned, mh_obj)
        }
        _ => MhSolver::build_boxed(MhDe::default(), mh_obj),
    };

    // Estimate generations from maxeval and population
    let pop = population.max(1);
    let gens = (maxeval.max(pop)).div_ceil(pop); // ceil(maxeval/pop)

    // Track iteration count
    let mut current_iter = 0_usize;
    let report_interval = 100; // Report every N evaluations

    let solver = builder
        .seed(0)
        .pop_num(pop)
        .task(move |_ctx| {
            current_iter += 1;

            // Report progress periodically
            if let Ok(mut state) = callback_state_task.lock() {
                let evals_since_last = state.eval_count.saturating_sub(state.last_report_eval);

                if evals_since_last >= report_interval {
                    // Create intermediate state for callback
                    let x_array = Array1::from(state.best_params.clone());
                    let intermediate = MHIntermediate {
                        x: x_array,
                        fun: state.best_fitness,
                        iter: current_iter,
                    };

                    // Call the callback
                    let action = callback(&intermediate);
                    state.last_report_eval = state.eval_count;

                    // Check if user wants to stop
                    if matches!(action, CallbackAction::Stop) {
                        return true; // Signal to stop optimization
                    }
                }
            }

            // Continue until max generations
            current_iter >= gens
        })
        .solve();

    // Write back the best parameters
    let best_xs = solver.as_best_xs();
    if best_xs.len() == x.len() {
        x.copy_from_slice(best_xs);
    }
    let best_val = *solver.as_best_fit();
    Ok((format!("Metaheuristics({})", mh_name), best_val))
}
