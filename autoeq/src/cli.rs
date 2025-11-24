//! AutoEQ - A library for audio equalization and filter optimization
//! Common command-line interface definitions shared across binaries
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use super::optim::{AlgorithmType, get_all_algorithms};
use crate::LossType;
use crate::de::Strategy;
use clap::{Parser, ValueEnum};
use std::fmt;
use std::path::PathBuf;
use std::process;

/// PEQ model types that define the structure and constraints of the equalizer
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PeqModel {
    /// All filters are peak filters
    #[value(name = "pk")]
    Pk,
    /// First filter is highpass, rest are peak filters
    #[value(name = "hp-pk")]
    HpPk,
    /// First filter is highpass, last is lowpass, rest are peak filters
    #[value(name = "hp-pk-lp")]
    HpPkLp,
    /// First filter is low shelve, rest are peak filters
    #[value(name = "ls-pk")]
    LsPk,
    /// First filter is low shelve, last is high shelve, rest are peak filters
    #[value(name = "ls-pk-hs")]
    LsPkHs,
    /// First and last filters are free (any type), rest are peak filters
    #[value(name = "free-pk-free")]
    FreePkFree,
    /// All filters are free to be any type
    #[value(name = "free")]
    Free,
}

impl fmt::Display for PeqModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeqModel::Pk => write!(f, "pk"),
            PeqModel::HpPk => write!(f, "hp-pk"),
            PeqModel::LsPk => write!(f, "ls-pk"),
            PeqModel::HpPkLp => write!(f, "hp-pk-lp"),
            PeqModel::LsPkHs => write!(f, "ls-pk-hs"),
            PeqModel::FreePkFree => write!(f, "free-pk-free"),
            PeqModel::Free => write!(f, "free"),
        }
    }
}

impl PeqModel {
    /// Get all available PEQ models
    pub fn all() -> Vec<Self> {
        vec![
            PeqModel::Pk,
            PeqModel::HpPk,
            PeqModel::LsPk,
            PeqModel::HpPkLp,
            PeqModel::LsPkHs,
            PeqModel::FreePkFree,
            PeqModel::Free,
        ]
    }

    /// Get a description of the model
    pub fn description(&self) -> &'static str {
        match self {
            PeqModel::Pk => "All filters are peak/bell filters",
            PeqModel::HpPk => "First filter is highpass, rest are peak filters",
            PeqModel::LsPk => "First filter is low shelve, rest are peak filters",
            PeqModel::HpPkLp => "First filter is highpass, last is lowpass, rest are peak filters",
            PeqModel::LsPkHs => {
                "First filter is low shelve, last is high shelve, rest are peak filters"
            }
            PeqModel::FreePkFree => {
                "First and last filters can be any type, middle filters are peak"
            }
            PeqModel::Free => "All filters can be any type (peak, highpass, lowpass, shelf)",
        }
    }
}

/// Shared CLI arguments for AutoEQ binaries.
#[derive(Parser, Debug, Clone)]
#[command(author, about, long_about = None)]
pub struct Args {
    /// Number of IIR filters to use for optimization.
    #[arg(short = 'n', long, default_value_t = 7)]
    pub num_filters: usize,

    /// Path to the input curve CSV file (format: frequency,spl).
    /// Required unless speaker, version, and measurement are provided for API data.
    #[arg(short, long)]
    pub curve: Option<PathBuf>,

    /// Path to the optional target curve CSV file (format: frequency,spl).
    /// If not provided, a flat 0 dB target is assumed.
    #[arg(short, long)]
    pub target: Option<PathBuf>,

    /// The sample rate for the IIR filters.
    #[arg(short, long, default_value_t = 48000.0)]
    pub sample_rate: f64,

    /// Maximum absolute dB gain allowed for each filter.
    #[arg(long, default_value_t = 3.0, value_parser = parse_nonnegative_f64)]
    pub max_db: f64,

    /// Minimum absolute dB gain allowed for each filter.
    #[arg(long, default_value_t = 1.0, value_parser = parse_strictly_positive_f64)]
    pub min_db: f64,

    /// Maximum Q factor allowed for each filter.
    #[arg(long, default_value_t = 3.0)]
    pub max_q: f64,

