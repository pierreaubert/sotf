// AutoEQ DE-specific optimization code

use ndarray::Array1;
use std::sync::Arc;

use super::backend::{AlgorithmType, ConstraintCapabilities, FilterOptimizer};
use super::callback::{ProgressTracker, format_param_summary};
use super::params::OptimParams as BackendOptimParams;
use super::{ObjectiveData, OptimProgressCallback, PenaltyMode, compute_fitness_penalties_ref};
use crate::constraints::{
    CeilingConstraintData, MinGainConstraintData, SpacingConstraintData, constraint_ceiling,
    constraint_min_gain, constraint_spacing,
};
use crate::de::init_sobol::init_sobol;
use crate::de::{
    CallbackAction, DEConfig, DEConfigBuilder, DEIntermediate, DEReport, Init, Mutation,
    NonlinearConstraintHelper, ParallelConfig, Strategy, differential_evolution,
};
use crate::initial_guess::{SmartInitConfig, create_smart_initial_guesses};

/// AutoEQ DE-backed `FilterOptimizer`. Single instance — name is `"autoeq:de"`
/// today; the strategy variants (best1bin, lshadebin, …) are picked from
/// `OptimParams::strategy` inside `optimize_filters_autoeq`.
pub struct AutoeqDeBackend {
    name: &'static str,
}

impl AutoeqDeBackend {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl FilterOptimizer for AutoeqDeBackend {
    fn name(&self) -> &'static str {
        self.name
    }
    fn library(&self) -> &'static str {
        "AutoEQ"
    }
    fn algorithm_type(&self) -> AlgorithmType {
        AlgorithmType::Global
    }
    fn capabilities(&self) -> ConstraintCapabilities {
        ConstraintCapabilities {
            nonlinear_ineq: true,
            nonlinear_eq: true,
            linear: true,
            iteration_callback: true,
            // Unused (nonlinear_ineq=true means install_constraints disables penalties)
            // but keep a sensible value for completeness.
            fallback_penalty_mode: PenaltyMode::Disabled,
        }
    }
    fn optimize(
        &self,
        x: &mut [f64],
        lower: &[f64],
        upper: &[f64],
        objective: ObjectiveData,
        params: &BackendOptimParams,
        callback: Option<OptimProgressCallback>,
    ) -> Result<(String, f64), (String, f64)> {
        match callback {
            Some(user_cb) => {
                // Adapt unified callback to DE's typed callback. EPA mid-run
                // computation (the original behaviour from optim.rs:1163-1211)
                // is performed by the caller wrapping `user_cb` before passing
                // it in — the trait stays generic; EPA is autoeq-loss-specific.
                let de_cb: Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send> = {
                    let mut user_cb = user_cb;
                    Box::new(move |im| user_cb(im.iter, im.fun, None))
                };
                optimize_filters_autoeq_with_callback(
                    x, lower, upper, objective, self.name, params, de_cb,
                )
            }
            None => optimize_filters_autoeq(x, lower, upper, objective, self.name, params),
        }
    }
}

/// Re-exported for the optim.rs wrapper that adds EPA progress to callbacks.
///
/// The unified [`FilterOptimizer::optimize`] passes callbacks through with
/// `epa = None`. The EPA-aware wrapper in [`super::optimize_filters_with_callback`]
/// builds its own typed `DEIntermediate` callback and calls this entry point
/// directly, bypassing the trait.
pub use self::optimize_filters_autoeq_with_callback as autoeq_de_with_callback_typed;

/// Common setup for DE-based optimization
///
/// Contains all the shared configuration parameters for both standard and adaptive DE algorithms.
pub struct DESetup {
    /// Parameter bounds as (lower, upper) tuples for optde
    pub bounds: Vec<(f64, f64)>,
    /// Objective data with penalty weights configured
    pub penalty_data: ObjectiveData,
    /// Population size multiplier for the DE engine
    pub pop_multiplier: usize,
    /// Actual population size after applying the multiplier to free parameters
    pub population_size: usize,
    /// Maximum iterations derived from maxeval and population
    pub max_iter: usize,
}

fn count_free_dimensions(lower_bounds: &[f64], upper_bounds: &[f64]) -> usize {
    lower_bounds
        .iter()
        .zip(upper_bounds.iter())
        .filter(|(lo, hi)| **hi > **lo)
        .count()
        .max(1)
}

/// Minimum number of DE generations to ensure adequate exploration when
/// the user's `maxeval` is large enough to afford it.
const MIN_DE_GENERATIONS: usize = 5000;

