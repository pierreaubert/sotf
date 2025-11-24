#![doc = include_str!("../README.md")]
#![doc = include_str!("../REFERENCES.md")]
#![allow(missing_docs)]
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use ndarray::{Array1, Array2};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Instant;

pub mod stack_linear_penalty;

pub mod apply_integrality;
pub mod apply_wls;

pub mod distinct_indices;
pub mod init_latin_hypercube;
pub mod init_random;

pub mod mutant_adaptive;
pub mod mutant_best1;
pub mod mutant_best2;
pub mod mutant_current_to_best1;
pub mod mutant_rand1;
pub mod mutant_rand2;
pub mod mutant_rand_to_best1;

pub mod crossover_binomial;
pub mod crossover_exponential;

pub mod differential_evolution;
pub mod function_registry;
pub mod impl_helpers;
pub mod metadata;
pub mod parallel_eval;
pub mod recorder;
pub mod run_recorded;
pub use differential_evolution::differential_evolution;
pub use parallel_eval::ParallelConfig;
pub use recorder::{OptimizationRecord, OptimizationRecorder};
pub use run_recorded::run_recorded_differential_evolution;

// Type aliases to reduce complexity
/// Scalar constraint function type
pub type ScalarConstraintFn = Arc<dyn Fn(&Array1<f64>) -> f64 + Send + Sync>;
/// Vector constraint function type
pub type VectorConstraintFn = Arc<dyn Fn(&Array1<f64>) -> Array1<f64> + Send + Sync>;
/// Penalty tuple type (function, weight)
pub type PenaltyTuple = (ScalarConstraintFn, f64);
/// Callback function type
pub type CallbackFn = Box<dyn FnMut(&DEIntermediate) -> CallbackAction>;

pub(crate) fn argmin(v: &Array1<f64>) -> (usize, f64) {
    let mut best_i = 0usize;
    let mut best_v = v[0];
    for (i, &val) in v.iter().enumerate() {
        if val < best_v {
            best_v = val;
            best_i = i;
        }
    }
    (best_i, best_v)
}

/// Differential Evolution strategy
#[derive(Debug, Clone, Copy)]
pub enum Strategy {
    Best1Bin,
    Best1Exp,
    Rand1Bin,
    Rand1Exp,
    Rand2Bin,
    Rand2Exp,
    CurrentToBest1Bin,
    CurrentToBest1Exp,
    Best2Bin,
    Best2Exp,
    RandToBest1Bin,
    RandToBest1Exp,
    /// Adaptive mutation based on the SAM approach: dynamic sampling from top w% individuals
    /// where w decreases linearly from w_max to w_min based on current iteration
    AdaptiveBin,
    /// Adaptive mutation with exponential crossover
    AdaptiveExp,
}

impl FromStr for Strategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.to_lowercase();
        match t.as_str() {
            "best1bin" | "best1" => Ok(Strategy::Best1Bin),
            "best1exp" => Ok(Strategy::Best1Exp),
            "rand1bin" | "rand1" => Ok(Strategy::Rand1Bin),
            "rand1exp" => Ok(Strategy::Rand1Exp),
            "rand2bin" | "rand2" => Ok(Strategy::Rand2Bin),
            "rand2exp" => Ok(Strategy::Rand2Exp),
            "currenttobest1bin" | "current-to-best1bin" | "current_to_best1bin" => {
                Ok(Strategy::CurrentToBest1Bin)
            }
            "currenttobest1exp" | "current-to-best1exp" | "current_to_best1exp" => {
                Ok(Strategy::CurrentToBest1Exp)
            }
            "best2bin" | "best2" => Ok(Strategy::Best2Bin),
            "best2exp" => Ok(Strategy::Best2Exp),
            "randtobest1bin" | "rand-to-best1bin" | "rand_to_best1bin" => {
                Ok(Strategy::RandToBest1Bin)
            }
            "randtobest1exp" | "rand-to-best1exp" | "rand_to_best1exp" => {
                Ok(Strategy::RandToBest1Exp)
            }
            "adaptivebin" | "adaptive-bin" | "adaptive_bin" | "adaptive" => {
                Ok(Strategy::AdaptiveBin)
            }
            "adaptiveexp" | "adaptive-exp" | "adaptive_exp" => Ok(Strategy::AdaptiveExp),
            _ => Err(format!("unknown strategy: {}", s)),
        }
    }
}

/// Crossover type
#[derive(Debug, Clone, Copy, Default)]
pub enum Crossover {
    /// Binomial (uniform) crossover
    #[default]
    Binomial,
    /// Exponential crossover
    Exponential,
}

/// Mutation setting: either a fixed factor, a uniform range (dithering), or adaptive
#[derive(Debug, Clone, Copy)]
pub enum Mutation {
    /// Fixed mutation factor F in [0, 2)
    Factor(f64),
    /// Dithering range [min, max) with 0 <= min < max <= 2
    Range { min: f64, max: f64 },
    /// Adaptive mutation factor using Cauchy distribution with location parameter tracking
    Adaptive { initial_f: f64 },
}

impl Default for Mutation {
    fn default() -> Self {
        let _ = Mutation::Factor(0.8);
        Mutation::Range { min: 0.0, max: 2.0 }
    }
}

