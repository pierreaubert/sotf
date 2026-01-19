// AutoEQ DE-specific optimization code

use ndarray::Array1;
use std::sync::Arc;

use super::constraints::{
    CeilingConstraintData, MinGainConstraintData, SpacingConstraintData, constraint_ceiling,
    constraint_min_gain, constraint_spacing,
};
use super::init_sobol::init_sobol;
use super::initial_guess::{SmartInitConfig, create_smart_initial_guesses};
use super::optim::{ObjectiveData, PenaltyMode, compute_fitness_penalties};
use super::optim_callback::{ProgressTracker, format_param_summary};
use crate::de::{
    CallbackAction, DEConfig, DEConfigBuilder, DEIntermediate, DEReport, Init, Mutation,
    NonlinearConstraintHelper, ParallelConfig, Strategy, differential_evolution,
};

/// Common setup for DE-based optimization
///
/// Contains all the shared configuration parameters for both standard and adaptive DE algorithms.
pub struct DESetup {
    /// Parameter bounds as (lower, upper) tuples for optde
    pub bounds: Vec<(f64, f64)>,
    /// Objective data with penalty weights configured
    pub penalty_data: ObjectiveData,
    /// Population size (minimum 15)
    pub pop_size: usize,
    /// Maximum iterations derived from maxeval and population
    pub max_iter: usize,
}

/// Set up common DE parameters
///
/// Converts bounds format, configures penalty weights, and estimates population/iteration parameters.
///
/// # Arguments
/// * `lower_bounds` - Lower bounds for each parameter
/// * `upper_bounds` - Upper bounds for each parameter
/// * `objective_data` - Base objective configuration
/// * `population` - Requested population size
/// * `maxeval` - Maximum function evaluations
/// * `qa_mode` - Whether to suppress debug output
///
/// # Returns
/// Configured DESetup with all common parameters
pub fn setup_de_common(
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    population: usize,
    maxeval: usize,
    qa_mode: bool,
) -> DESetup {
    // Convert bounds format for optde
    let bounds: Vec<(f64, f64)> = lower_bounds
        .iter()
        .zip(upper_bounds.iter())
        .map(|(&lo, &hi)| (lo, hi))
        .collect();

    // Estimate parameters
    let pop_size = population.max(15); // minimum reasonable population
    let max_iter = maxeval.min(pop_size * 10);

    // Set up objective data for DE with zero penalties since we use native constraints
    let mut penalty_data = objective_data.clone();
    penalty_data.configure_penalties(PenaltyMode::Disabled);

    // Log setup configuration (unless in QA mode)
    if !qa_mode {
        let params_desc = if penalty_data.loss_type == crate::LossType::DriversFlat {
            format!("{} parameters", bounds.len())
        } else {
            let params_per_filter = crate::param_utils::params_per_filter(penalty_data.peq_model);
            let num_filters = bounds.len() / params_per_filter;
            format!("{} filters", num_filters)
        };

        eprintln!(
            "DE Setup: {}, pop_size={}, max_iter={}, maxeval={}",
            params_desc, pop_size, max_iter, maxeval
        );
        eprintln!(
            "  Penalty weights: ceiling={:.1e}, spacing={:.1e}, mingain={:.1e}",
            penalty_data.penalty_w_ceiling,
            penalty_data.penalty_w_spacing,
            penalty_data.penalty_w_mingain
        );
        eprintln!(
            "  Constraints: max_db={:.1}, min_spacing={:.3} oct, min_db={:.1}",
            penalty_data.max_db, penalty_data.min_spacing_oct, penalty_data.min_db
        );
    }

    DESetup {
        bounds,
        penalty_data,
        pop_size,
        max_iter,
    }
}

