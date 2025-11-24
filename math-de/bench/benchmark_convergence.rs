use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::*;
use clap::{Arg, Command};
use ndarray::Array1;
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

/// Function registry mapping names to actual function pointers
struct FunctionRegistry {
    functions: HashMap<String, fn(&Array1<f64>) -> f64>,
}

impl FunctionRegistry {
    fn new() -> Self {
        let mut functions = HashMap::new();

        // Unimodal functions
        functions.insert("sphere".to_string(), sphere as fn(&Array1<f64>) -> f64);
        functions.insert(
            "rosenbrock".to_string(),
            rosenbrock as fn(&Array1<f64>) -> f64,
        );
        functions.insert("booth".to_string(), booth as fn(&Array1<f64>) -> f64);
        functions.insert("matyas".to_string(), matyas as fn(&Array1<f64>) -> f64);
        functions.insert("beale".to_string(), beale as fn(&Array1<f64>) -> f64);
        functions.insert(
            "himmelblau".to_string(),
            himmelblau as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "sum_squares".to_string(),
            sum_squares as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "different_powers".to_string(),
            different_powers as fn(&Array1<f64>) -> f64,
        );
        functions.insert("elliptic".to_string(), elliptic as fn(&Array1<f64>) -> f64);
        functions.insert("cigar".to_string(), cigar as fn(&Array1<f64>) -> f64);
        functions.insert("tablet".to_string(), tablet as fn(&Array1<f64>) -> f64);
        functions.insert("discus".to_string(), discus as fn(&Array1<f64>) -> f64);
        functions.insert("ridge".to_string(), ridge as fn(&Array1<f64>) -> f64);
        functions.insert(
            "sharp_ridge".to_string(),
            sharp_ridge as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "perm_0_d_beta".to_string(),
            perm_0_d_beta as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "perm_d_beta".to_string(),
            perm_d_beta as fn(&Array1<f64>) -> f64,
        );

        // Multimodal functions
        functions.insert("ackley".to_string(), ackley as fn(&Array1<f64>) -> f64);
        functions.insert(
            "rastrigin".to_string(),
            rastrigin as fn(&Array1<f64>) -> f64,
        );
        functions.insert("griewank".to_string(), griewank as fn(&Array1<f64>) -> f64);
        functions.insert("schwefel".to_string(), schwefel as fn(&Array1<f64>) -> f64);
        functions.insert("branin".to_string(), branin as fn(&Array1<f64>) -> f64);
        functions.insert(
            "goldstein_price".to_string(),
            goldstein_price as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "six_hump_camel".to_string(),
            six_hump_camel as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "hartman_4d".to_string(),
            hartman_4d as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "xin_she_yang_n1".to_string(),
            xin_she_yang_n1 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "xin_she_yang_n2".to_string(),
            xin_she_yang_n2 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "xin_she_yang_n3".to_string(),
            xin_she_yang_n3 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "xin_she_yang_n4".to_string(),
            xin_she_yang_n4 as fn(&Array1<f64>) -> f64,
        );
        functions.insert("katsuura".to_string(), katsuura as fn(&Array1<f64>) -> f64);
        functions.insert("happycat".to_string(), happycat as fn(&Array1<f64>) -> f64);

        // Alpine functions
        functions.insert(
            "alpine_n1".to_string(),
            alpine_n1 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "alpine_n2".to_string(),
            alpine_n2 as fn(&Array1<f64>) -> f64,
        );

        // Additional functions
        functions.insert(
            "gramacy_lee_2012".to_string(),
            gramacy_lee_2012 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "forrester_2008".to_string(),
            forrester_2008 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "power_sum".to_string(),
            power_sum as fn(&Array1<f64>) -> f64,
        );
        functions.insert("shekel".to_string(), shekel as fn(&Array1<f64>) -> f64);
        functions.insert(
            "gramacy_lee_function".to_string(),
            gramacy_lee_function as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "expanded_griewank_rosenbrock".to_string(),
            expanded_griewank_rosenbrock as fn(&Array1<f64>) -> f64,
        );

        // More classical functions
        functions.insert(
            "bohachevsky1".to_string(),
            bohachevsky1 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "bohachevsky2".to_string(),
            bohachevsky2 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "bohachevsky3".to_string(),
            bohachevsky3 as fn(&Array1<f64>) -> f64,
        );
        functions.insert("brown".to_string(), brown as fn(&Array1<f64>) -> f64);
        functions.insert("bukin_n6".to_string(), bukin_n6 as fn(&Array1<f64>) -> f64);
        functions.insert(
            "chung_reynolds".to_string(),
            chung_reynolds as fn(&Array1<f64>) -> f64,
        );
        functions.insert("colville".to_string(), colville as fn(&Array1<f64>) -> f64);
        functions.insert(
            "cosine_mixture".to_string(),
            cosine_mixture as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "cross_in_tray".to_string(),
            cross_in_tray as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "dixons_price".to_string(),
            dixons_price as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "drop_wave".to_string(),
            drop_wave as fn(&Array1<f64>) -> f64,
        );
        functions.insert("easom".to_string(), easom as fn(&Array1<f64>) -> f64);
        functions.insert(
            "eggholder".to_string(),
            eggholder as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "exponential".to_string(),
            exponential as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "freudenstein_roth".to_string(),
            freudenstein_roth as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "griewank2".to_string(),
            griewank2 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "holder_table".to_string(),
            holder_table as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "langermann".to_string(),
            langermann as fn(&Array1<f64>) -> f64,
        );
        functions.insert("levi13".to_string(), levi13 as fn(&Array1<f64>) -> f64);
        functions.insert(
            "mccormick".to_string(),
            mccormick as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "michalewicz".to_string(),
            michalewicz as fn(&Array1<f64>) -> f64,
        );
        functions.insert("periodic".to_string(), periodic as fn(&Array1<f64>) -> f64);
        functions.insert("pinter".to_string(), pinter as fn(&Array1<f64>) -> f64);
        functions.insert("powell".to_string(), powell as fn(&Array1<f64>) -> f64);
        functions.insert("qing".to_string(), qing as fn(&Array1<f64>) -> f64);
        functions.insert(
            "quadratic".to_string(),
            quadratic as fn(&Array1<f64>) -> f64,
        );
        functions.insert("quartic".to_string(), quartic as fn(&Array1<f64>) -> f64);
        functions.insert(
            "schwefel2".to_string(),
            schwefel2 as fn(&Array1<f64>) -> f64,
        );
        functions.insert("step".to_string(), step as fn(&Array1<f64>) -> f64);
        functions.insert(
            "styblinski_tang2".to_string(),
            styblinski_tang2 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "three_hump_camel".to_string(),
            three_hump_camel as fn(&Array1<f64>) -> f64,
        );
        functions.insert("trid".to_string(), trid as fn(&Array1<f64>) -> f64);
        functions.insert("vincent".to_string(), vincent as fn(&Array1<f64>) -> f64);
        functions.insert("whitley".to_string(), whitley as fn(&Array1<f64>) -> f64);
        functions.insert(
            "zakharov2".to_string(),
            zakharov2 as fn(&Array1<f64>) -> f64,
        );

        // Additional functions from test files
        functions.insert(
            "ackley_n2".to_string(),
            ackley_n2 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "ackley_n3".to_string(),
            ackley_n3 as fn(&Array1<f64>) -> f64,
        );
        functions.insert("bird".to_string(), bird as fn(&Array1<f64>) -> f64);
        functions.insert(
            "bent_cigar".to_string(),
            bent_cigar as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "cross_in_tray".to_string(),
            cross_in_tray as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "de_jong_step2".to_string(),
            de_jong_step2 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "dejong_f5_foxholes".to_string(),
            dejong_f5_foxholes as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "drop_wave".to_string(),
            drop_wave as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "eggholder".to_string(),
            eggholder as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "epistatic_michalewicz".to_string(),
            epistatic_michalewicz as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "freudenstein_roth".to_string(),
            freudenstein_roth as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "happy_cat".to_string(),
            happy_cat as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "hartman_3d".to_string(),
            hartman_3d as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "holder_table".to_string(),
            holder_table as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "lampinen_simplified".to_string(),
            lampinen_simplified as fn(&Array1<f64>) -> f64,
        );
        functions.insert("levy".to_string(), levy as fn(&Array1<f64>) -> f64);
        functions.insert("levy_n13".to_string(), levy_n13 as fn(&Array1<f64>) -> f64);
        functions.insert(
            "rotated_hyper_ellipsoid".to_string(),
            rotated_hyper_ellipsoid as fn(&Array1<f64>) -> f64,
        );
        functions.insert("salomon".to_string(), salomon as fn(&Array1<f64>) -> f64);
        functions.insert(
            "schaffer_n2".to_string(),
            schaffer_n2 as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "schaffer_n4".to_string(),
            schaffer_n4 as fn(&Array1<f64>) -> f64,
        );
        functions.insert("shubert".to_string(), shubert as fn(&Array1<f64>) -> f64);
        functions.insert(
            "sum_of_different_powers".to_string(),
            sum_of_different_powers as fn(&Array1<f64>) -> f64,
        );
        functions.insert(
            "three_hump_camel".to_string(),
            three_hump_camel as fn(&Array1<f64>) -> f64,
        );
        functions.insert("zakharov".to_string(), zakharov as fn(&Array1<f64>) -> f64);

        Self { functions }
    }