impl Mutation {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        match *self {
            Mutation::Factor(f) => f,
            Mutation::Range { min, max } => rng.random_range(min..max),
            Mutation::Adaptive { initial_f } => initial_f, // Will be overridden by adaptive logic
        }
    }

    /// Sample from Cauchy distribution for adaptive mutation (F parameter)
    #[allow(dead_code)]
    fn sample_cauchy<R: Rng + ?Sized>(&self, f_m: f64, _scale: f64, rng: &mut R) -> f64 {
        // Simplified version using normal random for now
        let perturbation = (rng.random::<f64>() - 0.5) * 0.2; // Small perturbation
        (f_m + perturbation).clamp(0.0, 2.0) // Clamp to valid range
    }
}

/// Initialization scheme for the population
#[derive(Debug, Clone, Copy, Default)]
pub enum Init {
    #[default]
    LatinHypercube,
    Random,
}

/// Whether best updates during a generation (we use Deferred only)
#[derive(Debug, Clone, Copy, Default)]
pub enum Updating {
    #[default]
    Deferred,
}

/// Linear penalty specification: lb <= A x <= ub (component-wise)
#[derive(Debug, Clone)]
pub struct LinearPenalty {
    pub a: Array2<f64>,
    pub lb: Array1<f64>,
    pub ub: Array1<f64>,
    pub weight: f64,
}

/// SciPy-like linear constraint helper: lb <= A x <= ub
#[derive(Debug, Clone)]
pub struct LinearConstraintHelper {
    pub a: Array2<f64>,
    pub lb: Array1<f64>,
    pub ub: Array1<f64>,
}

impl LinearConstraintHelper {
    /// Apply helper by merging into DEConfig.linear_penalty (stacking rows if already present)
    pub fn apply_to(&self, cfg: &mut DEConfig, weight: f64) {
        use stack_linear_penalty::stack_linear_penalty;

        let new_lp = LinearPenalty {
            a: self.a.clone(),
            lb: self.lb.clone(),
            ub: self.ub.clone(),
            weight,
        };
        match &mut cfg.linear_penalty {
            Some(existing) => stack_linear_penalty(existing, &new_lp),
            None => cfg.linear_penalty = Some(new_lp),
        }
    }
}

/// SciPy-like nonlinear constraint helper: vector-valued fun(x) with lb <= fun(x) <= ub
#[derive(Clone)]
pub struct NonlinearConstraintHelper {
    pub fun: VectorConstraintFn,
    pub lb: Array1<f64>,
    pub ub: Array1<f64>,
}

impl NonlinearConstraintHelper {
    /// Apply helper by emitting penalty closures per component.
    /// lb <= f_i(x) <= ub becomes two inequalities: f_i(x)-ub <= 0 and lb - f_i(x) <= 0.
    /// If lb==ub, emit an equality penalty for f_i(x)-lb.
    pub fn apply_to(&self, cfg: &mut DEConfig, weight_ineq: f64, weight_eq: f64) {
        let f = self.fun.clone();
        let lb = self.lb.clone();
        let ub = self.ub.clone();
        let m = lb.len().min(ub.len());
        for i in 0..m {
            let l = lb[i];
            let u = ub[i];
            if (u - l).abs() < 1e-18 {
                let fi = f.clone();
                cfg.penalty_eq.push((
                    Arc::new(move |x: &Array1<f64>| {
                        let y = (fi)(x);
                        y[i] - l
                    }),
                    weight_eq,
                ));
            } else {
                let fi_u = f.clone();
                cfg.penalty_ineq.push((
                    Arc::new(move |x: &Array1<f64>| {
                        let y = (fi_u)(x);
                        y[i] - u
                    }),
                    weight_ineq,
                ));
                let fi_l = f.clone();
                cfg.penalty_ineq.push((
                    Arc::new(move |x: &Array1<f64>| {
                        let y = (fi_l)(x);
                        l - y[i]
                    }),
                    weight_ineq,
                ));
            }
        }
    }
}

/// Structures for tracking adaptive parameters
#[derive(Debug, Clone)]
struct AdaptiveState {
    /// Current F_m parameter for Cauchy distribution (mutation)
    f_m: f64,
    /// Current CR_m parameter for Gaussian distribution (crossover)
    cr_m: f64,
    /// Successful F values from this generation
    successful_f: Vec<f64>,
    /// Successful CR values from this generation
    successful_cr: Vec<f64>,
    /// Current linearly decreasing weight for adaptive mutation
    current_w: f64,
}

impl AdaptiveState {
    fn new(config: &AdaptiveConfig) -> Self {
        Self {
            f_m: config.f_m,
            cr_m: config.cr_m,
            successful_f: Vec::new(),
            successful_cr: Vec::new(),
            current_w: config.w_max, // Start with maximum weight
        }
    }

    /// Update adaptive parameters based on successful trials
    fn update(&mut self, config: &AdaptiveConfig, iter: usize, max_iter: usize) {
        // Update linearly decreasing weight (Equation 19 from the paper)
        let iter_ratio = iter as f64 / max_iter as f64;
        self.current_w = config.w_max - (config.w_max - config.w_min) * iter_ratio;

        // Update F_m using Lehmer mean of successful F values
        if !self.successful_f.is_empty() {
            let mean_f = self.compute_lehmer_mean(&self.successful_f);
            self.f_m = (1.0 - config.w_f) * self.f_m + config.w_f * mean_f;
            self.f_m = self.f_m.clamp(0.2, 1.2);
        }

        // Update CR_m using arithmetic mean of successful CR values
        if !self.successful_cr.is_empty() {
            let mean_cr = self.compute_arithmetic_mean(&self.successful_cr);
            self.cr_m = (1.0 - config.w_cr) * self.cr_m + config.w_cr * mean_cr;
            self.cr_m = self.cr_m.clamp(0.1, 0.9);
        }

        // Clear successful values for next generation
        self.successful_f.clear();
        self.successful_cr.clear();
    }