    /// Minimum Q factor allowed for each filter.
    #[arg(long, default_value_t = 1.0)]
    pub min_q: f64,

    /// Minimum frequency allowed for each filter.
    #[arg(long, default_value_t = 60.0)]
    pub min_freq: f64,

    /// Maximum frequency allowed for each filter.
    #[arg(long, default_value_t = 16000.0)]
    pub max_freq: f64,

    /// Output PNG file for plotting results.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Speaker name for API data fetching.
    #[arg(long)]
    pub speaker: Option<String>,

    /// Version for API data fetching.
    #[arg(long)]
    pub version: Option<String>,

    /// Measurement type for API data fetching.
    #[arg(long)]
    pub measurement: Option<String>,

    /// Curve name inside CEA2034 plots to use (only when --measurement CEA2034)
    /// e.g., "Listening Window", "On Axis", "Early Reflections". Default: Listening Window
    #[arg(long, default_value = "Listening Window")]
    pub curve_name: String,

    /// Optimization algorithm to use (e.g., isres, cobyla)
    #[arg(long, default_value = "nlopt:cobyla")]
    pub algo: String,

    /// Optional population size for population-based algorithms (e.g., ISRES)
    #[arg(long, default_value_t = 300)]
    pub population: usize,

    /// Maximum number of evaluations for the optimizer
    #[arg(long, default_value_t = 2_000)]
    pub maxeval: usize,

    /// Whether to run a local refinement after global optimization
    #[arg(long, default_value_t = false)]
    pub refine: bool,

    /// Local optimizer to use for refinement (e.g., cobyla)
    #[arg(long, default_value = "cobyla")]
    pub local_algo: String,

    /// Minimum spacing between filter center frequencies in octaves (0 disables)
    #[arg(long, default_value_t = 0.2)]
    pub min_spacing_oct: f64,

    /// Weight for the spacing penalty in the objective function
    #[arg(long, default_value_t = 20.0)]
    pub spacing_weight: f64,

    /// Enable smoothing (regularization) of the inverted target curve
    #[arg(long, default_value_t = true)]
    pub smooth: bool,

    /// Smoothing level as 1/N octave (N in [1..24]). Example: N=6 => 1/6 octave smoothing
    #[arg(long, default_value_t = 2)]
    pub smooth_n: usize,

    /// Loss function to optimize (flat or score).
    #[arg(long, value_enum, default_value_t = LossType::SpeakerFlat)]
    pub loss: LossType,

    /// PEQ model that defines the filter structure
    #[arg(long, value_enum, default_value_t = PeqModel::Pk)]
    pub peq_model: PeqModel,

    /// Display list of available PEQ models with descriptions and exit.
    #[arg(long, default_value_t = false)]
    pub peq_model_list: bool,

    /// Display list of available optimization algorithms with descriptions and exit.
    #[arg(long, default_value_t = false)]
    pub algo_list: bool,

    /// Optimization tolerance (tol parameter for DE algorithm)
    #[arg(long, default_value_t = 1e-3)]
    pub tolerance: f64,

    /// Absolute tolerance (atol parameter for DE algorithm)
    #[arg(long, default_value_t = 1e-4)]
    pub atolerance: f64,

    /// Recombination probability for DE algorithm (0.0 to 1.0)
    #[arg(long, default_value_t = 0.9, value_parser = parse_recombination_probability)]
    pub recombination: f64,

    /// DE strategy to use (e.g., best1bin, rand1bin, currenttobest1bin, adaptive)
    #[arg(long, default_value = "currenttobest1bin")]
    pub strategy: String,

    /// Display list of available DE strategies and exit.
    #[arg(long, default_value_t = false)]
    pub strategy_list: bool,

    /// Adaptive weight for F parameter (DE adaptive strategies only)
    #[arg(long, default_value_t = 0.9)]
    pub adaptive_weight_f: f64,

    /// Adaptive weight for CR parameter (DE adaptive strategies only)
    #[arg(long, default_value_t = 0.9)]
    pub adaptive_weight_cr: f64,

    /// Disable parallel evaluation for DE algorithm (default: parallel is enabled)
    #[arg(long = "no-parallel", default_value_t = false)]
    pub no_parallel: bool,