fn derive_de_budget(
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    population: usize,
    maxeval: usize,
) -> (usize, usize, usize) {
    let n_free = count_free_dimensions(lower_bounds, upper_bounds);
    let desired_population = population.max(1).min(maxeval.max(1));
    let pop_multiplier = desired_population.div_ceil(n_free).max(4);
    let population_size = pop_multiplier * n_free;

    // B6 — respect the user's `maxeval` budget. The previous behaviour
    // was `(... / pop_size).max(MIN_DE_GENERATIONS)`, which silently
    // over-spent when `maxeval < MIN_DE_GENERATIONS * population_size`
    // (e.g. `maxeval=500 population=500` produced 5000 generations,
    // ~2.5 M evals — ten times the user-specified budget). We now
    // only apply the floor when the user's budget can actually afford
    // it; otherwise we run the computed number of generations and log
    // a warning so QA / benchmark runs can see the disagreement.
    let computed = maxeval.saturating_sub(population_size) / population_size;
    let budget_supports_floor = maxeval >= MIN_DE_GENERATIONS.saturating_mul(population_size);
    let max_iter = if budget_supports_floor {
        computed.max(MIN_DE_GENERATIONS)
    } else {
        // The `.max(1)` guarantees at least one generation runs so the
        // optimiser produces a result. A second cap by
        // `maxeval / population_size` prevents the total eval count
        // (initial population + N generations) from drifting past the
        // user's budget when `computed == 0` (i.e. maxeval ≤ pop_size).
        let budget_generations = maxeval / population_size.max(1);
        let capped = computed.max(1).min(budget_generations.max(1));
        log::warn!(
            "DE maxeval={} with population_size={} is below MIN_DE_GENERATIONS × pop = {}. \
             Running {} generations (≈{} evals) instead of the usual {} floor — expect \
             degraded convergence. Increase maxeval to {} or more to regain full exploration.",
            maxeval,
            population_size,
            MIN_DE_GENERATIONS.saturating_mul(population_size),
            capped,
            capped
                .saturating_mul(population_size)
                .saturating_add(population_size),
            MIN_DE_GENERATIONS,
            MIN_DE_GENERATIONS.saturating_mul(population_size),
        );
        capped
    };
    (pop_multiplier, population_size, max_iter)
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
    let (pop_multiplier, population_size, max_iter) =
        derive_de_budget(lower_bounds, upper_bounds, population, maxeval);

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

        log::debug!(
            "DE Setup: {}, pop_multiplier={}, population_size={}, max_iter={}, maxeval={}",
            params_desc,
            pop_multiplier,
            population_size,
            max_iter,
            maxeval
        );
        log::debug!(
            "  Penalty weights: ceiling={:.1e}, spacing={:.1e}, mingain={:.1e}",
            penalty_data.penalty_w_ceiling,
            penalty_data.penalty_w_spacing,
            penalty_data.penalty_w_mingain
        );
        log::debug!(
            "  Constraints: max_db={:.1}, min_spacing={:.3} oct, min_db={:.1}",
            penalty_data.max_db,
            penalty_data.min_spacing_oct,
            penalty_data.min_db
        );
    }

    DESetup {
        bounds,
        penalty_data,
        pop_multiplier,
        population_size,
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
            log::debug!(
                "{} iter {:4}  fitness={:.6e} {} conv={:.3e}",
                name,
                intermediate.iter,
                intermediate.fun,
                improvement,
                intermediate.convergence
            );
        }

        // Show parameter details every 100 iterations (unless in QA mode)
        if !qa_mode && intermediate.iter.is_multiple_of(100) {
            let summary = format_param_summary(intermediate.x.as_slice().unwrap(), 3);
            log::debug!("  --> Best params: {}", summary);
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
        compute_fitness_penalties_ref(x_slice, &penalty_data)
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
    params: &crate::OptimParams,
) -> Result<(String, f64), (String, f64)> {
    // Create the callback with all the logging and user feedback
    let callback = create_de_callback("autoeq::DE", params.quiet);

    // Delegate to the callback-based version
    optimize_filters_autoeq_with_callback(
        x,
        lower_bounds,
        upper_bounds,
        objective_data,
        autoeq_name,
        params,
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
    params: &crate::OptimParams,
    mut callback: Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send>,
) -> Result<(String, f64), (String, f64)> {
    // Extract parameters from args
    let population = params.population;
    let maxeval = params.maxeval;

    // Reuse same setup as standard AutoEQ DE
    let setup = setup_de_common(
        lower_bounds,
        upper_bounds,
        objective_data.clone(),
        population,
        maxeval,
        params.quiet,
    );
    let base_objective_fn = create_de_objective(setup.penalty_data.clone());

    // Create smart initialization based on frequency response analysis
    // Skip for drivers-flat loss as it uses a different parameter layout
    let smart_guesses = if matches!(
        setup.penalty_data.loss_type,
        crate::LossType::DriversFlat | crate::LossType::MultiSubFlat
    ) {
        Vec::new()
    } else {
        let params_per_filter = crate::param_utils::params_per_filter(params.peq_model);
        let num_filters = x.len() / params_per_filter;
        // If the caller (typically roomeq's `prepare_single_channel_eq`)
        // already detected high-quality room-mode problems via SSIR /
        // decomposed correction, feed them into the smart-guess
        // generator instead of letting it run its own cruder
        // find_peaks over the smoothed deviation. Empty list → fall
        // back to the legacy auto-detection.
        let pre_detected_problems = setup.penalty_data.detected_problems.clone();
        if !pre_detected_problems.is_empty() && !params.quiet {
            log::debug!(
                "🎯 Seeding smart initial guesses with {} pre-detected problem(s) from upstream analysis",
                pre_detected_problems.len()
            );
        }
        let smart_config = SmartInitConfig {
            seed: params.seed, // Pass seed for deterministic initialization
            pre_detected_problems,
            ..SmartInitConfig::default()
        };

        // Use the deviation curve (target - measurement) to identify problems.
        // Positive deviation = needs boost, negative = needs cut.
        let target_response = &setup.penalty_data.deviation;
        let freq_grid = &setup.penalty_data.freqs;

        if !params.quiet {
            log::debug!(
                "🧠 Generating smart initial guesses based on frequency response analysis..."
            );
        }
        let guesses = create_smart_initial_guesses(
            target_response,
            freq_grid,
            num_filters,
            &setup.bounds,
            &smart_config,
            params.peq_model,
        );

        if !params.quiet {
            log::debug!("📊 Generated {} smart initial guesses", guesses.len());
        }
        guesses
    };

    // Generate Sobol quasi-random population for better space coverage
    let sobol_samples = init_sobol(
        x.len(),
        setup.population_size.saturating_sub(smart_guesses.len()),
        &setup.bounds,
    );

    if !params.quiet {
        log::debug!(
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

    if !params.quiet {
        log::debug!("🚀 Using smart initial guess with Sobol population initialization");
    }

    // Parse strategy from CLI args
    use std::str::FromStr;
    let strategy = Strategy::from_str(&params.strategy).unwrap_or_else(|_| {
        if !params.quiet {
            log::debug!(
                "⚠️ Warning: Invalid strategy '{}', falling back to CurrentToBest1Bin",
                params.strategy
            );
        }
        Strategy::CurrentToBest1Bin
    });

    // Set up adaptive configuration if using adaptive strategies
    let adaptive_config = if matches!(strategy, Strategy::AdaptiveBin | Strategy::AdaptiveExp) {
        Some(crate::de::AdaptiveConfig {
            adaptive_mutation: true,
            wls_enabled: false,                    // Disable WLS for stability
            w_max: 0.8,                            // Reduce max weight for more stability
            w_min: 0.2,                            // Increase min weight for more stability
            w_f: params.adaptive_weight_f * 0.5,   // Make adaptation even more conservative
            w_cr: params.adaptive_weight_cr * 0.5, // Make adaptation even more conservative
            f_m: 0.6,                              // Start with slightly higher F
            cr_m: 0.5,                             // Start with slightly lower CR
            wls_prob: 0.0,                         // Completely disable WLS
            wls_scale: 0.0,                        // Completely disable WLS
        })
    } else {
        None
    };

    // Adjust tolerance for adaptive strategies (they need much more relaxed convergence)
    let (tolerance, atolerance) =
        if matches!(strategy, Strategy::AdaptiveBin | Strategy::AdaptiveExp) {
            // Use much more relaxed tolerances for adaptive strategies - they converge differently
            (params.tolerance * 10.0, params.atolerance * 10.0)
        } else {
            (params.tolerance, params.atolerance)
        };

    // Use constraint helpers for nonlinear constraints
    let mut config_builder = DEConfigBuilder::new()
        .maxiter(setup.max_iter)
        .popsize(setup.pop_multiplier)
        .tol(tolerance)
        .atol(atolerance)
        .strategy(strategy)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .recombination(params.recombination)
        .init(Init::LatinHypercube) // Use Latin Hypercube sampling for population
        .x0(best_initial_guess) // Use smart guess as initial best individual
        .disp(false)
        .callback(Box::new(move |intermediate| callback(intermediate)));

    // Add seed if provided for deterministic results
    if let Some(seed_value) = params.seed {
        config_builder = config_builder.seed(seed_value);
        if !params.quiet {
            log::debug!("🎲 Using deterministic seed: {}", seed_value);
        }
    }

    // Add adaptive configuration if present
    if let Some(adaptive_cfg) = adaptive_config {
        config_builder = config_builder.adaptive(adaptive_cfg);
    }

    // Configure parallel evaluation
    let parallel_config = ParallelConfig {
        enabled: !params.no_parallel,
        num_threads: if params.parallel_threads == 0 {
            None // Use all available cores
        } else {
            Some(params.parallel_threads)
        },
    };
    config_builder = config_builder.parallel(parallel_config);

    if !params.no_parallel && !params.quiet {
        log::debug!(
            "🚄 Parallel evaluation enabled with {} threads",
            if params.parallel_threads.eq(&0) {
                "all available".to_string()
            } else {
                params.parallel_threads.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LossType;
    use crate::cli::PeqModel;
    use ndarray::{Array1, array};

    fn test_objective_data() -> ObjectiveData {
        ObjectiveData {
            freqs: array![100.0, 1000.0],
            target: Array1::zeros(2),
            deviation: Array1::zeros(2),
            srate: 48_000.0,
            min_spacing_oct: 0.0,
            spacing_weight: 0.0,
            max_db: 6.0,
            min_db: 0.0,
            min_freq: 20.0,
            max_freq: 20_000.0,
            peq_model: PeqModel::Pk,
            loss_type: LossType::SpeakerFlat,
            speaker_score_data: None,
            headphone_score_data: None,
            input_curve: None,
            drivers_data: None,
            fixed_crossover_freqs: None,
            penalty_w_ceiling: 0.0,
            penalty_w_spacing: 0.0,
            penalty_w_mingain: 0.0,
            integrality: None,
            multi_objective: None,
            smooth: false,
            smooth_n: 2,
            max_boost_envelope: None,
            min_cut_envelope: None,
            epa_config: None,
            detected_problems: Vec::new(),
            null_suppression: None,
            smoothness_penalty: None,
        }
    }

    #[test]
    fn setup_de_common_clamps_to_maxeval_when_budget_is_small() {
        // B6 — when maxeval is smaller than MIN_DE_GENERATIONS × pop_size,
        // the floor is disabled so user intent is honoured.
        let lower_bounds = vec![-1.0, -1.0];
        let upper_bounds = vec![1.0, 1.0];
        let setup = setup_de_common(
            &lower_bounds,
            &upper_bounds,
            test_objective_data(),
            20,
            55,
            true,
        );

        assert_eq!(setup.population_size, 20);
        assert!(
            setup.max_iter < MIN_DE_GENERATIONS,
            "tiny maxeval must cap max_iter below the floor, got {}",
            setup.max_iter,
        );
        assert!(setup.max_iter >= 1);
        // Review P3: total evals must not drift past 2 × maxeval.
        let total_evals = setup.population_size + setup.max_iter * setup.population_size;
        assert!(
            total_evals <= 2 * 55,
            "total evals {} must not exceed 2 × maxeval (={})",
            total_evals,
            2 * 55
        );
    }

    #[test]
    fn setup_de_common_honours_maxeval_equal_to_popsize() {
        // Review edge case: maxeval == pop_size. Previously `computed.max(1)`
        // ran 1 generation × pop_size evals + pop_size seed = 2 × budget.
        let lower_bounds = vec![-1.0, -1.0];
        let upper_bounds = vec![1.0, 1.0];
        let setup = setup_de_common(
            &lower_bounds,
            &upper_bounds,
            test_objective_data(),
            20,
            20,
            true,
        );
        assert_eq!(setup.max_iter, 1);
    }

    #[test]
    fn setup_de_common_respects_large_maxeval() {
        let lower_bounds = vec![-1.0, -1.0, -1.0];
        let upper_bounds = vec![1.0, 1.0, 1.0];
        let setup = setup_de_common(
            &lower_bounds,
            &upper_bounds,
            test_objective_data(),
            20,
            1_000_000,
            true,
        );

        // With large maxeval, computed generations should exceed MIN_DE_GENERATIONS
        assert!(setup.max_iter >= MIN_DE_GENERATIONS);
        // Check actual generation count: (1_000_000 - pop_size) / pop_size
        let expected = (1_000_000 - setup.population_size) / setup.population_size;
        assert_eq!(setup.max_iter, expected);
    }
}
