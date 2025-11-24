use autoeq_de::{
    CallbackAction, Crossover, DEConfigBuilder, Init, Mutation, ParallelConfig, Strategy,
    differential_evolution,
    function_registry::{FunctionRegistry, TestFunction},
};
use autoeq_testfunctions::{FunctionMetadata, get_function_metadata};
use clap::{Parser, ValueEnum};
use ndarray::Array1;
use std::fmt::Write as FmtWrite;
use std::process;
use std::str::FromStr;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(
    name = "run_autoeq_de",
    about = "Optimize AutoEQ differential evolution on a selected benchmark function"
)]
struct Cli {
    /// Name of the benchmark function to optimize (use --list-functions to see available options)
    #[arg(long)]
    function: Option<String>,

    /// Dimensionality of the problem (defaults to the function's recommended dimension)
    #[arg(long)]
    dim: Option<usize>,

    /// Maximum number of iterations for the optimizer
    #[arg(long, default_value_t = 1000)]
    maxiter: usize,

    /// Population size factor (total population = popsize * number_of_free_variables)
    #[arg(long, default_value_t = 20)]
    population: usize,

    /// Convergence tolerance on the population standard deviation
    #[arg(long, default_value_t = 1e-6)]
    tol: f64,

    /// Absolute convergence tolerance on the best fitness value
    #[arg(long, default_value_t = 0.0)]
    atol: f64,

    /// Differential evolution recombination (crossover) probability in [0, 1]
    #[arg(long, default_value_t = 0.9)]
    recombination: f64,

    /// Differential evolution strategy (e.g. best1bin, rand1bin, currenttobest1bin)
    #[arg(long, default_value = "currenttobest1bin")]
    strategy: String,

    /// Mutation configuration
    #[arg(long, value_enum, default_value_t = MutationChoice::Range)]
    mutation: MutationChoice,

    /// Mutation factor (used by factor and adaptive mutation types)
    #[arg(long, default_value_t = 0.8)]
    mutation_factor: f64,

    /// Minimum mutation factor (used by range mutation type)
    #[arg(long, default_value_t = 0.4)]
    mutation_min: f64,

    /// Maximum mutation factor (used by range mutation type)
    #[arg(long, default_value_t = 1.2)]
    mutation_max: f64,

    /// Crossover type to use
    #[arg(long, value_enum, default_value_t = CrossoverChoice::Binomial)]
    crossover: CrossoverChoice,

    /// Initialization scheme for the population
    #[arg(long, value_enum, default_value_t = InitChoice::LatinHypercube)]
    init: InitChoice,

    /// Optional random seed for reproducibility
    #[arg(long)]
    seed: Option<u64>,

    /// Print intermediate progress every N iterations (>= 1)
    #[arg(long, default_value_t = 10)]
    progress_every: usize,

    /// Stop the optimization after this many seconds (optional)
    #[arg(long)]
    max_seconds: Option<f64>,

    /// Disable parallel evaluation of the population
    #[arg(long)]
    no_parallel: bool,

    /// Number of threads for parallel evaluation (0 = use all available cores)
    #[arg(long, default_value_t = 0)]
    threads: usize,

    /// List all available functions and exit
    #[arg(long)]
    list_functions: bool,