    /// Number of threads to use for parallel evaluation (0 = use all available cores)
    #[arg(long, default_value_t = 0)]
    pub parallel_threads: usize,

    /// Random seed for deterministic optimization (default: random seed for each run)
    /// Setting this makes optimization results reproducible
    #[arg(long)]
    pub seed: Option<u64>,

    /// Quality assurance mode with optional threshold: suppress normal output, show summary line and analysis
    /// If a threshold is provided (e.g., --qa 0.4), also perform QA analysis
    #[arg(long, value_name = "THRESHOLD")]
    pub qa: Option<f64>,

    /// Path to first driver measurement CSV file (for multi-driver optimization with --loss drivers-flat)
    #[arg(long)]
    pub driver1: Option<PathBuf>,

    /// Path to second driver measurement CSV file (for multi-driver optimization with --loss drivers-flat)
    #[arg(long)]
    pub driver2: Option<PathBuf>,

    /// Path to third driver measurement CSV file (for multi-driver optimization with --loss drivers-flat)
    #[arg(long)]
    pub driver3: Option<PathBuf>,

    /// Path to fourth driver measurement CSV file (for multi-driver optimization with --loss drivers-flat)
    #[arg(long)]
    pub driver4: Option<PathBuf>,

    /// Crossover type for multi-driver optimization (butterworth2, linkwitzriley2, linkwitzriley4)
    #[arg(long, default_value = "linkwitzriley4")]
    pub crossover_type: String,
}

impl Args {
    /// Get the effective PEQ model
    pub fn effective_peq_model(&self) -> PeqModel {
        self.peq_model
    }

    /// Check if the first filter should be a highpass (for compatibility)
    pub fn uses_highpass_first(&self) -> bool {
        matches!(
            self.effective_peq_model(),
            PeqModel::HpPk | PeqModel::HpPkLp
        )
    }
}