    fn get(&self, name: &str) -> Option<fn(&Array1<f64>) -> f64> {
        self.functions.get(name).copied()
    }

    fn _list_functions(&self) -> Vec<String> {
        let mut names: Vec<_> = self.functions.keys().cloned().collect();
        names.sort();
        names
    }
}

static FUNCTION_REGISTRY: std::sync::LazyLock<FunctionRegistry> =
    std::sync::LazyLock::new(FunctionRegistry::new);

/// Configuration for a benchmark
#[derive(Clone, Debug)]
struct BenchmarkConfig {
    name: String,
    function_name: String,
    bounds: Vec<(f64, f64)>,
    expected_optimum: Vec<f64>,
    fun_tolerance: f64,
    position_tolerance: f64,
    maxiter: usize,
    popsize: usize,
    strategy: Strategy,
    recombination: f64,
    seed: u64,
}

/// Generate benchmark configurations for all test functions
fn generate_all_benchmarks() -> HashMap<String, Box<dyn Fn() -> BenchmarkResult>> {
    let mut benchmarks: HashMap<String, Box<dyn Fn() -> BenchmarkResult>> = HashMap::new();
    let _metadata = get_function_metadata();

    // Custom configurations for specific functions
    let mut configs = Vec::new();

    // ACKLEY function benchmarks
    configs.push(BenchmarkConfig {
        name: "ackley_2d".to_string(),
        function_name: "ackley".to_string(),
        bounds: vec![(-32.768, 32.768), (-32.768, 32.768)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-3,
        position_tolerance: 0.5,
        maxiter: 800,
        popsize: 40,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 42,
    });

    configs.push(BenchmarkConfig {
        name: "ackley_10d".to_string(),
        function_name: "ackley".to_string(),
        bounds: vec![(-32.768, 32.768); 10],
        expected_optimum: vec![0.0; 10],
        fun_tolerance: 1e-2,
        position_tolerance: 0.5,
        maxiter: 1200,
        popsize: 100,
        strategy: Strategy::Rand1Exp,
        recombination: 0.95,
        seed: 43,
    });

    // ALPINE N1 function benchmarks
    configs.push(BenchmarkConfig {
        name: "alpine_n1_2d".to_string(),
        function_name: "alpine_n1".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-2,
        position_tolerance: 0.2,
        maxiter: 800,
        popsize: 40,
        strategy: Strategy::Best1Bin,
        recombination: 0.9,
        seed: 42,
    });

    configs.push(BenchmarkConfig {
        name: "alpine_n1_5d".to_string(),
        function_name: "alpine_n1".to_string(),
        bounds: vec![(-10.0, 10.0); 5],
        expected_optimum: vec![0.0; 5],
        fun_tolerance: 1e-2,
        position_tolerance: 0.15,
        maxiter: 1200,
        popsize: 80,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.9,
        seed: 43,
    });

    // ALPINE N2 function benchmarks
    configs.push(BenchmarkConfig {
        name: "alpine_n2_2d".to_string(),
        function_name: "alpine_n2".to_string(),
        bounds: vec![(0.0, 10.0), (0.0, 10.0)],
        expected_optimum: vec![2.808, 2.808],
        fun_tolerance: -7.0,     // Maximization: negative values are better
        position_tolerance: 6.0, // Relaxed for multimodal function
        maxiter: 25000,
        popsize: 600,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.95,
        seed: 50,
    });

    configs.push(BenchmarkConfig {
        name: "alpine_n2_3d".to_string(),
        function_name: "alpine_n2".to_string(),
        bounds: vec![(0.0, 10.0); 3],
        expected_optimum: vec![2.808; 3],
        fun_tolerance: -20.0, // 3D: expected minimum ≈ -2.808^3 ≈ -22.2
        position_tolerance: 0.5,
        maxiter: 4000,
        popsize: 200,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 51,
    });

    // ROSENBROCK function benchmarks
    configs.push(BenchmarkConfig {
        name: "rosenbrock_2d".to_string(),
        function_name: "rosenbrock".to_string(),
        bounds: vec![(-2.048, 2.048), (-2.048, 2.048)],
        expected_optimum: vec![1.0, 1.0],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 800,
        popsize: 40,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 48,
    });

    configs.push(BenchmarkConfig {
        name: "rosenbrock_10d".to_string(),
        function_name: "rosenbrock".to_string(),
        bounds: vec![(-2.048, 2.048); 10],
        expected_optimum: vec![1.0; 10],
        fun_tolerance: 1e-1,
        position_tolerance: 0.1,
        maxiter: 2000,
        popsize: 150,
        strategy: Strategy::RandToBest1Exp,
        recombination: 0.95,
        seed: 49,
    });

    // SPHERE function benchmark
    configs.push(BenchmarkConfig {
        name: "sphere_5d".to_string(),
        function_name: "sphere".to_string(),
        bounds: vec![(-5.12, 5.12); 5],
        expected_optimum: vec![0.0; 5],
        fun_tolerance: 1e-8,
        position_tolerance: 1e-4,
        maxiter: 500,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 44,
    });

    // RASTRIGIN function benchmark
    configs.push(BenchmarkConfig {
        name: "rastrigin_5d".to_string(),
        function_name: "rastrigin".to_string(),
        bounds: vec![(-5.12, 5.12); 5],
        expected_optimum: vec![0.0; 5],
        fun_tolerance: 1e-2,
        position_tolerance: 1e-1,
        maxiter: 1500,
        popsize: 100,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 45,
    });

    // SPHERE function benchmarks (multiple dimensions)
    configs.push(BenchmarkConfig {
        name: "sphere_2d".to_string(),
        function_name: "sphere".to_string(),
        bounds: vec![(-5.0, 5.0); 2],
        expected_optimum: vec![0.0; 2],
        fun_tolerance: 1e-12,
        position_tolerance: 1e-6,
        maxiter: 300,
        popsize: 20,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 100,
    });

    configs.push(BenchmarkConfig {
        name: "sphere_10d".to_string(),
        function_name: "sphere".to_string(),
        bounds: vec![(-5.0, 5.0); 10],
        expected_optimum: vec![0.0; 10],
        fun_tolerance: 1e-8,
        position_tolerance: 1e-4,
        maxiter: 800,
        popsize: 50,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 101,
    });

    // ROSENBROCK benchmarks (more dimensions)
    configs.push(BenchmarkConfig {
        name: "rosenbrock_4d".to_string(),
        function_name: "rosenbrock".to_string(),
        bounds: vec![(-2.048, 2.048); 4],
        expected_optimum: vec![1.0; 4],
        fun_tolerance: 1e-2,
        position_tolerance: 0.05,
        maxiter: 1200,
        popsize: 80,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 102,
    });

    // RASTRIGIN benchmarks (2D)
    configs.push(BenchmarkConfig {
        name: "rastrigin_2d".to_string(),
        function_name: "rastrigin".to_string(),
        bounds: vec![(-5.12, 5.12); 2],
        expected_optimum: vec![0.0; 2],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 1000,
        popsize: 50,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 103,
    });

    // GRIEWANK benchmarks
    configs.push(BenchmarkConfig {
        name: "griewank_2d".to_string(),
        function_name: "griewank".to_string(),
        bounds: vec![(-600.0, 600.0); 2],
        expected_optimum: vec![0.0; 2],
        fun_tolerance: 1e-2,
        position_tolerance: 1.0,
        maxiter: 1500,
        popsize: 80,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 104,
    });

    configs.push(BenchmarkConfig {
        name: "griewank_10d".to_string(),
        function_name: "griewank".to_string(),
        bounds: vec![(-600.0, 600.0); 10],
        expected_optimum: vec![0.0; 10],
        fun_tolerance: 0.5, // Relaxed for challenging 10D multimodal function
        position_tolerance: 15.0, // Relaxed for large search space
        maxiter: 2000,
        popsize: 150,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 105,
    });

    // SCHWEFEL benchmarks
    configs.push(BenchmarkConfig {
        name: "schwefel_2d".to_string(),
        function_name: "schwefel".to_string(),
        bounds: vec![(-500.0, 500.0); 2],
        expected_optimum: vec![420.9687, 420.9687],
        fun_tolerance: 1e-2,
        position_tolerance: 1.0,
        maxiter: 2000,
        popsize: 100,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 106,
    });

    configs.push(BenchmarkConfig {
        name: "schwefel_5d".to_string(),
        function_name: "schwefel".to_string(),
        bounds: vec![(-500.0, 500.0); 5],
        expected_optimum: vec![420.9687; 5],
        fun_tolerance: 1e-1,
        position_tolerance: 5.0,
        maxiter: 3000,
        popsize: 200,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 107,
    });

    // 2D CLASSICAL FUNCTIONS

    // BEALE
    configs.push(BenchmarkConfig {
        name: "beale_2d".to_string(),
        function_name: "beale".to_string(),
        bounds: vec![(-4.5, 4.5); 2],
        expected_optimum: vec![3.0, 0.5],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 800,
        popsize: 40,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 108,
    });

    // BOOTH
    configs.push(BenchmarkConfig {
        name: "booth_2d".to_string(),
        function_name: "booth".to_string(),
        bounds: vec![(-10.0, 10.0); 2],
        expected_optimum: vec![1.0, 3.0],
        fun_tolerance: 1e-12,
        position_tolerance: 1e-6,
        maxiter: 600,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 109,
    });

    // MATYAS
    configs.push(BenchmarkConfig {
        name: "matyas_2d".to_string(),
        function_name: "matyas".to_string(),
        bounds: vec![(-10.0, 10.0); 2],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-12,
        position_tolerance: 1e-6,
        maxiter: 400,
        popsize: 25,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 110,
    });

    // HIMMELBLAU (one of the global minima)
    configs.push(BenchmarkConfig {
        name: "himmelblau_2d".to_string(),
        function_name: "himmelblau".to_string(),
        bounds: vec![(-5.0, 5.0); 2],
        expected_optimum: vec![3.0, 2.0],
        fun_tolerance: 1e-6,
        position_tolerance: 0.1, // Relaxed since there are 4 global minima
        maxiter: 1000,
        popsize: 60,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 111,
    });

    // GOLDSTEIN-PRICE
    configs.push(BenchmarkConfig {
        name: "goldstein_price_2d".to_string(),
        function_name: "goldstein_price".to_string(),
        bounds: vec![(-2.0, 2.0); 2],
        expected_optimum: vec![0.0, -1.0],
        fun_tolerance: 3.1, // Global minimum is 3.0, allow some tolerance
        position_tolerance: 0.05,
        maxiter: 1200,
        popsize: 80,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 112,
    });

    // SIX-HUMP CAMEL (one of the global minima)
    configs.push(BenchmarkConfig {
        name: "six_hump_camel_2d".to_string(),
        function_name: "six_hump_camel".to_string(),
        bounds: vec![(-3.0, 3.0), (-2.0, 2.0)],
        expected_optimum: vec![0.0898, -0.7126],
        fun_tolerance: -1.0, // Global minimum is -1.0316, allow some tolerance
        position_tolerance: 1.5, // Relaxed - this function has two symmetric global minima
        maxiter: 1000,
        popsize: 60,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 113,
    });

    // BRANIN (one of the global minima)
    configs.push(BenchmarkConfig {
        name: "branin_2d".to_string(),
        function_name: "branin".to_string(),
        bounds: vec![(-5.0, 10.0), (0.0, 15.0)],
        expected_optimum: vec![std::f64::consts::PI, 2.275],
        fun_tolerance: 0.4, // Global minimum is 0.397887
        position_tolerance: 0.2,
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 114,
    });

    // HARTMAN 4D
    configs.push(BenchmarkConfig {
        name: "hartman_4d".to_string(),
        function_name: "hartman_4d".to_string(),
        bounds: vec![(0.0, 1.0); 4],
        expected_optimum: vec![0.1873, 0.1936, 0.5576, 0.2647],
        fun_tolerance: -3.7, // Global minimum is -3.72983
        position_tolerance: 0.05,
        maxiter: 2000,
        popsize: 120,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 115,
    });

    // SHEKEL 4D
    configs.push(BenchmarkConfig {
        name: "shekel_4d".to_string(),
        function_name: "shekel".to_string(),
        bounds: vec![(0.0, 10.0); 4],
        expected_optimum: vec![4.0, 4.0, 4.0, 4.0],
        fun_tolerance: -10.0, // Global minimum is -10.5364
        position_tolerance: 0.5,
        maxiter: 4000,
        popsize: 200,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 116,
    });

    // 1D FUNCTIONS

    // GRAMACY & LEE 2012
    configs.push(BenchmarkConfig {
        name: "gramacy_lee_2012_1d".to_string(),
        function_name: "gramacy_lee_2012".to_string(),
        bounds: vec![(0.5, 2.5)],
        expected_optimum: vec![0.548563444114526],
        fun_tolerance: -0.86, // Global minimum is -0.869011134989500
        position_tolerance: 0.01,
        maxiter: 500,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 117,
    });

    // FORRESTER 2008
    configs.push(BenchmarkConfig {
        name: "forrester_2008_1d".to_string(),
        function_name: "forrester_2008".to_string(),
        bounds: vec![(0.0, 1.0)],
        expected_optimum: vec![0.757249],
        fun_tolerance: -6.0, // Global minimum is -6.02074
        position_tolerance: 0.01,
        maxiter: 500,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 118,
    });

    // MORE CHALLENGING FUNCTIONS

    // XIN-SHE YANG N1
    configs.push(BenchmarkConfig {
        name: "xin_she_yang_n1_2d".to_string(),
        function_name: "xin_she_yang_n1".to_string(),
        bounds: vec![(-5.0, 5.0); 2],
        expected_optimum: vec![0.0; 2],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 1500,
        popsize: 80,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 119,
    });

    // XIN-SHE YANG N2
    configs.push(BenchmarkConfig {
        name: "xin_she_yang_n2_2d".to_string(),
        function_name: "xin_she_yang_n2".to_string(),
        bounds: vec![(-5.0, 5.0); 2],
        expected_optimum: vec![0.0; 2],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 1500,
        popsize: 80,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 120,
    });

    // DIFFICULT ILL-CONDITIONED FUNCTIONS

    // ELLIPTIC
    configs.push(BenchmarkConfig {
        name: "elliptic_2d".to_string(),
        function_name: "elliptic".to_string(),
        bounds: vec![(-100.0, 100.0); 2],
        expected_optimum: vec![0.0; 2],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 1200,
        popsize: 60,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 121,
    });

    configs.push(BenchmarkConfig {
        name: "elliptic_5d".to_string(),
        function_name: "elliptic".to_string(),
        bounds: vec![(-100.0, 100.0); 5],
        expected_optimum: vec![0.0; 5],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 2000,
        popsize: 120,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 122,
    });

    // ADDITIONAL BENCHMARKS FROM ORIGINAL TEST FILES

    // BOHACHEVSKY family
    configs.push(BenchmarkConfig {
        name: "bohachevsky1_2d".to_string(),
        function_name: "bohachevsky1".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 400,
        popsize: 30,
        strategy: Strategy::RandToBest1Exp,
        recombination: 0.9,
        seed: 31,
    });

    configs.push(BenchmarkConfig {
        name: "bohachevsky2_2d".to_string(),
        function_name: "bohachevsky2".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 400,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.7,
        seed: 31,
    });

    configs.push(BenchmarkConfig {
        name: "bohachevsky3_2d".to_string(),
        function_name: "bohachevsky3".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 400,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.7,
        seed: 31,
    });

    // BROWN function (ill-conditioned)
    configs.push(BenchmarkConfig {
        name: "brown_2d".to_string(),
        function_name: "brown".to_string(),
        bounds: vec![(-1.0, 4.0), (-1.0, 4.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-5,
        position_tolerance: 1e-3,
        maxiter: 1500,
        popsize: 80,
        strategy: Strategy::Best1Bin,
        recombination: 0.9,
        seed: 110,
    });

    configs.push(BenchmarkConfig {
        name: "brown_4d".to_string(),
        function_name: "brown".to_string(),
        bounds: vec![(-1.0, 4.0); 4],
        expected_optimum: vec![0.0; 4],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 2000,
        popsize: 120,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.95,
        seed: 111,
    });

    // BUKIN N6 (extremely difficult)
    configs.push(BenchmarkConfig {
        name: "bukin_n6_2d".to_string(),
        function_name: "bukin_n6".to_string(),
        bounds: vec![(-15.0, -5.0), (-3.0, 3.0)],
        expected_optimum: vec![-10.0, 1.0],
        fun_tolerance: 1.0, // Very relaxed - Bukin N6 is extremely difficult
        position_tolerance: 2.0,
        maxiter: 4000,
        popsize: 200,
        strategy: Strategy::Rand1Bin,
        recombination: 0.98,
        seed: 26,
    });

    // COSINE MIXTURE function
    configs.push(BenchmarkConfig {
        name: "cosine_mixture_2d".to_string(),
        function_name: "cosine_mixture".to_string(),
        bounds: vec![(-1.0, 1.0), (-1.0, 1.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 0.1,
        position_tolerance: 0.1,
        maxiter: 600,
        popsize: 40,
        strategy: Strategy::RandToBest1Exp,
        recombination: 0.9,
        seed: 82,
    });

    configs.push(BenchmarkConfig {
        name: "cosine_mixture_4d".to_string(),
        function_name: "cosine_mixture".to_string(),
        bounds: vec![(-1.0, 1.0); 4],
        expected_optimum: vec![0.0; 4],
        fun_tolerance: 0.1,
        position_tolerance: 0.1,
        maxiter: 800,
        popsize: 50,
        strategy: Strategy::RandToBest1Exp,
        recombination: 0.9,
        seed: 82,
    });

    // DROP WAVE function
    configs.push(BenchmarkConfig {
        name: "drop_wave_2d".to_string(),
        function_name: "drop_wave".to_string(),
        bounds: vec![(-5.12, 5.12), (-5.12, 5.12)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: -0.99, // Global minimum is -1
        position_tolerance: 0.1,
        maxiter: 2500,
        popsize: 120,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 72,
    });

    // EASOM function
    configs.push(BenchmarkConfig {
        name: "easom_2d".to_string(),
        function_name: "easom".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![std::f64::consts::PI, std::f64::consts::PI],
        fun_tolerance: -0.9, // Global minimum is -1 at (π, π)
        position_tolerance: 1.0,
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 10,
    });

    // ADDITIONAL TESTS FROM REMAINING TEST FILES

    // ACKLEY N2 variant
    configs.push(BenchmarkConfig {
        name: "ackley_n2_2d".to_string(),
        function_name: "ackley_n2".to_string(),
        bounds: vec![(-32.0, 32.0), (-32.0, 32.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-3,
        position_tolerance: 0.1,
        maxiter: 2500,
        popsize: 120,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 43,
    });

    // ACKLEY N3 variant
    configs.push(BenchmarkConfig {
        name: "ackley_n3_2d".to_string(),
        function_name: "ackley_n3".to_string(),
        bounds: vec![(-32.0, 32.0), (-32.0, 32.0)],
        expected_optimum: vec![0.682, -0.367],
        fun_tolerance: -198.0,   // Approximate global minimum
        position_tolerance: 5.0, // Relaxed for extremely difficult function
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 44,
    });

    // BENT CIGAR function
    configs.push(BenchmarkConfig {
        name: "bent_cigar_2d".to_string(),
        function_name: "bent_cigar".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-3,
        position_tolerance: 1e-1,
        maxiter: 800,
        popsize: 40,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 77,
    });

    configs.push(BenchmarkConfig {
        name: "bent_cigar_5d".to_string(),
        function_name: "bent_cigar".to_string(),
        bounds: vec![(-100.0, 100.0); 5],
        expected_optimum: vec![0.0; 5],
        fun_tolerance: 1e3, // Relaxed due to ill-conditioning
        position_tolerance: 1.0,
        maxiter: 1500,
        popsize: 100,
        strategy: Strategy::RandToBest1Exp,
        recombination: 0.95,
        seed: 77,
    });

    // BIRD function
    configs.push(BenchmarkConfig {
        name: "bird_2d".to_string(),
        function_name: "bird".to_string(),
        bounds: vec![
            (-2.0 * std::f64::consts::PI, 2.0 * std::f64::consts::PI),
            (-2.0 * std::f64::consts::PI, 2.0 * std::f64::consts::PI),
        ],
        expected_optimum: vec![4.701, 3.152], // Approximate optimum
        fun_tolerance: 0.0,                   // Just find negative value
        position_tolerance: 2.0,
        maxiter: 3000,
        popsize: 120,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.95,
        seed: 70,
    });

    // CHUNG REYNOLDS function
    configs.push(BenchmarkConfig {
        name: "chung_reynolds_2d".to_string(),
        function_name: "chung_reynolds".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 1000,
        popsize: 60,
        strategy: Strategy::Best1Bin,
        recombination: 0.7,
        seed: 160,
    });

    configs.push(BenchmarkConfig {
        name: "chung_reynolds_5d".to_string(),
        function_name: "chung_reynolds".to_string(),
        bounds: vec![(-100.0, 100.0); 5],
        expected_optimum: vec![0.0; 5],
        fun_tolerance: 1e-4,
        position_tolerance: 0.1,
        maxiter: 1500,
        popsize: 80,
        strategy: Strategy::Rand1Bin,
        recombination: 0.8,
        seed: 161,
    });

    // COLVILLE function
    configs.push(BenchmarkConfig {
        name: "colville_4d".to_string(),
        function_name: "colville".to_string(),
        bounds: vec![(-10.0, 10.0); 4],
        expected_optimum: vec![1.0; 4],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 1200,
        popsize: 80,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 210,
    });

    // CROSS IN TRAY function
    configs.push(BenchmarkConfig {
        name: "cross_in_tray_2d".to_string(),
        function_name: "cross_in_tray".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![1.349, 1.349], // One of the optima
        fun_tolerance: -2.0,                  // Global minimum is -2.06261
        position_tolerance: 0.2,
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 71,
    });

    // DIXONS PRICE function
    configs.push(BenchmarkConfig {
        name: "dixons_price_2d".to_string(),
        function_name: "dixons_price".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![1.0, 0.707], // 2^(-0.5)
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 2500,
        popsize: 120,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 83,
    });

    configs.push(BenchmarkConfig {
        name: "dixons_price_5d".to_string(),
        function_name: "dixons_price".to_string(),
        bounds: vec![(-10.0, 10.0); 5],
        expected_optimum: vec![1.0, 0.707, 0.595, 0.500, 0.420], // 2^(-(2^i - 2)/(2^i))
        fun_tolerance: 1e-2,
        position_tolerance: 0.1,
        maxiter: 4000,
        popsize: 200,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 84,
    });

    // EGGHOLDER function
    configs.push(BenchmarkConfig {
        name: "eggholder_2d".to_string(),
        function_name: "eggholder".to_string(),
        bounds: vec![(-512.0, 512.0), (-512.0, 512.0)],
        expected_optimum: vec![512.0, 404.2319], // Approximate optimum
        fun_tolerance: -700.0,                   // Global minimum is -959.6407
        position_tolerance: 50.0,                // Very relaxed due to extreme difficulty
        maxiter: 5000,
        popsize: 250,
        strategy: Strategy::Rand1Bin,
        recombination: 0.98,
        seed: 27,
    });

    // EPISTATIC MICHALEWICZ function
    configs.push(BenchmarkConfig {
        name: "epistatic_michalewicz_2d".to_string(),
        function_name: "epistatic_michalewicz".to_string(),
        bounds: vec![(0.0, std::f64::consts::PI); 2],
        expected_optimum: vec![2.693, 0.0], // Approximate
        fun_tolerance: -0.8,                // Approximate minimum
        position_tolerance: 0.5,
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 74,
    });

    // EXPONENTIAL function
    configs.push(BenchmarkConfig {
        name: "exponential_2d".to_string(),
        function_name: "exponential".to_string(),
        bounds: vec![(-1.0, 1.0), (-1.0, 1.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: -0.99, // Global minimum is -1
        position_tolerance: 0.1,
        maxiter: 400,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 200,
    });

    // FREUDENSTEIN ROTH function
    configs.push(BenchmarkConfig {
        name: "freudenstein_roth_2d".to_string(),
        function_name: "freudenstein_roth".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![5.0, 4.0],
        fun_tolerance: 1e-3,
        position_tolerance: 0.1,
        maxiter: 800,
        popsize: 40,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 240,
    });

    // GRIEWANK2 function
    configs.push(BenchmarkConfig {
        name: "griewank2_2d".to_string(),
        function_name: "griewank2".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-3,
        position_tolerance: 0.1,
        maxiter: 800,
        popsize: 50,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 250,
    });

    // HAPPY CAT function
    configs.push(BenchmarkConfig {
        name: "happy_cat_2d".to_string(),
        function_name: "happy_cat".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![-1.0, -1.0],
        fun_tolerance: 0.1, // Global minimum is 0
        position_tolerance: 0.1,
        maxiter: 1500,
        popsize: 80,
        strategy: Strategy::RandToBest1Exp,
        recombination: 0.9,
        seed: 30,
    });

    // HARTMAN 3D function
    configs.push(BenchmarkConfig {
        name: "hartman_3d".to_string(),
        function_name: "hartman_3d".to_string(),
        bounds: vec![(0.0, 1.0); 3],
        expected_optimum: vec![0.114, 0.556, 0.852], // Approximate optimum
        fun_tolerance: -3.8,                         // Global minimum is -3.86
        position_tolerance: 0.1,
        maxiter: 1000,
        popsize: 60,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 45,
    });

    // HOLDER TABLE function
    configs.push(BenchmarkConfig {
        name: "holder_table_2d".to_string(),
        function_name: "holder_table".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![8.055, 9.665], // One of the optima
        fun_tolerance: -19.0,                 // Global minimum is -19.2085
        position_tolerance: 0.5,
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 280,
    });

    // LANGERMANN function
    configs.push(BenchmarkConfig {
        name: "langermann_2d".to_string(),
        function_name: "langermann".to_string(),
        bounds: vec![(0.0, 10.0), (0.0, 10.0)],
        expected_optimum: vec![2.808, 8.883], // Approximate
        fun_tolerance: -5.0,                  // Global minimum around -5.16
        position_tolerance: 1.0,
        maxiter: 4000,
        popsize: 200,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 290,
    });

    // LEVI13 function (also known as Levy N.13)
    configs.push(BenchmarkConfig {
        name: "levi13_2d".to_string(),
        function_name: "levi13".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![1.0, 1.0],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 2000,
        popsize: 100,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.95,
        seed: 300,
    });

    // MCCORMICK function
    configs.push(BenchmarkConfig {
        name: "mccormick_2d".to_string(),
        function_name: "mccormick".to_string(),
        bounds: vec![(-1.5, 4.0), (-3.0, 4.0)],
        expected_optimum: vec![-0.54719, -1.54719],
        fun_tolerance: -1.9, // Global minimum is -1.9133
        position_tolerance: 0.01,
        maxiter: 600,
        popsize: 40,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 310,
    });

    // MICHALEWICZ function
    configs.push(BenchmarkConfig {
        name: "michalewicz_2d".to_string(),
        function_name: "michalewicz".to_string(),
        bounds: vec![(0.0, std::f64::consts::PI); 2],
        expected_optimum: vec![2.20, 1.57], // Approximate
        fun_tolerance: -1.8,                // Global minimum around -1.8013
        position_tolerance: 0.1,
        maxiter: 2500,
        popsize: 120,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 320,
    });

    configs.push(BenchmarkConfig {
        name: "michalewicz_5d".to_string(),
        function_name: "michalewicz".to_string(),
        bounds: vec![(0.0, std::f64::consts::PI); 5],
        expected_optimum: vec![2.20, 1.57, 1.28, 1.92, 1.72], // Approximate
        fun_tolerance: -4.5,                                  // Global minimum around -4.687
        position_tolerance: 0.2,
        maxiter: 4000,
        popsize: 200,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 321,
    });

    // PERIODIC function
    configs.push(BenchmarkConfig {
        name: "periodic_2d".to_string(),
        function_name: "periodic".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1.0, // Global minimum is 0.9
        position_tolerance: 0.1,
        maxiter: 800,
        popsize: 50,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 330,
    });

    // PINTER function
    configs.push(BenchmarkConfig {
        name: "pinter_2d".to_string(),
        function_name: "pinter".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-8,
        position_tolerance: 1e-4,
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 340,
    });

    // POWELL function
    configs.push(BenchmarkConfig {
        name: "powell_4d".to_string(),
        function_name: "powell".to_string(),
        bounds: vec![(-4.0, 5.0); 4],
        expected_optimum: vec![0.0; 4],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 1200,
        popsize: 80,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 350,
    });

    configs.push(BenchmarkConfig {
        name: "powell_8d".to_string(),
        function_name: "powell".to_string(),
        bounds: vec![(-4.0, 5.0); 8],
        expected_optimum: vec![0.0; 8],
        fun_tolerance: 1e-2,
        position_tolerance: 0.1,
        maxiter: 1800,
        popsize: 120,
        strategy: Strategy::RandToBest1Exp,
        recombination: 0.95,
        seed: 351,
    });

    // QING function
    configs.push(BenchmarkConfig {
        name: "qing_2d".to_string(),
        function_name: "qing".to_string(),
        bounds: vec![(-500.0, 500.0), (-500.0, 500.0)],
        expected_optimum: vec![1.414, 2.0], // sqrt(i) for i=1,2
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.95,
        seed: 360,
    });

    // QUADRATIC function
    configs.push(BenchmarkConfig {
        name: "quadratic_2d".to_string(),
        function_name: "quadratic".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![0.19388, 0.48513],
        fun_tolerance: -3873.7, // Global minimum
        position_tolerance: 0.01,
        maxiter: 2500,
        popsize: 120,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.95,
        seed: 370,
    });

    // QUARTIC function
    configs.push(BenchmarkConfig {
        name: "quartic_2d".to_string(),
        function_name: "quartic".to_string(),
        bounds: vec![(-1.28, 1.28), (-1.28, 1.28)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 600,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 380,
    });

    configs.push(BenchmarkConfig {
        name: "quartic_10d".to_string(),
        function_name: "quartic".to_string(),
        bounds: vec![(-1.28, 1.28); 10],
        expected_optimum: vec![0.0; 10],
        fun_tolerance: 1e-2,
        position_tolerance: 0.1,
        maxiter: 1200,
        popsize: 80,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 381,
    });

    // ROTATED HYPER ELLIPSOID function
    configs.push(BenchmarkConfig {
        name: "rotated_hyper_ellipsoid_2d".to_string(),
        function_name: "rotated_hyper_ellipsoid".to_string(),
        bounds: vec![(-65.536, 65.536), (-65.536, 65.536)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 600,
        popsize: 40,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 390,
    });

    configs.push(BenchmarkConfig {
        name: "rotated_hyper_ellipsoid_5d".to_string(),
        function_name: "rotated_hyper_ellipsoid".to_string(),
        bounds: vec![(-65.536, 65.536); 5],
        expected_optimum: vec![0.0; 5],
        fun_tolerance: 1e-4,
        position_tolerance: 0.01,
        maxiter: 1000,
        popsize: 60,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 391,
    });

    // SALOMON function
    configs.push(BenchmarkConfig {
        name: "salomon_2d".to_string(),
        function_name: "salomon".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 0.1,
        position_tolerance: 0.1,
        maxiter: 1000,
        popsize: 60,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 400,
    });

    // SCHAFFER N2 function
    configs.push(BenchmarkConfig {
        name: "schaffer_n2_2d".to_string(),
        function_name: "schaffer_n2".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 800,
        popsize: 50,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 410,
    });

    // SCHAFFER N4 function
    configs.push(BenchmarkConfig {
        name: "schaffer_n4_2d".to_string(),
        function_name: "schaffer_n4".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 1.25313],
        fun_tolerance: 0.3, // Global minimum is 0.292579
        position_tolerance: 0.1,
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 420,
    });

    // SCHWEFEL2 function
    configs.push(BenchmarkConfig {
        name: "schwefel2_2d".to_string(),
        function_name: "schwefel2".to_string(),
        bounds: vec![(-100.0, 100.0), (-100.0, 100.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-6,
        position_tolerance: 1e-3,
        maxiter: 800,
        popsize: 50,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 430,
    });

    // STEP function
    configs.push(BenchmarkConfig {
        name: "step_5d".to_string(),
        function_name: "step".to_string(),
        bounds: vec![(-100.0, 100.0); 5],
        expected_optimum: vec![0.0; 5],
        fun_tolerance: 1e-4,
        position_tolerance: 0.5,
        maxiter: 800,
        popsize: 50,
        strategy: Strategy::Rand1Bin,
        recombination: 0.9,
        seed: 440,
    });

    // STYBLINSKI TANG function
    configs.push(BenchmarkConfig {
        name: "styblinski_tang_2d".to_string(),
        function_name: "styblinski_tang2".to_string(),
        bounds: vec![(-5.0, 5.0), (-5.0, 5.0)],
        expected_optimum: vec![-2.903534, -2.903534],
        fun_tolerance: -78.3, // Global minimum is -78.332
        position_tolerance: 0.01,
        maxiter: 800,
        popsize: 40,
        strategy: Strategy::Best1Exp,
        recombination: 0.9,
        seed: 450,
    });

    // SUM OF DIFFERENT POWERS function
    configs.push(BenchmarkConfig {
        name: "sum_of_different_powers_2d".to_string(),
        function_name: "sum_of_different_powers".to_string(),
        bounds: vec![(-1.0, 1.0), (-1.0, 1.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-10,
        position_tolerance: 1e-5,
        maxiter: 500,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 460,
    });

    // THREE HUMP CAMEL function
    configs.push(BenchmarkConfig {
        name: "three_hump_camel_2d".to_string(),
        function_name: "three_hump_camel".to_string(),
        bounds: vec![(-5.0, 5.0), (-5.0, 5.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-8,
        position_tolerance: 1e-4,
        maxiter: 600,
        popsize: 30,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 470,
    });

    // TRID function
    configs.push(BenchmarkConfig {
        name: "trid_6d".to_string(),
        function_name: "trid".to_string(),
        bounds: vec![(-36.0, 36.0); 6],
        expected_optimum: vec![6.0, 10.0, 12.0, 12.0, 10.0, 6.0], // i(d+1-i)
        fun_tolerance: -50.0,                                     // Global minimum is -50 for d=6
        position_tolerance: 0.1,
        maxiter: 3000,
        popsize: 150,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.95,
        seed: 480,
    });

    // VINCENT function
    configs.push(BenchmarkConfig {
        name: "vincent_2d".to_string(),
        function_name: "vincent".to_string(),
        bounds: vec![(0.25, 10.0), (0.25, 10.0)],
        expected_optimum: vec![7.706, 7.706], // Approximate
        fun_tolerance: -2.0,                  // Global minimum around -2 for 2D
        position_tolerance: 2.0,              // Many local minima
        maxiter: 4000,
        popsize: 200,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 490,
    });

    // WHITLEY function
    configs.push(BenchmarkConfig {
        name: "whitley_2d".to_string(),
        function_name: "whitley".to_string(),
        bounds: vec![(-10.24, 10.24), (-10.24, 10.24)],
        expected_optimum: vec![1.0, 1.0],
        fun_tolerance: 1e-4,
        position_tolerance: 1e-2,
        maxiter: 1200,
        popsize: 80,
        strategy: Strategy::RandToBest1Exp,
        recombination: 0.9,
        seed: 500,
    });

    // XIN SHE YANG N3 function
    configs.push(BenchmarkConfig {
        name: "xin_she_yang_n3_2d".to_string(),
        function_name: "xin_she_yang_n3".to_string(),
        bounds: vec![(-20.0, 20.0), (-20.0, 20.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: -1.0, // Global minimum
        position_tolerance: 0.1,
        maxiter: 2500,
        popsize: 120,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 510,
    });

    // XIN SHE YANG N4 function
    configs.push(BenchmarkConfig {
        name: "xin_she_yang_n4_2d".to_string(),
        function_name: "xin_she_yang_n4".to_string(),
        bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: -1.0, // Global minimum
        position_tolerance: 0.1,
        maxiter: 2500,
        popsize: 120,
        strategy: Strategy::RandToBest1Bin,
        recombination: 0.98,
        seed: 520,
    });

    // ZAKHAROV function variants
    configs.push(BenchmarkConfig {
        name: "zakharov_2d".to_string(),
        function_name: "zakharov".to_string(),
        bounds: vec![(-5.0, 10.0), (-5.0, 10.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-8,
        position_tolerance: 1e-4,
        maxiter: 600,
        popsize: 40,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 530,
    });

    configs.push(BenchmarkConfig {
        name: "zakharov2_2d".to_string(),
        function_name: "zakharov2".to_string(),
        bounds: vec![(-5.0, 10.0), (-5.0, 10.0)],
        expected_optimum: vec![0.0, 0.0],
        fun_tolerance: 1e-8,
        position_tolerance: 1e-4,
        maxiter: 600,
        popsize: 40,
        strategy: Strategy::Best1Bin,
        recombination: 0.8,
        seed: 540,
    });

    // Generate benchmark closures from configurations
    for config in configs {
        let config_clone = config.clone();
        benchmarks.insert(
            config.name.clone(),
            Box::new(move || {
                if let Some(function) = FUNCTION_REGISTRY.get(&config_clone.function_name) {
                    run_benchmark(
                        &config_clone.name,
                        function,
                        config_clone.bounds.clone(),
                        DEConfigBuilder::new()
                            .seed(config_clone.seed)
                            .maxiter(config_clone.maxiter)
                            .popsize(config_clone.popsize)
                            .strategy(config_clone.strategy)
                            .recombination(config_clone.recombination)
                            .build(),
                        config_clone.fun_tolerance,
                        config_clone.expected_optimum.clone(),
                        config_clone.position_tolerance,
                    )
                } else {
                    BenchmarkResult {
                        name: config_clone.name.clone(),
                        success: false,
                        fun_value: f64::INFINITY,
                        fun_tolerance: config_clone.fun_tolerance,
                        position_errors: vec![f64::INFINITY],
                        position_tolerance: config_clone.position_tolerance,
                        duration: Duration::from_secs(0),
                        error_message: Some(format!(
                            "Function {} not found in registry",
                            config_clone.function_name
                        )),
                    }
                }
            }),
        );
    }

    benchmarks
}

#[derive(Debug, Clone)]
struct BenchmarkResult {
    name: String,
    success: bool,
    fun_value: f64,
    fun_tolerance: f64,
    position_errors: Vec<f64>,
    position_tolerance: f64,
    duration: Duration,
    error_message: Option<String>,
}

impl fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "✅ PASS" } else { "❌ FAIL" };
        write!(
            f,
            "{} {} (fun: {:.6e} < {:.2e}, pos_errs: max {:.6} < {:.2}, time: {:.2}s)",
            status,
            self.name,
            self.fun_value,
            self.fun_tolerance,
            self.position_errors.iter().fold(0.0f64, |a, &b| a.max(b)),
            self.position_tolerance,
            self.duration.as_secs_f64()
        )?;
        if let Some(ref err) = self.error_message {
            write!(f, " - {}", err)?;
        }
        Ok(())
    }
}

/// Run a benchmark with the given parameters and validate results
fn run_benchmark(
    name: &str,
    function: fn(&Array1<f64>) -> f64,
    bounds: Vec<(f64, f64)>,
    config: autoeq_de::DEConfig,
    fun_tolerance: f64,
    expected_optimum: Vec<f64>,
    position_tolerance: f64,
) -> BenchmarkResult {
    let start_time = Instant::now();

    let result = run_recorded_differential_evolution(name, function, &bounds, config);
    let duration = start_time.elapsed();

    match result {
        Ok((report, _csv_path)) => {
            let fun_ok = report.fun < fun_tolerance;

            let position_errors: Vec<f64> = report
                .x
                .iter()
                .zip(expected_optimum.iter())
                .map(|(actual, expected)| (actual - expected).abs())
                .collect();

            let position_ok = position_errors.iter().all(|&err| err < position_tolerance);

            let success = fun_ok && position_ok;
            let error_message = if !success {
                let mut msgs = Vec::new();
                if !fun_ok {
                    msgs.push(format!(
                        "fun value {:.6e} >= {:.2e}",
                        report.fun, fun_tolerance
                    ));
                }
                if !position_ok {
                    let max_err = position_errors.iter().fold(0.0f64, |a, &b| a.max(b));
                    msgs.push(format!(
                        "max position error {:.6} >= {:.2}",
                        max_err, position_tolerance
                    ));
                }
                Some(msgs.join(", "))
            } else {
                None
            };

            BenchmarkResult {
                name: name.to_string(),
                success,
                fun_value: report.fun,
                fun_tolerance,
                position_errors,
                position_tolerance,
                duration,
                error_message,
            }
        }
        Err(e) => BenchmarkResult {
            name: name.to_string(),
            success: false,
            fun_value: f64::INFINITY,
            fun_tolerance,
            position_errors: vec![f64::INFINITY],
            position_tolerance,
            duration,
            error_message: Some(format!("Optimization failed: {}", e)),
        },
    }
}

fn main() {
    let matches = Command::new("benchmark_convergence")
        .version("0.1.0")
        .about("Runs optimization convergence benchmarks and reports success/failure")
        .arg(
            Arg::new("filter")
                .short('f')
                .long("filter")
                .value_name("PATTERN")
                .help("Only run benchmarks matching this pattern")
                .num_args(1),
        )
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .help("List available benchmarks")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Show detailed results for each benchmark")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Generate benchmarks dynamically from function metadata
    let benchmarks = generate_all_benchmarks();

    if matches.get_flag("list") {
        println!("Available benchmarks:");
        let mut names: Vec<_> = benchmarks.keys().collect();
        names.sort();
        for name in names {
            println!("  {}", name);
        }
        return;
    }

    let filter = matches.get_one::<String>("filter");
    let verbose = matches.get_flag("verbose");

    // Filter benchmarks if a pattern is provided
    let mut selected_benchmarks: Vec<_> = benchmarks
        .keys()
        .filter(|name| {
            if let Some(pattern) = filter {
                name.contains(pattern)
            } else {
                true
            }
        })
        .collect();
    selected_benchmarks.sort();

    if selected_benchmarks.is_empty() {
        eprintln!("No benchmarks match the filter criteria");
        std::process::exit(1);
    }

    println!("Running {} benchmark(s)...", selected_benchmarks.len());

    let mut results = Vec::new();
    let total_start = Instant::now();

    for &name in &selected_benchmarks {
        println!("Running {}...", name);
        let benchmark_fn = &benchmarks[name];
        let result = benchmark_fn();

        if verbose {
            println!("  {}", result);
        } else if result.success {
            println!("  ✅ PASS");
        } else {
            println!(
                "  ❌ FAIL - {}",
                result
                    .error_message
                    .as_ref()
                    .unwrap_or(&"Unknown error".to_string())
            );
        }

        results.push(result);
    }

    let total_duration = total_start.elapsed();

    // Summary
    println!("\n=== BENCHMARK SUMMARY ===");
    let passed = results.iter().filter(|r| r.success).count();
    let failed = results.len() - passed;

    println!("Passed: {} / {}", passed, results.len());
    println!("Failed: {}", failed);
    println!("Total time: {:.2}s", total_duration.as_secs_f64());

    if verbose || failed > 0 {
        println!("\nDetailed results:");
        for result in &results {
            println!("  {}", result);
        }
    }

    if failed == 0 {
        println!("\n🎉 All benchmarks passed!");
        std::process::exit(0);
    } else {
        println!("\n💥 {} benchmark(s) failed", failed);
        std::process::exit(1);
    }
}