    /// Compute Lehmer mean for successful F values (p=2)
    fn compute_lehmer_mean(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.5; // Default fallback
        }

        let sum_sq: f64 = values.iter().map(|&x| x * x).sum();
        let sum: f64 = values.iter().sum();

        if sum > 0.0 {
            sum_sq / sum
        } else {
            0.5 // Fallback if sum is zero
        }
    }

    /// Compute arithmetic mean for successful CR values
    fn compute_arithmetic_mean(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.5; // Default fallback
        }
        values.iter().sum::<f64>() / values.len() as f64
    }

    /// Record successful parameter values
    fn record_success(&mut self, f_val: f64, cr_val: f64) {
        self.successful_f.push(f_val);
        self.successful_cr.push(cr_val);
    }

    /// Sample adaptive F parameter using conservative normal distribution
    fn sample_f<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        // Use simple normal distribution with smaller variance for stability
        let u1: f64 = rng.random();
        let u2: f64 = rng.random();

        // Box-Muller transform with smaller standard deviation
        let normal = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        let sample = self.f_m + 0.05 * normal; // Reduced variance from 0.1 to 0.05

        // Clamp to conservative bounds
        sample.clamp(0.3, 1.0)
    }

    /// Sample adaptive CR parameter using conservative Gaussian distribution
    fn sample_cr<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        // Use normal distribution with smaller variance for stability
        let u1: f64 = rng.random();
        let u2: f64 = rng.random();

        // Box-Muller transform with smaller standard deviation
        let normal = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        let sample = self.cr_m + 0.05 * normal; // Reduced variance from 0.1 to 0.05

        sample.clamp(0.1, 0.9)
    }
}

/// Adaptive differential evolution configuration
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    /// Enable adaptive mutation strategy
    pub adaptive_mutation: bool,
    /// Enable Wrapper Local Search (WLS)
    pub wls_enabled: bool,
    /// Maximum weight for adaptive mutation (w_max)
    pub w_max: f64,
    /// Minimum weight for adaptive mutation (w_min)
    pub w_min: f64,
    /// Weight factor for F parameter adaptation (between 0.8 and 1.0)
    pub w_f: f64,
    /// Weight factor for CR parameter adaptation (between 0.9 and 1.0)
    pub w_cr: f64,
    /// Initial location parameter for Cauchy distribution (F_m)
    pub f_m: f64,
    /// Initial location parameter for Gaussian distribution (CR_m)
    pub cr_m: f64,
    /// WLS probability (what fraction of population to apply WLS to)
    pub wls_prob: f64,
    /// WLS Cauchy scale parameter
    pub wls_scale: f64,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            adaptive_mutation: false,
            wls_enabled: false,
            w_max: 0.9,
            w_min: 0.1,
            w_f: 0.9,
            w_cr: 0.9,
            f_m: 0.5,
            cr_m: 0.6,
            wls_prob: 0.1,
            wls_scale: 0.1,
        }
    }
}

/// Polishing configuration using NLopt local optimizer within bounds
#[derive(Debug, Clone)]
pub struct PolishConfig {
    pub enabled: bool,
    pub algo: String,   // e.g., "neldermead", "sbplx", "cobyla"
    pub maxeval: usize, // e.g., 200*n
}

/// Configuration for the Differential Evolution optimizer
pub struct DEConfig {
    pub maxiter: usize,
    pub popsize: usize, // total NP = popsize * n_params_free
    pub tol: f64,
    pub atol: f64,
    pub mutation: Mutation,
    pub recombination: f64, // CR in [0,1]
    pub strategy: Strategy,
    pub crossover: Crossover,
    pub init: Init,
    pub updating: Updating,
    pub seed: Option<u64>,
    /// Optional integrality mask; true => variable is integer-constrained
    pub integrality: Option<Vec<bool>>,
    /// Optional initial guess used to replace the best member after init
    pub x0: Option<Array1<f64>>,
    /// Print objective best at each iteration
    pub disp: bool,
    /// Optional per-iteration callback (may stop early)
    pub callback: Option<CallbackFn>,
    /// Penalty-based inequality constraints: fc(x) <= 0
    pub penalty_ineq: Vec<PenaltyTuple>,
    /// Penalty-based equality constraints: h(x) = 0
    pub penalty_eq: Vec<PenaltyTuple>,
    /// Optional linear constraints treated by penalty: lb <= A x <= ub (component-wise)
    pub linear_penalty: Option<LinearPenalty>,
    /// Polishing configuration (optional)
    pub polish: Option<PolishConfig>,
    /// Adaptive differential evolution configuration
    pub adaptive: AdaptiveConfig,
    /// Parallel evaluation configuration
    pub parallel: parallel_eval::ParallelConfig,
}

impl Default for DEConfig {
    fn default() -> Self {
        Self {
            maxiter: 1000,
            popsize: 15,
            tol: 1e-2,
            atol: 0.0,
            mutation: Mutation::default(),
            recombination: 0.7,
            strategy: Strategy::Best1Bin,
            crossover: Crossover::default(),
            init: Init::default(),
            updating: Updating::default(),
            seed: None,
            integrality: None,
            x0: None,
            disp: false,
            callback: None,
            penalty_ineq: Vec::new(),
            penalty_eq: Vec::new(),
            linear_penalty: None,
            polish: None,
            adaptive: AdaptiveConfig::default(),
            parallel: parallel_eval::ParallelConfig::default(),
        }
    }
}