/// Display available optimization algorithms with descriptions and exit
pub fn display_algorithm_list() -> ! {
    println!("Available Optimization Algorithms");
    println!("=================================\n");

    let algorithms = get_all_algorithms();

    // Group algorithms by library
    let mut nlopt_algos = Vec::new();
    let mut metaheuristics_algos = Vec::new();
    let mut autoeq_algos = Vec::new();

    for algo in &algorithms {
        match algo.library {
            "NLOPT" => nlopt_algos.push(algo),
            "Metaheuristics" => metaheuristics_algos.push(algo),
            "AutoEQ" => autoeq_algos.push(algo),
            _ => {} // Skip unknown libraries
        }
    }

    // Display NLOPT algorithms
    if !nlopt_algos.is_empty() {
        println!("📊 NLOPT Library Algorithms:");

        // Separate global and local algorithms
        let mut global = Vec::new();
        let mut local = Vec::new();

        for algo in nlopt_algos {
            match algo.algorithm_type {
                AlgorithmType::Global => global.push(algo),
                AlgorithmType::Local => local.push(algo),
            }
        }

        if !global.is_empty() {
            println!("   🌍 Global Optimizers (best for exploring solution space):");
            for algo in global {
                print!("   - {:<20}", algo.name);
                print!(" | Constraints: ");
                if algo.supports_nonlinear_constraints {
                    print!("✅ Nonlinear");
                } else if algo.supports_linear_constraints {
                    print!("🔶 Linear only");
                } else {
                    print!("❌ None");
                }

                // Add specific descriptions
                let description = match algo.name {
                    "nlopt:isres" => {
                        " | Improved Stochastic Ranking Evolution Strategy (recommended)"
                    }
                    "nlopt:ags" => " | Adaptive Geometric Search",
                    "nlopt:origdirect" => " | DIRECT global optimization (original version)",
                    "nlopt:crs2lm" => " | Controlled Random Search with local mutation",
                    "nlopt:direct" => " | DIRECT global optimization",
                    "nlopt:directl" => " | DIRECT-L (locally biased version)",
                    "nlopt:gmlsl" => " | Global Multi-Level Single-Linkage",
                    "nlopt:gmlsllds" => " | GMLSL with low-discrepancy sequence",
                    "nlopt:stogo" => " | Stochastic Global Optimization",
                    "nlopt:stogorand" => " | StoGO with randomized search",
                    _ => "",
                };
                println!("{}", description);
            }
            println!();
        }

        if !local.is_empty() {
            println!("   🎯 Local Optimizers (fast refinement from good starting points):");
            for algo in local {
                print!("   - {:<20}", algo.name);
                print!(" | Constraints: ");
                if algo.supports_nonlinear_constraints {
                    print!("✅ Nonlinear");
                } else if algo.supports_linear_constraints {
                    print!("🔶 Linear only");
                } else {
                    print!("❌ None");
                }

                let description = match algo.name {
                    "nlopt:cobyla" => {
                        " | Constrained Optimization BY Linear Approximations (recommended for local)"
                    }
                    "nlopt:bobyqa" => " | Bound Optimization BY Quadratic Approximation",
                    "nlopt:neldermead" => " | Nelder-Mead simplex algorithm",
                    "nlopt:sbplx" => " | Subplex (variant of Nelder-Mead)",
                    "nlopt:slsqp" => " | Sequential Least SQuares Programming",
                    _ => "",
                };
                println!("{}", description);
            }
            println!();
        }
    }

    // Display Metaheuristics algorithms
    if !metaheuristics_algos.is_empty() {
        println!("🧬 Metaheuristics Library Algorithms:");
        println!("   Nature-inspired global optimization (penalty-based constraints)\n");

        for algo in metaheuristics_algos {
            print!("   - {:<20}", algo.name);
            let description = match algo.name {
                "mh:de" => " | Differential Evolution (robust, good convergence)",
                "mh:pso" => " | Particle Swarm Optimization (fast exploration)",
                "mh:rga" => " | Real-coded Genetic Algorithm (diverse search)",
                "mh:tlbo" => " | Teaching-Learning-Based Optimization (parameter-free)",
                "mh:firefly" => " | Firefly Algorithm (multi-modal problems)",
                _ => "",
            };
            println!("{}", description);
        }
        println!();
    }

    // Display AutoEQ algorithms
    if !autoeq_algos.is_empty() {
        println!("🎵 AutoEQ Custom Algorithms:");
        println!("   Specialized algorithms developed for audio filter optimization\n");

        for algo in autoeq_algos {
            print!("   - {:<20}", algo.name);
            print!(" | Constraints: ");
            if algo.supports_nonlinear_constraints {
                print!("✅ Nonlinear");
            } else {
                print!("❌ Penalty-based");
            }

            let description = match algo.name {
                "autoeq:de" => " | Adaptive DE with constraint handling (experimental)",
                _ => "",
            };
            println!("{}", description);
        }
        println!();
    }

    println!("Usage Examples:");
    println!("==============\n");
    println!("  # Use ISRES (recommended global optimizer):");
    println!("  autoeq --algo nlopt:isres --curve input.csv\n");
    println!("  # Use COBYLA (fast local optimizer):");
    println!("  autoeq --algo nlopt:cobyla --curve input.csv\n");
    println!("  # Use Differential Evolution from metaheuristics:");
    println!("  autoeq --algo mh:de --curve input.csv\n");
    println!("  # Backward compatibility (maps to nlopt:cobyla):");
    println!("  autoeq --algo cobyla --curve input.csv\n");

    println!("Recommendations:");
    println!("===============\n");
    println!("  🎯 For best results: nlopt:isres (global) + --refine with nlopt:cobyla (local)");
    println!("  ⚡ For speed: nlopt:cobyla (if you have a good initial guess)");
    println!("  🧪 For experimentation: mh:de or mh:pso from metaheuristics library");
    println!(
        "  ⚖️  For constrained problems: Prefer algorithms with ✅ Nonlinear constraint support"
    );

    process::exit(0);
}