    /// Show metadata for the selected function before running optimization
    #[arg(long)]
    show_metadata: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum MutationChoice {
    Factor,
    Range,
    Adaptive,
}

impl MutationChoice {
    fn to_mutation(self, factor: f64, min: f64, max: f64) -> Result<Mutation, String> {
        match self {
            MutationChoice::Factor => Ok(Mutation::Factor(factor)),
            MutationChoice::Adaptive => Ok(Mutation::Adaptive { initial_f: factor }),
            MutationChoice::Range => {
                if !(0.0..=2.0).contains(&min) || !(0.0..=2.0).contains(&max) {
                    return Err(format!(
                        "Mutation range bounds must lie within [0, 2]; got min={min}, max={max}"
                    ));
                }
                if min >= max {
                    return Err(format!(
                        "Mutation range requires min < max; got min={min}, max={max}"
                    ));
                }
                Ok(Mutation::Range { min, max })
            }
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CrossoverChoice {
    Binomial,
    Exponential,
}

impl From<CrossoverChoice> for Crossover {
    fn from(choice: CrossoverChoice) -> Self {
        match choice {
            CrossoverChoice::Binomial => Crossover::Binomial,
            CrossoverChoice::Exponential => Crossover::Exponential,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum InitChoice {
    LatinHypercube,
    Random,
}

impl From<InitChoice> for Init {
    fn from(choice: InitChoice) -> Self {
        match choice {
            InitChoice::LatinHypercube => Init::LatinHypercube,
            InitChoice::Random => Init::Random,
        }
    }
}

fn main() {
    let args = Cli::parse();

    let registry = FunctionRegistry::new();

    if args.list_functions {
        list_available_functions(&registry);
        return;
    }

    let function_name = match &args.function {
        Some(name) => name.trim(),
        None => {
            eprintln!("Error: --function must be provided unless --list-functions is used.");
            process::exit(2);
        }
    };

    let (resolved_name, function) = match resolve_function(&registry, function_name) {
        Some(resolved) => resolved,
        None => {
            eprintln!(
                "Error: function '{function_name}' not found. Use --list-functions to inspect available names."
            );
            process::exit(2);
        }
    };

    let metadata_map = get_function_metadata();
    let metadata = metadata_map.get(&resolved_name);

    if args.show_metadata {
        if let Some(meta) = metadata {
            print_metadata(meta);
        } else {
            eprintln!(
                "Warning: no metadata available for '{resolved_name}'. Using default bounds (-5, 5)."
            );
        }
    }

    let dimension = determine_dimension(&args, metadata);

    if dimension == 0 {
        eprintln!("Error: problem dimension must be greater than zero.");
        process::exit(2);
    }

    let bounds = determine_bounds(metadata, dimension);

    if !(0.0..=1.0).contains(&args.recombination) {
        eprintln!(
            "Error: --recombination must lie within [0, 1]; got {}",
            args.recombination
        );
        process::exit(2);
    }

    if args.progress_every == 0 {
        eprintln!("Error: --progress-every must be at least 1.");
        process::exit(2);
    }

    let strategy = Strategy::from_str(&args.strategy).unwrap_or_else(|err| {
        eprintln!("Error parsing strategy '{}': {}", args.strategy, err);
        process::exit(2);
    });

    let mutation = args
        .mutation
        .to_mutation(args.mutation_factor, args.mutation_min, args.mutation_max)
        .unwrap_or_else(|err| {
            eprintln!("Error: {err}");
            process::exit(2);
        });

    let crossover: Crossover = args.crossover.into();
    let init: Init = args.init.into();

    let parallel = ParallelConfig {
        enabled: !args.no_parallel,
        num_threads: if args.threads == 0 {
            None
        } else {
            Some(args.threads)
        },
    };

    let mut builder = DEConfigBuilder::new()
        .maxiter(args.maxiter)
        .popsize(args.population)
        .tol(args.tol)
        .atol(args.atol)
        .mutation(mutation)
        .recombination(args.recombination)
        .strategy(strategy)
        .crossover(crossover)
        .init(init)
        .disp(false)
        .parallel(parallel);

    if let Some(seed) = args.seed {
        builder = builder.seed(seed);
    }

    let overall_start = Instant::now();
    let progress_every = args.progress_every;
    let time_limit = args.max_seconds;
    let mut best_so_far = f64::INFINITY;

    builder = builder.callback(Box::new(move |intermediate| {
        if intermediate.fun < best_so_far {
            best_so_far = intermediate.fun;
        }

        if intermediate.iter == 1 || intermediate.iter % progress_every == 0 {
            let mut x_buffer = String::new();
            for (idx, value) in intermediate.x.iter().enumerate() {
                if idx > 0 {
                    x_buffer.push_str(", ");
                }
                let _ = write!(&mut x_buffer, "{value:.6}");
            }
            println!(
                "iter {:>5} | f(x) = {:>12.6e} | conv = {:>10.3e} | best = {:>12.6e}",
                intermediate.iter, intermediate.fun, intermediate.convergence, best_so_far
            );
            println!("            x = [{}]", x_buffer);
        }

        if let Some(limit) = time_limit {
            if overall_start.elapsed().as_secs_f64() >= limit {
                println!(
                    "Stopping early after {:.2} seconds (time limit reached)",
                    limit
                );
                return CallbackAction::Stop;
            }
        }

        CallbackAction::Continue
    }));

    println!(
        "Running AutoEQ DE on '{}' ({}D) with {:?} strategy...",
        resolved_name, dimension, strategy
    );

    let config = builder.build();
    let objective = |x: &Array1<f64>| (function)(x);

    let report = differential_evolution(&objective, &bounds, config);

    let elapsed = overall_start.elapsed();
    println!("\nOptimization completed in {:.2?}", elapsed);
    println!("Status: {}", report.message);
    println!(
        "Iterations: {} | Evaluations: {} | Success: {}",
        report.nit, report.nfev, report.success
    );
    println!("Best objective: {:.6e}", report.fun);

    let mut best_vector = String::new();
    for (idx, value) in report.x.iter().enumerate() {
        if idx > 0 {
            best_vector.push_str(", ");
        }
        let _ = write!(&mut best_vector, "{value:.6}");
    }
    println!("Best parameters: [{}]", best_vector);

    if !report.success {
        process::exit(1);
    }
}

fn list_available_functions(registry: &FunctionRegistry) {
    let mut names = registry.list_functions();
    names.sort();
    println!("Available test functions ({}):", names.len());
    for name in names {
        println!("- {name}");
    }
}

fn resolve_function(
    registry: &FunctionRegistry,
    requested: &str,
) -> Option<(String, TestFunction)> {
    if let Some(func) = registry.get(requested) {
        return Some((requested.to_string(), func));
    }

    let requested_lower = requested.to_lowercase();
    for name in registry.list_functions() {
        if name.to_lowercase() == requested_lower {
            if let Some(func) = registry.get(&name) {
                return Some((name, func));
            }
        }
    }
    None
}

fn determine_dimension(args: &Cli, metadata: Option<&FunctionMetadata>) -> usize {
    if let Some(dim) = args.dim {
        return dim;
    }

    if let Some(meta) = metadata {
        if let Some(&preferred) = meta.dimensions.first() {
            if preferred > 0 {
                return preferred;
            }
        }
        if !meta.bounds.is_empty() {
            return meta.bounds.len();
        }
    }

    2
}

fn determine_bounds(metadata: Option<&FunctionMetadata>, dim: usize) -> Vec<(f64, f64)> {
    const DEFAULT_BOUND: (f64, f64) = (-5.0, 5.0);

    match metadata {
        Some(meta) if !meta.bounds.is_empty() => {
            if meta.bounds.len() == dim {
                meta.bounds.clone()
            } else if meta.bounds.len() == 1 {
                vec![meta.bounds[0]; dim]
            } else if meta.bounds.len() > dim {
                meta.bounds[..dim].to_vec()
            } else {
                let mut bounds = Vec::with_capacity(dim);
                for i in 0..dim {
                    bounds.push(meta.bounds[i % meta.bounds.len()]);
                }
                bounds
            }
        }
        _ => vec![DEFAULT_BOUND; dim],
    }
}

fn print_metadata(meta: &FunctionMetadata) {
    println!("Function metadata:");
    println!("  Name: {}", meta.name);
    println!("  Description: {}", meta.description);
    println!("  Typical dimensions: {:?}", meta.dimensions);
    if !meta.bounds.is_empty() {
        let bounds: Vec<String> = meta
            .bounds
            .iter()
            .map(|(lo, hi)| format!("[{lo}, {hi}]"))
            .collect();
        println!("  Bounds: {}", bounds.join(", "));
    }
    println!("  Multimodal: {}", meta.multimodal);
    if !meta.global_minima.is_empty() {
        println!("  Known global minima:");
        for (coords, value) in &meta.global_minima {
            println!("    f({coords:?}) = {value}");
        }
    }
}