/// Create progress reporting callback - print every 100 iterations
///
/// Creates a callback function that prints optimization progress at regular intervals.
///
/// # Arguments
/// * `algo_name` - Algorithm name to display in progress messages
/// * `qa_mode` - Whether to suppress all output
///
/// # Returns
/// Boxed callback function for DE optimization
pub fn create_de_callback(
    algo_name: &str,
    qa_mode: bool,
) -> Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send> {
    let name = algo_name.to_string();
    let mut tracker = ProgressTracker::default();

    Box::new(move |intermediate: &DEIntermediate| -> CallbackAction {
        let (improvement, _) = tracker.update(intermediate.fun);

        // Print when stalling (unless in QA mode)
        if !qa_mode && (tracker.just_started_stalling() || tracker.stall_at_interval(25)) {
            eprintln!(
                "{} iter {:4}  fitness={:.6e} {} conv={:.3e}",
                name, intermediate.iter, intermediate.fun, improvement, intermediate.convergence
            );
        }

        // Show parameter details every 100 iterations (unless in QA mode)
        if !qa_mode && intermediate.iter.is_multiple_of(100) {
            let summary = format_param_summary(intermediate.x.as_slice().unwrap(), 3);
            eprintln!("  --> Best params: {}", summary);
        }

        CallbackAction::Continue
    })
}

/// Create objective function for DE optimization
///
/// Wraps the penalty-based fitness computation for use with the optde library.
///
/// # Arguments
/// * `penalty_data` - Objective data with penalty weights configured
///
/// # Returns
/// Closure that computes fitness from ndarray parameter vector
pub fn create_de_objective(penalty_data: ObjectiveData) -> impl Fn(&Array1<f64>) -> f64 {
    move |x_arr: &Array1<f64>| -> f64 {
        let x_slice = x_arr.as_slice().unwrap();
        let mut data_copy = penalty_data.clone();
        compute_fitness_penalties(x_slice, None, &mut data_copy)
    }
}

/// Register a nonlinear inequality constraint with the DE config.
///
/// This helper reduces boilerplate when adding constraints to DE optimization.
/// The constraint is feasible when the constraint function returns <= 0.
///
/// # Type Parameters
/// * `T` - Constraint data type (must be Clone + Send + Sync + 'static)
/// * `F` - Constraint function type
fn register_de_constraint<T, F>(config: &mut DEConfig, constraint_fn: F, data: T)
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&[f64], Option<&mut [f64]>, &mut T) -> f64 + Send + Sync + 'static,
{
    let constraint = NonlinearConstraintHelper {
        fun: Arc::new(move |x: &Array1<f64>| {
            let mut result = Array1::zeros(1);
            let mut data = data.clone();
            result[0] = constraint_fn(x.as_slice().unwrap(), None, &mut data);
            result
        }),
        // Use large finite value instead of -inf to avoid bug in apply_to()
        // where inf tolerance causes incorrect equality constraint handling
        lb: Array1::from(vec![-1e30]),
        ub: Array1::from(vec![0.0]),
    };
    constraint.apply_to(config, 1e3, 1e3);
}

/// Process DE optimization results
///
/// Copies optimized parameters back to input array and formats status message.
///
/// # Arguments
/// * `x` - Mutable parameter array to update with optimized values
/// * `result` - DE optimization result containing optimal parameters and status
/// * `algo_name` - Algorithm name for status message formatting
///
/// # Returns
/// Result tuple with (status_message, objective_value)
pub fn process_de_results(
    x: &mut [f64],
    result: DEReport,
    algo_name: &str,
) -> Result<(String, f64), (String, f64)> {
    // Copy results back to input array
    if result.x.len() == x.len() {
        for (i, &value) in result.x.iter().enumerate() {
            x[i] = value;
        }
    }

    let status = if result.success {
        format!("AutoEQ {}: {}", algo_name, result.message)
    } else {
        format!("AutoEQ {}: {} (not converged)", algo_name, result.message)
    };

    Ok((status, result.fun))
}

/// Optimize filter parameters using AutoEQ custom algorithms
pub fn optimize_filters_autoeq(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    autoeq_name: &str,
    cli_args: &crate::cli::Args,
) -> Result<(String, f64), (String, f64)> {
    // Create the callback with all the logging and user feedback
    let callback = create_de_callback("autoeq::DE", cli_args.qa.is_some());

    // Delegate to the callback-based version
    optimize_filters_autoeq_with_callback(
        x,
        lower_bounds,
        upper_bounds,
        objective_data,
        autoeq_name,
        cli_args,
        callback,
    )
}