/// Display available DE strategies with descriptions and exit
pub fn display_strategy_list() -> ! {
    println!("Available Differential Evolution (DE) Strategies");
    println!("===============================================\n");

    let strategies = [
        (
            "best1bin",
            "Best1Bin",
            "Use best individual + 1 random difference (binomial crossover)",
            "Global exploration with fast convergence",
        ),
        (
            "best1exp",
            "Best1Exp",
            "Use best individual + 1 random difference (exponential crossover)",
            "Similar to best1bin with different crossover",
        ),
        (
            "rand1bin",
            "Rand1Bin",
            "Use random individual + 1 random difference (binomial crossover)",
            "Good diversity, slower convergence",
        ),
        (
            "rand1exp",
            "Rand1Exp",
            "Use random individual + 1 random difference (exponential crossover)",
            "Similar to rand1bin with different crossover",
        ),
        (
            "rand2bin",
            "Rand2Bin",
            "Use random individual + 2 random differences (binomial crossover)",
            "High exploration, may be slower",
        ),
        (
            "rand2exp",
            "Rand2Exp",
            "Use random individual + 2 random differences (exponential crossover)",
            "Similar to rand2bin with different crossover",
        ),
        (
            "currenttobest1bin",
            "CurrentToBest1Bin",
            "Blend current with best + random difference (binomial)",
            "Balanced exploration/exploitation (recommended)",
        ),
        (
            "currenttobest1exp",
            "CurrentToBest1Exp",
            "Blend current with best + random difference (exponential)",
            "Similar to currenttobest1bin",
        ),
        (
            "best2bin",
            "Best2Bin",
            "Use best individual + 2 random differences (binomial crossover)",
            "Fast convergence, may get trapped locally",
        ),
        (
            "best2exp",
            "Best2Exp",
            "Use best individual + 2 random differences (exponential crossover)",
            "Similar to best2bin",
        ),
        (
            "randtobest1bin",
            "RandToBest1Bin",
            "Blend random with best + random difference (binomial)",
            "Good balance of diversity and convergence",
        ),
        (
            "randtobest1exp",
            "RandToBest1Exp",
            "Blend random with best + random difference (exponential)",
            "Similar to randtobest1bin",
        ),
        (
            "adaptivebin",
            "AdaptiveBin",
            "Self-adaptive mutation with top-w% selection (binomial)",
            "Advanced adaptive strategy (experimental)",
        ),
        (
            "adaptiveexp",
            "AdaptiveExp",
            "Self-adaptive mutation with top-w% selection (exponential)",
            "Advanced adaptive strategy (experimental)",
        ),
    ];

    println!("🎯 Classic DE Strategies (well-tested, reliable):");
    for &(name, _enum_name, description, recommendation) in strategies.iter().take(12) {
        if name.starts_with("adaptive") {
            continue;
        }
        println!("   - {:<20} | {}", name, description);
        println!("     {:<20} | 💡 {}", "", recommendation);
        if name == "currenttobest1bin" {
            println!("     {:<20} | ⭐ Recommended default strategy", "");
        }
        println!();
    }

    println!("🧬 Adaptive DE Strategies (experimental, research-based):");
    for &(name, _enum_name, description, recommendation) in strategies.iter() {
        if !name.starts_with("adaptive") {
            continue;
        }
        println!("   - {:<20} | {}", name, description);
        println!("     {:<20} | 💡 {}", "", recommendation);
        println!(
            "     {:<20} | 🔧 Requires --adaptive-weight-f and --adaptive-weight-cr",
            ""
        );
        println!();
    }

    println!("Strategy Naming Conventions:");
    println!("==========================\n");
    println!("  • 'bin' = Binomial (uniform) crossover - each gene has equal probability");
    println!("  • 'exp' = Exponential crossover - contiguous segments are more likely");
    println!("  • Numbers (1, 2) indicate how many difference vectors are used\n");

    println!("Usage Examples:");
    println!("==============\n");
    println!("  # Use recommended default strategy:");
    println!("  autoeq --algo autoeq:de --strategy currenttobest1bin --curve input.csv\n");
    println!("  # Use adaptive strategy with custom weights:");
    println!(
        "  autoeq --algo autoeq:de --strategy adaptivebin --adaptive-weight-f 0.8 --adaptive-weight-cr 0.7\n"
    );
    println!("  # Use classic exploration strategy:");
    println!("  autoeq --algo autoeq:de --strategy rand1bin --curve input.csv\n");

    println!("Recommendations:");
    println!("===============\n");
    println!(
        "  ⭐ For general use: currenttobest1bin (good balance of exploration and exploitation)"
    );
    println!("  🚀 For fast convergence: best1bin or best2bin (may get trapped in local optima)");
    println!("  🌍 For thorough exploration: rand1bin or rand2bin (slower but more robust)");
    println!(
        "  🧪 For research/experimentation: adaptivebin or adaptiveexp (requires parameter tuning)"
    );

    process::exit(0);
}

