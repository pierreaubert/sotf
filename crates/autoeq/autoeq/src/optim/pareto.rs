//! Multi-objective optimization for Pareto-optimal filter sets.
//!
//! Research:
//! "Multi-Objective Genetic Algorithms for Loudspeaker Equalization"
//! "Pareto-Optimal Solutions for Loudspeaker System Design"

use crate::cli::Args;
use crate::optim::ObjectiveData;

/// Pareto-optimal filter solution
#[derive(Debug, Clone)]
pub struct ParetoFilter {
    /// Optimized parameters
    pub params: Vec<f64>,
    /// Flatness loss value
    pub flatness_loss: f64,
    /// Score loss value (if computed)
    pub score_loss: Option<f64>,
    /// Number of filters used
    pub num_filters: usize,
    /// Convergence status
    pub converged: bool,
}

/// Run optimization for different filter counts and collect Pareto front
pub fn pareto_optimization(
    objective_data: &ObjectiveData,
    args: &Args,
    filter_counts: Vec<usize>,
) -> Vec<ParetoFilter> {
    let mut pareto_front = Vec::new();

    for &n_filters in &filter_counts {
        // Clone args with different filter count
        let mut args_with_filters = args.clone();
        args_with_filters.num_filters = n_filters;

        // Run optimization
        // We need to initialize x (params), lower_bounds, upper_bounds
        let (lower_bounds, upper_bounds) = crate::workflow::setup_bounds(&args_with_filters);
        // Initialize x with random/initial values or let optimizer handle it
        // The optimizer expects x to be initialized.
        // We can use setup_initial_guess from workflow
        let mut x =
            crate::workflow::initial_guess(&args_with_filters, &lower_bounds, &upper_bounds);

        let result = crate::optim::optimize_filters(
            &mut x, // Will be filled by optimizer
            &lower_bounds,
            &upper_bounds,
            objective_data.clone(),
            &args_with_filters,
        );

        match result {
            Ok((_, loss)) => {
                pareto_front.push(ParetoFilter {
                    params: x,
                    flatness_loss: loss,
                    score_loss: None,
                    num_filters: n_filters,
                    converged: true,
                });
            }
            Err((_, loss)) => {
                pareto_front.push(ParetoFilter {
                    params: x,
                    flatness_loss: loss,
                    score_loss: None,
                    num_filters: n_filters,
                    converged: false,
                });
            }
        }
    }

    pareto_front
}

/// Extract non-dominated solutions from Pareto front
pub fn extract_non_dominated(filters: &[ParetoFilter]) -> Vec<&ParetoFilter> {
    let mut non_dominated = Vec::new();

    for candidate in filters {
        let mut is_dominated = false;
        for other in filters {
            if std::ptr::eq(other, candidate) {
                continue;
            }
            // other dominates candidate if:
            // - other has less or equal loss in all objectives
            // - and strictly less in at least one
            let other_flat_le = other.flatness_loss <= candidate.flatness_loss;
            let other_flat_lt = other.flatness_loss < candidate.flatness_loss;
            let other_filters_le = other.num_filters <= candidate.num_filters;
            let other_filters_lt = other.num_filters < candidate.num_filters;

            if other_flat_le && other_filters_le && (other_flat_lt || other_filters_lt) {
                is_dominated = true;
                break;
            }
        }

        if !is_dominated {
            non_dominated.push(candidate);
        }
    }

    non_dominated
}

/// Print Pareto front for user selection
pub fn print_pareto_front(filters: &[ParetoFilter]) {
    println!("\nPareto-Optimal Filter Configurations:");
    println!("=====================================");
    println!("# | Filters | Flatness Loss | Converged");
    println!("--+---------+---------------+-----------");

    for (i, f) in filters.iter().enumerate() {
        println!(
            "{} | {:3}     | {:12.6}   | {}",
            i + 1,
            f.num_filters,
            f.flatness_loss,
            if f.converged { "Yes" } else { "No" }
        );
    }

    println!("\nRecommendation: Choose the configuration with the fewest");
    println!("filters that meets your loss tolerance threshold.");
}