/// AutoEQ DE optimization with external progress callback
pub fn optimize_filters_autoeq_with_callback(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    _autoeq_name: &str,
    cli_args: &crate::cli::Args,
    mut callback: Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send>,
) -> Result<(String, f64), (String, f64)> {
    // Extract parameters from args
    let population = cli_args.population;
    let maxeval = cli_args.maxeval;

    // Reuse same setup as standard AutoEQ DE
    let setup = setup_de_common(
        lower_bounds,
        upper_bounds,
        objective_data.clone(),
        population,
        maxeval,
        cli_args.qa.is_some(),
    );
    let base_objective_fn = create_de_objective(setup.penalty_data.clone());

    // Create smart initialization based on frequency response analysis
    // Skip for drivers-flat loss as it uses a different parameter layout
    let smart_guesses = if setup.penalty_data.loss_type == crate::LossType::DriversFlat {
        Vec::new()
    } else {
        let params_per_filter =
            crate::param_utils::params_per_filter(cli_args.effective_peq_model());
        let num_filters = x.len() / params_per_filter;
        let smart_config = SmartInitConfig {
            seed: cli_args.seed, // Pass seed for deterministic initialization
            ..SmartInitConfig::default()
        };

        // Use the inverted target as the response to analyze for problems
        let target_response = &setup.penalty_data.deviation;
        let freq_grid = &setup.penalty_data.freqs;

        if cli_args.qa.is_none() {
            eprintln!(
                "🧠 Generating smart initial guesses based on frequency response analysis..."
            );
        }
        let guesses = create_smart_initial_guesses(
            target_response,
            freq_grid,
            num_filters,
            &setup.bounds,
            &smart_config,
            cli_args.effective_peq_model(),
        );

        if cli_args.qa.is_none() {
            eprintln!("📊 Generated {} smart initial guesses", guesses.len());
        }
        guesses
    };

    // Generate Sobol quasi-random population for better space coverage
    let sobol_samples = init_sobol(
        x.len(),
        setup.pop_size.saturating_sub(smart_guesses.len()),
        &setup.bounds,
    );

    if cli_args.qa.is_none() {
        eprintln!(
            "🎯 Generated {} Sobol quasi-random samples",
            sobol_samples.len()
        );
    }

    // Use the best smart guess as initial x0, fall back to Sobol initialization
    let best_initial_guess = if !smart_guesses.is_empty() {
        // Use the first (best) smart guess
        Array1::from(smart_guesses[0].clone())
    } else if !sobol_samples.is_empty() {
        // Fallback to the first Sobol sample if no smart guesses
        Array1::from(sobol_samples[0].clone())
    } else {
        // Ultimate fallback: use current x as initial guess
        Array1::from(x.to_vec())
    };

    if cli_args.qa.is_none() {
        eprintln!("🚀 Using smart initial guess with Sobol population initialization");
    }

    // Parse strategy from CLI args
    use std::str::FromStr;
    let strategy = Strategy::from_str(&cli_args.strategy).unwrap_or_else(|_| {
        if cli_args.qa.is_none() {
            eprintln!(
                "⚠️ Warning: Invalid strategy '{}', falling back to CurrentToBest1Bin",
                cli_args.strategy
            );
        }
        Strategy::CurrentToBest1Bin
    });

    // Set up adaptive configuration if using adaptive strategies
    let adaptive_config = if matches!(strategy, Strategy::AdaptiveBin | Strategy::AdaptiveExp) {
        Some(crate::de::AdaptiveConfig {
            adaptive_mutation: true,
            wls_enabled: false,                      // Disable WLS for stability
            w_max: 0.8,                              // Reduce max weight for more stability
            w_min: 0.2,                              // Increase min weight for more stability
            w_f: cli_args.adaptive_weight_f * 0.5,   // Make adaptation even more conservative
            w_cr: cli_args.adaptive_weight_cr * 0.5, // Make adaptation even more conservative
            f_m: 0.6,                                // Start with slightly higher F
            cr_m: 0.5,                               // Start with slightly lower CR
            wls_prob: 0.0,                           // Completely disable WLS
            wls_scale: 0.0,                          // Completely disable WLS
        })
    } else {
        None
    };

    // Adjust tolerance for adaptive strategies (they need much more relaxed convergence)
    let (tolerance, atolerance) =
        if matches!(strategy, Strategy::AdaptiveBin | Strategy::AdaptiveExp) {
            // Use much more relaxed tolerances for adaptive strategies - they converge differently
            (cli_args.tolerance * 10.0, cli_args.atolerance * 10.0)
        } else {
            (cli_args.tolerance, cli_args.atolerance)
        };

    // Use constraint helpers for nonlinear constraints
    let mut config_builder = DEConfigBuilder::new()
        .maxiter(setup.max_iter)
        .popsize(setup.pop_size)
        .tol(tolerance)
        .atol(atolerance)
        .strategy(strategy)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .recombination(cli_args.recombination)
        .init(Init::LatinHypercube) // Use Latin Hypercube sampling for population
        .x0(best_initial_guess) // Use smart guess as initial best individual
        .disp(false)
        .callback(Box::new(move |intermediate| callback(intermediate)));

    // Add seed if provided for deterministic results
    if let Some(seed_value) = cli_args.seed {
        config_builder = config_builder.seed(seed_value);
        if cli_args.qa.is_none() {
            eprintln!("🎲 Using deterministic seed: {}", seed_value);
        }
    }

    // Add adaptive configuration if present
    if let Some(adaptive_cfg) = adaptive_config {
        config_builder = config_builder.adaptive(adaptive_cfg);
    }

    // Configure parallel evaluation
    let parallel_config = ParallelConfig {
        enabled: !cli_args.no_parallel,
        num_threads: if cli_args.parallel_threads == 0 {
            None // Use all available cores
        } else {
            Some(cli_args.parallel_threads)
        },
    };
    config_builder = config_builder.parallel(parallel_config);

    if !cli_args.no_parallel && cli_args.qa.is_none() {
        eprintln!(
            "🚄 Parallel evaluation enabled with {} threads",
            if cli_args.parallel_threads.eq(&0) {
                "all available".to_string()
            } else {
                cli_args.parallel_threads.to_string()
            }
        );
    }

    // Add native nonlinear constraints
    let mut config = config_builder
        .build()
        .map_err(|e| (format!("DE config build failed: {:?}", e), f64::INFINITY))?;

    // Register nonlinear constraints using helper
    if setup.penalty_data.max_db > 0.0 {
        register_de_constraint(
            &mut config,
            constraint_ceiling,
            CeilingConstraintData {
                freqs: setup.penalty_data.freqs.clone(),
                srate: setup.penalty_data.srate,
                max_db: setup.penalty_data.max_db,
                peq_model: setup.penalty_data.peq_model,
            },
        );
    }

    if setup.penalty_data.min_db > 0.0 {
        register_de_constraint(
            &mut config,
            constraint_min_gain,
            MinGainConstraintData {
                min_db: setup.penalty_data.min_db,
                peq_model: setup.penalty_data.peq_model,
            },
        );
    }

    if setup.penalty_data.min_spacing_oct > 0.0 {
        register_de_constraint(
            &mut config,
            constraint_spacing,
            SpacingConstraintData {
                min_spacing_oct: setup.penalty_data.min_spacing_oct,
                peq_model: setup.penalty_data.peq_model,
            },
        );
    }

    let result = differential_evolution(&base_objective_fn, &setup.bounds, config)
        .map_err(|e| (format!("DE optimization failed: {:?}", e), f64::INFINITY))?;
    process_de_results(x, result, "AutoDE")
}