/// Validate CLI arguments and exit with error message if validation fails
pub fn validate_args(args: &Args) -> Result<(), String> {
    // Check if strategy is valid when using DE algorithm
    if args.algo == "autoeq:de" || args.algo.contains("de") {
        use std::str::FromStr;
        if let Err(err) = Strategy::from_str(&args.strategy) {
            return Err(format!(
                "Invalid DE strategy '{}': {}. Use --strategy-list to see available strategies.",
                args.strategy, err
            ));
        }
    }
    // Check if algorithm is valid
    if crate::optim::find_algorithm_info(&args.algo).is_some() {
        // Algorithm is valid
    } else {
        return Err(format!(
            "Unknown algorithm: '{}'. Use --algo-list to see available algorithms.",
            args.algo
        ));
    }

    // Check if local algorithm is valid (when refine is enabled)
    if args.refine {
        if crate::optim::find_algorithm_info(&args.local_algo).is_some() {
            // Local algorithm is valid
        } else {
            return Err(format!(
                "Unknown local algorithm: '{}'. Use --algo-list to see available algorithms.",
                args.local_algo
            ));
        }
    }

    // Check min/max Q factor constraints
    if args.min_q > args.max_q {
        return Err(format!(
            "Invalid Q factor range: min_q ({}) must be <= max_q ({})",
            args.min_q, args.max_q
        ));
    }

    // Check min/max frequency constraints
    if args.min_freq > args.max_freq {
        return Err(format!(
            "Invalid frequency range: min_freq ({}) must be <= max_freq ({})",
            args.min_freq, args.max_freq
        ));
    }

    // Check min/max dB constraints
    if args.min_db > args.max_db {
        return Err(format!(
            "Invalid dB range: min_db ({}) must be <= max_db ({})",
            args.min_db, args.max_db
        ));
    }

    // Check frequency bounds (reasonable audio range)
    if args.min_freq < 20.0 {
        return Err(format!(
            "Invalid min_freq: {} Hz. Must be >= 20 Hz (reasonable audio range)",
            args.min_freq
        ));
    }

    if args.max_freq > 20000.0 {
        return Err(format!(
            "Invalid max_freq: {} Hz. Must be <= 20,000 Hz (reasonable audio range)",
            args.max_freq
        ));
    }

    // Check smoothing parameters
    if args.smooth_n < 1 || args.smooth_n > 24 {
        return Err(format!(
            "Invalid smooth_n: {}. Must be in range [1..24]",
            args.smooth_n
        ));
    }

    // Check that population size is reasonable
    if args.population == 0 {
        return Err("Population size must be > 0".to_string());
    }

    // Check that maxeval is reasonable
    if args.maxeval == 0 {
        return Err("Maximum evaluations must be > 0".to_string());
    }

    // Check that num_filters is reasonable
    if args.num_filters == 0 {
        return Err("Number of filters must be > 0".to_string());
    }

    if args.num_filters > 50 {
        return Err(format!(
            "Number of filters ({}) is very high. Consider using <= 50 filters for reasonable performance",
            args.num_filters
        ));
    }

    // Check tolerance parameters
    if args.tolerance <= 0.0 {
        return Err("Tolerance must be > 0".to_string());
    }

    if args.atolerance < 0.0 {
        return Err("Absolute tolerance must be >= 0".to_string());
    }

    // Check adaptive weight parameters (should be in [0, 1])
    if args.adaptive_weight_f < 0.0 || args.adaptive_weight_f > 1.0 {
        return Err("Adaptive weight for F must be between 0.0 and 1.0".to_string());
    }

    if args.adaptive_weight_cr < 0.0 || args.adaptive_weight_cr > 1.0 {
        return Err("Adaptive weight for CR must be between 0.0 and 1.0".to_string());
    }

    // Validate multi-driver arguments
    if args.loss == LossType::DriversFlat {
        // Check that at least driver1 and driver2 are provided
        if args.driver1.is_none() || args.driver2.is_none() {
            return Err("Multi-driver optimization requires at least --driver1 and --driver2 when using --loss drivers-flat".to_string());
        }

        // Check crossover type
        let valid_crossover_types = ["butterworth2", "linkwitzriley2", "linkwitzriley4"];
        if !valid_crossover_types.contains(&args.crossover_type.as_str()) {
            return Err(format!(
                "Invalid crossover type '{}'. Valid types: {}",
                args.crossover_type,
                valid_crossover_types.join(", ")
            ));
        }

        // Count number of drivers
        let n_drivers = [&args.driver1, &args.driver2, &args.driver3, &args.driver4]
            .iter()
            .filter(|d| d.is_some())
            .count();
        if n_drivers < 2 || n_drivers > 4 {
            return Err(format!(
                "Multi-driver optimization requires 2-4 drivers, got {}",
                n_drivers
            ));
        }
    } else {
        // If not using drivers-flat loss, driver arguments should not be provided
        if args.driver1.is_some()
            || args.driver2.is_some()
            || args.driver3.is_some()
            || args.driver4.is_some()
        {
            return Err("Driver arguments (--driver1, --driver2, etc.) can only be used with --loss drivers-flat".to_string());
        }
    }

    Ok(())
}