/// Fluent builder for `DEConfig` for ergonomic configuration.
pub struct DEConfigBuilder {
    cfg: DEConfig,
}
impl Default for DEConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DEConfigBuilder {
    pub fn new() -> Self {
        Self {
            cfg: DEConfig::default(),
        }
    }
    pub fn maxiter(mut self, v: usize) -> Self {
        self.cfg.maxiter = v;
        self
    }
    pub fn popsize(mut self, v: usize) -> Self {
        self.cfg.popsize = v;
        self
    }
    pub fn tol(mut self, v: f64) -> Self {
        self.cfg.tol = v;
        self
    }
    pub fn atol(mut self, v: f64) -> Self {
        self.cfg.atol = v;
        self
    }
    pub fn mutation(mut self, v: Mutation) -> Self {
        self.cfg.mutation = v;
        self
    }
    pub fn recombination(mut self, v: f64) -> Self {
        self.cfg.recombination = v;
        self
    }
    pub fn strategy(mut self, v: Strategy) -> Self {
        self.cfg.strategy = v;
        self
    }
    pub fn crossover(mut self, v: Crossover) -> Self {
        self.cfg.crossover = v;
        self
    }
    pub fn init(mut self, v: Init) -> Self {
        self.cfg.init = v;
        self
    }
    pub fn seed(mut self, v: u64) -> Self {
        self.cfg.seed = Some(v);
        self
    }
    pub fn integrality(mut self, v: Vec<bool>) -> Self {
        self.cfg.integrality = Some(v);
        self
    }
    pub fn x0(mut self, v: Array1<f64>) -> Self {
        self.cfg.x0 = Some(v);
        self
    }
    pub fn disp(mut self, v: bool) -> Self {
        self.cfg.disp = v;
        self
    }
    pub fn callback(mut self, cb: Box<dyn FnMut(&DEIntermediate) -> CallbackAction>) -> Self {
        self.cfg.callback = Some(cb);
        self
    }
    pub fn add_penalty_ineq<FN>(mut self, f: FN, w: f64) -> Self
    where
        FN: Fn(&Array1<f64>) -> f64 + Send + Sync + 'static,
    {
        self.cfg.penalty_ineq.push((Arc::new(f), w));
        self
    }
    pub fn add_penalty_eq<FN>(mut self, f: FN, w: f64) -> Self
    where
        FN: Fn(&Array1<f64>) -> f64 + Send + Sync + 'static,
    {
        self.cfg.penalty_eq.push((Arc::new(f), w));
        self
    }
    pub fn linear_penalty(mut self, lp: LinearPenalty) -> Self {
        self.cfg.linear_penalty = Some(lp);
        self
    }
    pub fn polish(mut self, pol: PolishConfig) -> Self {
        self.cfg.polish = Some(pol);
        self
    }
    pub fn adaptive(mut self, adaptive: AdaptiveConfig) -> Self {
        self.cfg.adaptive = adaptive;
        self
    }
    pub fn enable_adaptive_mutation(mut self, enable: bool) -> Self {
        self.cfg.adaptive.adaptive_mutation = enable;
        self
    }
    pub fn enable_wls(mut self, enable: bool) -> Self {
        self.cfg.adaptive.wls_enabled = enable;
        self
    }
    pub fn adaptive_weights(mut self, w_max: f64, w_min: f64) -> Self {
        self.cfg.adaptive.w_max = w_max;
        self.cfg.adaptive.w_min = w_min;
        self
    }
    pub fn parallel(mut self, parallel: parallel_eval::ParallelConfig) -> Self {
        self.cfg.parallel = parallel;
        self
    }
    pub fn enable_parallel(mut self, enable: bool) -> Self {
        self.cfg.parallel.enabled = enable;
        self
    }
    pub fn parallel_threads(mut self, num_threads: usize) -> Self {
        self.cfg.parallel.num_threads = Some(num_threads);
        self
    }
    pub fn build(self) -> DEConfig {
        self.cfg
    }
}

/// Result/Report of a DE optimization run
#[derive(Clone)]
pub struct DEReport {
    pub x: Array1<f64>,
    pub fun: f64,
    pub success: bool,
    pub message: String,
    pub nit: usize,
    pub nfev: usize,
    pub population: Array2<f64>,
    pub population_energies: Array1<f64>,
}

impl fmt::Debug for DEReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DEReport")
            .field("x", &format!("len={}", self.x.len()))
            .field("fun", &self.fun)
            .field("success", &self.success)
            .field("message", &self.message)
            .field("nit", &self.nit)
            .field("nfev", &self.nfev)
            .field(
                "population",
                &format!("{}x{}", self.population.nrows(), self.population.ncols()),
            )
            .field(
                "population_energies",
                &format!("len={}", self.population_energies.len()),
            )
            .finish()
    }
}

/// Information passed to callback after each generation
pub struct DEIntermediate {
    pub x: Array1<f64>,
    pub fun: f64,
    pub convergence: f64, // measured as std(pop_f)
    pub iter: usize,
}

/// Action returned by callback
pub enum CallbackAction {
    Continue,
    Stop,
}

/// Differential Evolution optimizer
pub struct DifferentialEvolution<'a, F>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    func: &'a F,
    lower: Array1<f64>,
    upper: Array1<f64>,
    config: DEConfig,
}