/// Validate arguments and exit with error if validation fails
pub fn validate_args_or_exit(args: &Args) {
    if let Err(error) = validate_args(args) {
        eprintln!("❌ Validation Error: {}", error);
        process::exit(1);
    }
}

/// Display available PEQ models with descriptions and exit
pub fn display_peq_model_list() -> ! {
    println!("Available PEQ Models");
    println!("===================");
    println!();
    println!("The PEQ model defines the structure and constraints of the equalizer filters.");
    println!();

    for model in PeqModel::all() {
        println!("  --peq-model {}", model);
        println!("    {}", model.description());
        println!();
    }

    println!("Examples:");
    println!("  autoeq --peq-model pk           # All peak filters (default)");
    println!("  autoeq --peq-model hp-pk        # Highpass + peaks");
    println!("  autoeq --peq-model hp-pk-lp     # Highpass + peaks + lowpass");

    process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults() {
        // Simulate no CLI args: use default values
        let args = Args::parse_from(["autoeq-test"]);
        assert_eq!(args.num_filters, 7);
        assert_eq!(args.sample_rate, 48000.0);
        assert_eq!(args.maxeval, 2000);
        assert_eq!(args.curve_name, "Listening Window");
        assert_eq!(args.peq_model, PeqModel::Pk);
    }

    #[test]
    fn min_db_must_be_strictly_positive_zero_rejected() {
        let res = Args::try_parse_from(["autoeq-test", "--min-db", "0.0"]);
        assert!(res.is_err());
    }

    #[test]
    fn min_db_must_be_strictly_positive_negative_rejected() {
        let res = Args::try_parse_from(["autoeq-test", "--min-db", "-0.1"]);
        assert!(res.is_err());
    }

    #[test]
    fn max_db_allows_zero() {
        let res = Args::try_parse_from(["autoeq-test", "--max-db", "0.0"]);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().max_db, 0.0);
    }

    #[test]
    fn max_db_rejects_negative() {
        let res = Args::try_parse_from(["autoeq-test", "--max-db", "-1.0"]);
        assert!(res.is_err());
    }

    #[test]
    fn validate_args_valid_config() {
        let args = Args::parse_from(["autoeq-test"]);
        assert!(validate_args(&args).is_ok());
    }

    #[test]
    fn validate_args_invalid_algorithm() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.algo = "invalid-algo".to_string();
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown algorithm"));
    }

    #[test]
    fn validate_args_invalid_local_algorithm() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.refine = true;
        args.local_algo = "invalid-local-algo".to_string();
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown local algorithm"));
    }

    #[test]
    fn validate_args_min_q_greater_than_max_q() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.min_q = 5.0;
        args.max_q = 2.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid Q factor range"));
    }

    #[test]
    fn validate_args_min_freq_greater_than_max_freq() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.min_freq = 1000.0;
        args.max_freq = 500.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid frequency range"));
    }

    #[test]
    fn validate_args_min_db_greater_than_max_db() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.min_db = 5.0;
        args.max_db = 2.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid dB range"));
    }

    #[test]
    fn validate_args_min_freq_too_low() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.min_freq = 10.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must be >= 20 Hz"));
    }

    #[test]
    fn validate_args_max_freq_too_high() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.max_freq = 25000.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must be <= 20,000 Hz"));
    }

    #[test]
    fn validate_args_zero_population() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.population = 0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Population size must be > 0"));
    }

    #[test]
    fn validate_args_zero_num_filters() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.num_filters = 0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Number of filters must be > 0")
        );
    }

    #[test]
    fn validate_args_too_many_filters() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.num_filters = 100;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("very high"));
    }

    #[test]
    fn validate_args_invalid_smooth_n() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.smooth_n = 0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must be in range [1..24]"));

        args.smooth_n = 25;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must be in range [1..24]"));
    }

    #[test]
    fn validate_args_invalid_de_strategy() {
        let mut args = Args::parse_from(["autoeq-test", "--algo", "autoeq:de"]);
        args.strategy = "invalid-strategy".to_string();
        let result = validate_args(&args);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Invalid DE strategy"));
        assert!(err.contains("--strategy-list"));
    }

    #[test]
    fn validate_args_valid_de_strategy() {
        let mut args = Args::parse_from(["autoeq-test", "--algo", "autoeq:de"]);
        args.strategy = "best1bin".to_string();
        let result = validate_args(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_args_invalid_tolerance() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.tolerance = 0.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Tolerance must be > 0"));

        args.tolerance = -0.1;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Tolerance must be > 0"));
    }

    #[test]
    fn validate_args_invalid_atolerance() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.atolerance = -0.1;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Absolute tolerance must be >= 0")
        );
    }

    #[test]
    fn validate_args_invalid_adaptive_weights() {
        let mut args = Args::parse_from(["autoeq-test"]);

        // Test adaptive_weight_f out of bounds
        args.adaptive_weight_f = -0.1;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Adaptive weight for F must be between 0.0 and 1.0")
        );

        args.adaptive_weight_f = 1.1;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Adaptive weight for F must be between 0.0 and 1.0")
        );

        // Reset and test adaptive_weight_cr out of bounds
        args.adaptive_weight_f = 0.5;
        args.adaptive_weight_cr = -0.1;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Adaptive weight for CR must be between 0.0 and 1.0")
        );

        args.adaptive_weight_cr = 1.1;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Adaptive weight for CR must be between 0.0 and 1.0")
        );
    }

    #[test]
    fn parse_recombination_probability_valid() {
        assert_eq!(parse_recombination_probability("0.0").unwrap(), 0.0);
        assert_eq!(parse_recombination_probability("0.5").unwrap(), 0.5);
        assert_eq!(parse_recombination_probability("1.0").unwrap(), 1.0);
    }

    #[test]
    fn parse_recombination_probability_invalid() {
        assert!(parse_recombination_probability("-0.1").is_err());
        assert!(parse_recombination_probability("1.1").is_err());
        assert!(parse_recombination_probability("not_a_number").is_err());
    }

    #[test]
    fn cli_defaults_for_de_parameters() {
        let args = Args::parse_from(["autoeq-test"]);
        assert_eq!(args.tolerance, 1e-3);
        assert_eq!(args.atolerance, 1e-4);
        assert_eq!(args.recombination, 0.9);
        assert_eq!(args.strategy, "currenttobest1bin");
        assert_eq!(args.adaptive_weight_f, 0.9);
        assert_eq!(args.adaptive_weight_cr, 0.9);
        assert!(!args.strategy_list);
    }
}

// Custom value parser to enforce strictly positive f64
fn parse_strictly_positive_f64(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|_| format!("invalid float: {s}"))?;
    if v > 0.0 {
        Ok(v)
    } else {
        Err("value must be strictly positive (> 0)".to_string())
    }
}

// Custom value parser to enforce non-negative f64 (>= 0)
fn parse_nonnegative_f64(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|_| format!("invalid float: {s}"))?;
    if v >= 0.0 {
        Ok(v)
    } else {
        Err("value must be non-negative (>= 0)".to_string())
    }
}

// Custom value parser to enforce recombination probability (0.0 to 1.0)
fn parse_recombination_probability(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|_| format!("invalid float: {s}"))?;
    if (0.0..=1.0).contains(&v) {
        Ok(v)
    } else {
        Err("recombination probability must be between 0.0 and 1.0".to_string())
    }
}