impl<'a, F> DifferentialEvolution<'a, F>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    /// Create a new DE optimizer with objective `func` and bounds [lower, upper]
    pub fn new(func: &'a F, lower: Array1<f64>, upper: Array1<f64>) -> Self {
        assert_eq!(lower.len(), upper.len(), "lower/upper size mismatch");
        Self {
            func,
            lower,
            upper,
            config: DEConfig::default(),
        }
    }

    /// Mutable access to configuration
    pub fn config_mut(&mut self) -> &mut DEConfig {
        &mut self.config
    }

    /// Run the optimization and return a report
    pub fn solve(&mut self) -> DEReport {
        use apply_integrality::apply_integrality;
        use apply_wls::apply_wls;
        use crossover_binomial::binomial_crossover;
        use crossover_exponential::exponential_crossover;
        use init_latin_hypercube::init_latin_hypercube;
        use init_random::init_random;
        use mutant_adaptive::mutant_adaptive;
        use mutant_best1::mutant_best1;
        use mutant_best2::mutant_best2;
        use mutant_current_to_best1::mutant_current_to_best1;
        use mutant_rand_to_best1::mutant_rand_to_best1;
        use mutant_rand1::mutant_rand1;
        use mutant_rand2::mutant_rand2;
        use parallel_eval::evaluate_trials_parallel;
        use std::sync::Arc;

        let n = self.lower.len();

        // Identify fixed (equal-bounds) and free variables
        let mut is_free: Vec<bool> = Vec::with_capacity(n);
        for i in 0..n {
            is_free.push((self.upper[i] - self.lower[i]).abs() > 0.0);
        }
        let n_free = is_free.iter().filter(|&&b| b).count();
        let _n_equal = n - n_free;
        if n_free == 0 {
            // All fixed; just evaluate x = lower
            let x_fixed = self.lower.clone();
            let mut x_eval = x_fixed.clone();
            if let Some(mask) = &self.config.integrality {
                apply_integrality(&mut x_eval, mask, &self.lower, &self.upper);
            }
            let f = (self.func)(&x_eval);
            return DEReport {
                x: x_eval,
                fun: f,
                success: true,
                message: "All variables fixed by bounds".into(),
                nit: 0,
                nfev: 1,
                population: Array2::zeros((1, n)),
                population_energies: Array1::from(vec![f]),
            };
        }

        let npop = self.config.popsize * n_free;
        let _bounds_span = &self.upper - &self.lower;

        if self.config.disp {
            eprintln!(
                "DE Init: {} dimensions ({} free), population={}, maxiter={}",
                n, n_free, npop, self.config.maxiter
            );
            eprintln!(
                "  Strategy: {:?}, Mutation: {:?}, Crossover: CR={:.3}",
                self.config.strategy, self.config.mutation, self.config.recombination
            );
            eprintln!(
                "  Tolerances: tol={:.2e}, atol={:.2e}",
                self.config.tol, self.config.atol
            );
        }

        // Timing toggle via env var
        let timing_enabled = std::env::var("AUTOEQ_DE_TIMING")
            .map(|v| v != "0")
            .unwrap_or(false);

        // Configure global rayon thread pool once if requested
        if let Some(n) = self.config.parallel.num_threads {
            // Ignore error if global pool already set
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build_global();
        }

        // RNG
        let mut rng: StdRng = match self.config.seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => {
                let mut thread_rng = rand::rng();
                StdRng::from_rng(&mut thread_rng)
            }
        };

        // Initialize population in [lower, upper]
        let mut pop = match self.config.init {
            Init::LatinHypercube => {
                if self.config.disp {
                    eprintln!("  Using Latin Hypercube initialization");
                }
                init_latin_hypercube(n, npop, &self.lower, &self.upper, &is_free, &mut rng)
            }
            Init::Random => {
                if self.config.disp {
                    eprintln!("  Using Random initialization");
                }
                init_random(n, npop, &self.lower, &self.upper, &is_free, &mut rng)
            }
        };

        // Evaluate energies (objective + penalties)
        let mut nfev: usize = 0;
        if self.config.disp {
            eprintln!("  Evaluating initial population of {} individuals...", npop);
        }

        // Prepare population for evaluation (apply integrality constraints)
        let mut eval_pop = pop.clone();
        let t_integrality0 = Instant::now();
        if let Some(mask) = &self.config.integrality {
            for i in 0..npop {
                let mut row = eval_pop.row_mut(i);
                let mut x_eval = row.to_owned();
                apply_integrality(&mut x_eval, mask, &self.lower, &self.upper);
                row.assign(&x_eval);
            }
        }
        let t_integrality = t_integrality0.elapsed();

        // Build thread-safe energy function that includes penalties
        let func_ref = self.func;
        let penalty_ineq_vec: Vec<PenaltyTuple> = self
            .config
            .penalty_ineq
            .iter()
            .map(|(f, w)| (f.clone(), *w))
            .collect();
        let penalty_eq_vec: Vec<PenaltyTuple> = self
            .config
            .penalty_eq
            .iter()
            .map(|(f, w)| (f.clone(), *w))
            .collect();
        let linear_penalty = self.config.linear_penalty.clone();

        let energy_fn = Arc::new(move |x: &Array1<f64>| -> f64 {
            let base = (func_ref)(x);
            let mut p = 0.0;
            for (f, w) in &penalty_ineq_vec {
                let v = f(x);
                let viol = v.max(0.0);
                p += w * viol * viol;
            }
            for (h, w) in &penalty_eq_vec {
                let v = h(x);
                p += w * v * v;
            }
            if let Some(ref lp) = linear_penalty {
                let ax = lp.a.dot(&x.view());
                for i in 0..ax.len() {
                    let v = ax[i];
                    let lo = lp.lb[i];
                    let hi = lp.ub[i];
                    if v < lo {
                        let d = lo - v;
                        p += lp.weight * d * d;
                    }
                    if v > hi {
                        let d = v - hi;
                        p += lp.weight * d * d;
                    }
                }
            }
            base + p
        });

        let t_eval0 = Instant::now();
        let mut energies = parallel_eval::evaluate_population_parallel(
            &eval_pop,
            energy_fn,
            &self.config.parallel,
        );
        let t_eval_init = t_eval0.elapsed();
        nfev += npop;
        if timing_enabled {
            eprintln!(
                "TIMING init: integrality={:.3} ms, eval={:.3} ms",
                t_integrality.as_secs_f64() * 1e3,
                t_eval_init.as_secs_f64() * 1e3
            );
        }

        // Report initial population statistics
        let pop_mean = energies.mean().unwrap_or(0.0);
        let pop_std = energies.std(0.0);
        if self.config.disp {
            eprintln!(
                "  Initial population: mean={:.6e}, std={:.6e}",
                pop_mean, pop_std
            );
        }

        // If x0 provided, override the best member
        if let Some(x0) = &self.config.x0 {
            let mut x0c = x0.clone();
            // Clip to bounds using ndarray
            for i in 0..x0c.len() {
                x0c[i] = x0c[i].clamp(self.lower[i], self.upper[i]);
            }
            if let Some(mask) = &self.config.integrality {
                apply_integrality(&mut x0c, mask, &self.lower, &self.upper);
            }
            let f0 = self.energy(&x0c);
            nfev += 1;
            // find current best
            let (best_idx, _best_f) = argmin(&energies);
            pop.row_mut(best_idx).assign(&x0c.view());
            energies[best_idx] = f0;
        }

        let (mut best_idx, mut best_f) = argmin(&energies);
        let mut best_x = pop.row(best_idx).to_owned();

        if self.config.disp {
            eprintln!(
                "  Initial best: fitness={:.6e} at index {}",
                best_f, best_idx
            );
            let param_summary: Vec<String> = (0..best_x.len() / 3)
                .map(|i| {
                    let freq = 10f64.powf(best_x[i * 3]);
                    let q = best_x[i * 3 + 1];
                    let gain = best_x[i * 3 + 2];
                    format!("f{:.0}Hz/Q{:.2}/G{:.2}dB", freq, q, gain)
                })
                .collect();
            eprintln!("  Initial best params: [{}]", param_summary.join(", "));
        }

        if self.config.disp {
            eprintln!("DE iter {:4}  best_f={:.6e}", 0, best_f);
        }

        // Initialize adaptive state if adaptive strategies are enabled
        let mut adaptive_state = if matches!(
            self.config.strategy,
            Strategy::AdaptiveBin | Strategy::AdaptiveExp
        ) || self.config.adaptive.adaptive_mutation
        {
            Some(AdaptiveState::new(&self.config.adaptive))
        } else {
            None
        };

        // Main loop
        let mut success = false;
        let mut message = String::new();
        let mut nit = 0;
        let mut accepted_trials;
        let mut improvement_count;

        let mut t_build_tot = std::time::Duration::ZERO;
        let mut t_eval_tot = std::time::Duration::ZERO;
        let mut t_select_tot = std::time::Duration::ZERO;
        let mut t_iter_tot = std::time::Duration::ZERO;

        for iter in 1..=self.config.maxiter {
            nit = iter;
            accepted_trials = 0;
            improvement_count = 0;

            let iter_start = Instant::now();

            // Pre-sort indices for adaptive strategies to avoid re-sorting in the loop
            let sorted_indices = if matches!(
                self.config.strategy,
                Strategy::AdaptiveBin | Strategy::AdaptiveExp
            ) {
                let mut indices: Vec<usize> = (0..npop).collect();
                indices.sort_by(|&a, &b| {
                    energies[a]
                        .partial_cmp(&energies[b])
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                indices
            } else {
                Vec::new() // Not needed for other strategies
            };

            // Generate all trials first, then evaluate in parallel
            let t_build0 = Instant::now();

            // Parallelize trial generation using rayon
            use rayon::prelude::*;
            let trial_data: Vec<(Array1<f64>, f64, f64)> = (0..npop)
                .into_par_iter()
                .map(|i| {
                    // Create thread-local RNG from base seed + iteration + individual index
                    let mut local_rng: StdRng = if let Some(base_seed) = self.config.seed {
                        StdRng::seed_from_u64(
                            base_seed
                                .wrapping_add((iter as u64) << 32)
                                .wrapping_add(i as u64),
                        )
                    } else {
                        // Use thread_rng for unseeded runs
                        let mut thread_rng = rand::rng();
                        StdRng::from_rng(&mut thread_rng)
                    };

                    // Sample mutation factor and crossover rate (adaptive or fixed)
                    let (f, cr) = if let Some(ref adaptive) = adaptive_state {
                        // Use adaptive parameter sampling
                        let adaptive_f = adaptive.sample_f(&mut local_rng);
                        let adaptive_cr = adaptive.sample_cr(&mut local_rng);
                        (adaptive_f, adaptive_cr)
                    } else {
                        // Use fixed or dithered parameters
                        (
                            self.config.mutation.sample(&mut local_rng),
                            self.config.recombination,
                        )
                    };

                    // Generate mutant and apply crossover based on strategy
                    let (mutant, cross) = match self.config.strategy {
                        Strategy::Best1Bin => (
                            mutant_best1(i, &pop, best_idx, f, &mut local_rng),
                            Crossover::Binomial,
                        ),
                        Strategy::Best1Exp => (
                            mutant_best1(i, &pop, best_idx, f, &mut local_rng),
                            Crossover::Exponential,
                        ),
                        Strategy::Rand1Bin => (
                            mutant_rand1(i, &pop, f, &mut local_rng),
                            Crossover::Binomial,
                        ),
                        Strategy::Rand1Exp => (
                            mutant_rand1(i, &pop, f, &mut local_rng),
                            Crossover::Exponential,
                        ),
                        Strategy::Rand2Bin => (
                            mutant_rand2(i, &pop, f, &mut local_rng),
                            Crossover::Binomial,
                        ),
                        Strategy::Rand2Exp => (
                            mutant_rand2(i, &pop, f, &mut local_rng),
                            Crossover::Exponential,
                        ),
                        Strategy::CurrentToBest1Bin => (
                            mutant_current_to_best1(i, &pop, best_idx, f, &mut local_rng),
                            Crossover::Binomial,
                        ),
                        Strategy::CurrentToBest1Exp => (
                            mutant_current_to_best1(i, &pop, best_idx, f, &mut local_rng),
                            Crossover::Exponential,
                        ),
                        Strategy::Best2Bin => (
                            mutant_best2(i, &pop, best_idx, f, &mut local_rng),
                            Crossover::Binomial,
                        ),
                        Strategy::Best2Exp => (
                            mutant_best2(i, &pop, best_idx, f, &mut local_rng),
                            Crossover::Exponential,
                        ),
                        Strategy::RandToBest1Bin => (
                            mutant_rand_to_best1(i, &pop, best_idx, f, &mut local_rng),
                            Crossover::Binomial,
                        ),
                        Strategy::RandToBest1Exp => (
                            mutant_rand_to_best1(i, &pop, best_idx, f, &mut local_rng),
                            Crossover::Exponential,
                        ),
                        Strategy::AdaptiveBin => {
                            if let Some(ref adaptive) = adaptive_state {
                                (
                                    mutant_adaptive(
                                        i,
                                        &pop,
                                        &sorted_indices,
                                        adaptive.current_w,
                                        f,
                                        &mut local_rng,
                                    ),
                                    Crossover::Binomial,
                                )
                            } else {
                                // Fallback to rand1 if adaptive state not available
                                (
                                    mutant_rand1(i, &pop, f, &mut local_rng),
                                    Crossover::Binomial,
                                )
                            }
                        }
                        Strategy::AdaptiveExp => {
                            if let Some(ref adaptive) = adaptive_state {
                                (
                                    mutant_adaptive(
                                        i,
                                        &pop,
                                        &sorted_indices,
                                        adaptive.current_w,
                                        f,
                                        &mut local_rng,
                                    ),
                                    Crossover::Exponential,
                                )
                            } else {
                                // Fallback to rand1 if adaptive state not available
                                (
                                    mutant_rand1(i, &pop, f, &mut local_rng),
                                    Crossover::Exponential,
                                )
                            }
                        }
                    };

                    // If strategy didn't dictate crossover, fallback to config
                    let crossover = cross;
                    let trial = match crossover {
                        Crossover::Binomial => {
                            binomial_crossover(&pop.row(i).to_owned(), &mutant, cr, &mut local_rng)
                        }
                        Crossover::Exponential => exponential_crossover(
                            &pop.row(i).to_owned(),
                            &mutant,
                            cr,
                            &mut local_rng,
                        ),
                    };

                    // Apply WLS if enabled
                    let wls_trial = if self.config.adaptive.wls_enabled
                        && local_rng.random::<f64>() < self.config.adaptive.wls_prob
                    {
                        apply_wls(
                            &trial,
                            &self.lower,
                            &self.upper,
                            self.config.adaptive.wls_scale,
                            &mut local_rng,
                        )
                    } else {
                        trial.clone()
                    };

                    // Clip to bounds using ndarray
                    let mut trial_clipped = wls_trial;
                    for j in 0..trial_clipped.len() {
                        trial_clipped[j] = trial_clipped[j].clamp(self.lower[j], self.upper[j]);
                    }

                    // Apply integrality if provided
                    if let Some(mask) = &self.config.integrality {
                        apply_integrality(&mut trial_clipped, mask, &self.lower, &self.upper);
                    }

                    // Return trial and parameters
                    (trial_clipped, f, cr)
                })
                .collect();

            // Unpack trials and parameters
            let mut trials = Vec::with_capacity(npop);
            let mut trial_params = Vec::with_capacity(npop);
            for (trial, f, cr) in trial_data {
                trials.push(trial);
                trial_params.push((f, cr));
            }
            // Evaluate all trials including penalties, possibly in parallel
            let func_ref = self.func;
            let penalty_ineq_vec: Vec<PenaltyTuple> = self
                .config
                .penalty_ineq
                .iter()
                .map(|(f, w)| (f.clone(), *w))
                .collect();
            let penalty_eq_vec: Vec<PenaltyTuple> = self
                .config
                .penalty_eq
                .iter()
                .map(|(f, w)| (f.clone(), *w))
                .collect();
            let linear_penalty = self.config.linear_penalty.clone();

            let energy_fn_loop = Arc::new(move |x: &Array1<f64>| -> f64 {
                let base = (func_ref)(x);
                let mut p = 0.0;
                for (f, w) in &penalty_ineq_vec {
                    let v = f(x);
                    let viol = v.max(0.0);
                    p += w * viol * viol;
                }
                for (h, w) in &penalty_eq_vec {
                    let v = h(x);
                    p += w * v * v;
                }
                if let Some(ref lp) = linear_penalty {
                    let ax = lp.a.dot(&x.view());
                    for i in 0..ax.len() {
                        let v = ax[i];
                        let lo = lp.lb[i];
                        let hi = lp.ub[i];
                        if v < lo {
                            let d = lo - v;
                            p += lp.weight * d * d;
                        }
                        if v > hi {
                            let d = v - hi;
                            p += lp.weight * d * d;
                        }
                    }
                }
                base + p
            });

            let t_build = t_build0.elapsed();
            let t_eval0 = Instant::now();
            let trial_energies =
                evaluate_trials_parallel(trials.clone(), energy_fn_loop, &self.config.parallel);
            let t_eval = t_eval0.elapsed();
            nfev += npop;

            let t_select0 = Instant::now();
            // Selection phase: update population based on trial results
            for (i, (trial, trial_energy)) in
                trials.into_iter().zip(trial_energies.iter()).enumerate()
            {
                let (f, cr) = trial_params[i];

                // Selection: replace if better
                if *trial_energy <= energies[i] {
                    pop.row_mut(i).assign(&trial.view());
                    energies[i] = *trial_energy;
                    accepted_trials += 1;

                    // Update adaptive parameters if improvement
                    if let Some(ref mut adaptive) = adaptive_state {
                        adaptive.record_success(f, cr);
                    }

                    // Track if this is an improvement over the current best
                    if *trial_energy < best_f {
                        improvement_count += 1;
                    }
                }
            }
            let t_select = t_select0.elapsed();

            t_build_tot += t_build;
            t_eval_tot += t_eval;
            t_select_tot += t_select;
            let iter_dur = iter_start.elapsed();
            t_iter_tot += iter_dur;

            if timing_enabled && (iter <= 5 || iter % 10 == 0) {
                eprintln!(
                    "TIMING iter {:4}: build={:.3} ms, eval={:.3} ms, select={:.3} ms, total={:.3} ms",
                    iter,
                    t_build.as_secs_f64() * 1e3,
                    t_eval.as_secs_f64() * 1e3,
                    t_select.as_secs_f64() * 1e3,
                    iter_dur.as_secs_f64() * 1e3,
                );
            }

            // Update adaptive parameters after each generation
            if let Some(ref mut adaptive) = adaptive_state {
                adaptive.update(&self.config.adaptive, iter, self.config.maxiter);
            }

            // Update best solution after generation
            let (new_best_idx, new_best_f) = argmin(&energies);
            if new_best_f < best_f {
                best_idx = new_best_idx;
                best_f = new_best_f;
                best_x = pop.row(best_idx).to_owned();
            }

            // Convergence check
            let pop_mean = energies.mean().unwrap_or(0.0);
            let pop_std = energies.std(0.0);
            let convergence_threshold = self.config.atol + self.config.tol * pop_mean.abs();

            if self.config.disp {
                eprintln!(
                    "DE iter {:4}  best_f={:.6e}  std={:.3e}  accepted={}/{}, improved={}",
                    iter, best_f, pop_std, accepted_trials, npop, improvement_count
                );
            }

            // Callback
            if let Some(ref mut cb) = self.config.callback {
                let intermediate = DEIntermediate {
                    x: best_x.clone(),
                    fun: best_f,
                    convergence: pop_std,
                    iter,
                };
                match cb(&intermediate) {
                    CallbackAction::Stop => {
                        success = true;
                        message = "Optimization stopped by callback".to_string();
                        break;
                    }
                    CallbackAction::Continue => {}
                }
            }

            if pop_std <= convergence_threshold {
                success = true;
                message = format!(
                    "Converged: std(pop_f)={:.3e} <= threshold={:.3e}",
                    pop_std, convergence_threshold
                );
                break;
            }
        }

        if !success {
            message = format!("Maximum iterations reached: {}", self.config.maxiter);
        }

        if self.config.disp {
            eprintln!("DE finished: {}", message);
        }

        // Polish if configured
        let (final_x, final_f, polish_nfev) = if let Some(ref polish_cfg) = self.config.polish {
            if polish_cfg.enabled {
                self.polish(&best_x)
            } else {
                (best_x.clone(), best_f, 0)
            }
        } else {
            (best_x.clone(), best_f, 0)
        };

        if timing_enabled {
            eprintln!(
                "TIMING total: build={:.3} s, eval={:.3} s, select={:.3} s, iter_total={:.3} s",
                t_build_tot.as_secs_f64(),
                t_eval_tot.as_secs_f64(),
                t_select_tot.as_secs_f64(),
                t_iter_tot.as_secs_f64()
            );
        }

        self.finish_report(
            pop,
            energies,
            final_x,
            final_f,
            success,
            message,
            nit,
            nfev + polish_nfev,
        )
    }
}

#[cfg(test)]
mod strategy_tests {
    use super::*;

    #[test]
    fn test_parse_strategy_variants() {
        assert!(matches!(
            "best1exp".parse::<Strategy>().unwrap(),
            Strategy::Best1Exp
        ));
        assert!(matches!(
            "rand1bin".parse::<Strategy>().unwrap(),
            Strategy::Rand1Bin
        ));
        assert!(matches!(
            "randtobest1exp".parse::<Strategy>().unwrap(),
            Strategy::RandToBest1Exp
        ));
    }
}
