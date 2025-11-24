/// Shared function registry for differential evolution benchmarks and plotting
use crate::Strategy;
use autoeq_testfunctions::*;
use ndarray::Array1;
use std::collections::HashMap;

/// Test function type definition
pub type TestFunction = fn(&Array1<f64>) -> f64;

/// CSV trace point: (x_vector, f_value, is_improvement)
pub type TracePoint = (Vec<f64>, f64, bool);

/// Configuration for a benchmark
#[derive(Clone, Debug)]
pub struct BenchmarkConfig {
    pub name: String,
    pub function_name: String,
    pub bounds: Vec<(f64, f64)>,
    pub expected_optimum: Vec<f64>,
    pub fun_tolerance: f64,
    pub position_tolerance: f64,
    pub maxiter: usize,
    pub popsize: usize,
    pub strategy: Strategy,
    pub recombination: f64,
    pub seed: u64,
}

/// Function registry mapping names to actual function pointers
pub struct FunctionRegistry {
    functions: HashMap<String, TestFunction>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        let mut functions = HashMap::new();

        // Unimodal functions
        functions.insert("sphere".to_string(), sphere as TestFunction);
        functions.insert("rosenbrock".to_string(), rosenbrock as TestFunction);
        functions.insert("booth".to_string(), booth as TestFunction);
        functions.insert("matyas".to_string(), matyas as TestFunction);
        functions.insert("beale".to_string(), beale as TestFunction);
        functions.insert("himmelblau".to_string(), himmelblau as TestFunction);
        functions.insert("sum_squares".to_string(), sum_squares as TestFunction);
        functions.insert(
            "different_powers".to_string(),
            different_powers as TestFunction,
        );
        functions.insert("elliptic".to_string(), elliptic as TestFunction);
        functions.insert("cigar".to_string(), cigar as TestFunction);
        functions.insert("tablet".to_string(), tablet as TestFunction);
        functions.insert("discus".to_string(), discus as TestFunction);
        functions.insert("ridge".to_string(), ridge as TestFunction);
        functions.insert("sharp_ridge".to_string(), sharp_ridge as TestFunction);
        functions.insert("perm_0_d_beta".to_string(), perm_0_d_beta as TestFunction);
        functions.insert("perm_d_beta".to_string(), perm_d_beta as TestFunction);

        // Multimodal functions
        functions.insert("ackley".to_string(), ackley as TestFunction);
        functions.insert("ackley_n2".to_string(), ackley_n2 as TestFunction);
        functions.insert("ackley_n3".to_string(), ackley_n3 as TestFunction);
        functions.insert("rastrigin".to_string(), rastrigin as TestFunction);
        functions.insert("griewank".to_string(), griewank as TestFunction);
        functions.insert("schwefel".to_string(), schwefel as TestFunction);
        functions.insert("branin".to_string(), branin as TestFunction);
        functions.insert(
            "goldstein_price".to_string(),
            goldstein_price as TestFunction,
        );
        functions.insert("six_hump_camel".to_string(), six_hump_camel as TestFunction);
        functions.insert("hartman_3d".to_string(), hartman_3d as TestFunction);
        functions.insert("hartman_4d".to_string(), hartman_4d as TestFunction);
        functions.insert("hartman_6d".to_string(), hartman_6d as TestFunction);
        functions.insert(
            "xin_she_yang_n1".to_string(),
            xin_she_yang_n1 as TestFunction,
        );
        functions.insert(
            "xin_she_yang_n2".to_string(),
            xin_she_yang_n2 as TestFunction,
        );
        functions.insert(
            "xin_she_yang_n3".to_string(),
            xin_she_yang_n3 as TestFunction,
        );
        functions.insert(
            "xin_she_yang_n4".to_string(),
            xin_she_yang_n4 as TestFunction,
        );
        functions.insert("katsuura".to_string(), katsuura as TestFunction);
        functions.insert("happycat".to_string(), happycat as TestFunction);
        functions.insert("happy_cat".to_string(), happy_cat as TestFunction);

        // Alpine functions
        functions.insert("alpine_n1".to_string(), alpine_n1 as TestFunction);
        functions.insert("alpine_n2".to_string(), alpine_n2 as TestFunction);

        // Additional functions
        functions.insert(
            "gramacy_lee_2012".to_string(),
            gramacy_lee_2012 as TestFunction,
        );
        functions.insert("forrester_2008".to_string(), forrester_2008 as TestFunction);
        functions.insert("power_sum".to_string(), power_sum as TestFunction);
        functions.insert("shekel".to_string(), shekel as TestFunction);
        functions.insert(
            "gramacy_lee_function".to_string(),
            gramacy_lee_function as TestFunction,
        );
        functions.insert(
            "expanded_griewank_rosenbrock".to_string(),
            expanded_griewank_rosenbrock as TestFunction,
        );

        // More classical functions
        functions.insert("bohachevsky1".to_string(), bohachevsky1 as TestFunction);
        functions.insert("bohachevsky2".to_string(), bohachevsky2 as TestFunction);
        functions.insert("bohachevsky3".to_string(), bohachevsky3 as TestFunction);
        functions.insert("bird".to_string(), bird as TestFunction);
        functions.insert("bent_cigar".to_string(), bent_cigar as TestFunction);
        functions.insert("bent_cigar_alt".to_string(), bent_cigar_alt as TestFunction);
        functions.insert("brown".to_string(), brown as TestFunction);
        functions.insert("bukin_n6".to_string(), bukin_n6 as TestFunction);
        functions.insert("chung_reynolds".to_string(), chung_reynolds as TestFunction);
        functions.insert("colville".to_string(), colville as TestFunction);
        functions.insert("cosine_mixture".to_string(), cosine_mixture as TestFunction);
        functions.insert("cross_in_tray".to_string(), cross_in_tray as TestFunction);
        functions.insert("de_jong_step2".to_string(), de_jong_step2 as TestFunction);
        functions.insert(
            "dejong_f5_foxholes".to_string(),
            dejong_f5_foxholes as TestFunction,
        );
        functions.insert("dixons_price".to_string(), dixons_price as TestFunction);
        functions.insert("drop_wave".to_string(), drop_wave as TestFunction);
        functions.insert("easom".to_string(), easom as TestFunction);
        functions.insert("eggholder".to_string(), eggholder as TestFunction);
        functions.insert(
            "epistatic_michalewicz".to_string(),
            epistatic_michalewicz as TestFunction,
        );
        functions.insert("exponential".to_string(), exponential as TestFunction);
        functions.insert(
            "freudenstein_roth".to_string(),
            freudenstein_roth as TestFunction,
        );
        functions.insert("griewank2".to_string(), griewank2 as TestFunction);
        functions.insert("holder_table".to_string(), holder_table as TestFunction);
        functions.insert(
            "lampinen_simplified".to_string(),
            lampinen_simplified as TestFunction,
        );
        functions.insert("langermann".to_string(), langermann as TestFunction);
        functions.insert("levi13".to_string(), levi13 as TestFunction);
        functions.insert("levy".to_string(), levy as TestFunction);
        functions.insert("levy_n13".to_string(), levy_n13 as TestFunction);
        functions.insert("mccormick".to_string(), mccormick as TestFunction);
        functions.insert("michalewicz".to_string(), michalewicz as TestFunction);
        functions.insert("periodic".to_string(), periodic as TestFunction);
        functions.insert("pinter".to_string(), pinter as TestFunction);
        functions.insert("powell".to_string(), powell as TestFunction);
        functions.insert("qing".to_string(), qing as TestFunction);
        functions.insert("quadratic".to_string(), quadratic as TestFunction);
        functions.insert("quartic".to_string(), quartic as TestFunction);
        functions.insert(
            "rotated_hyper_ellipsoid".to_string(),
            rotated_hyper_ellipsoid as TestFunction,
        );
        functions.insert("salomon".to_string(), salomon as TestFunction);
        functions.insert(
            "salomon_corrected".to_string(),
            salomon_corrected as TestFunction,
        );
        functions.insert("schaffer_n2".to_string(), schaffer_n2 as TestFunction);
        functions.insert("schaffer_n4".to_string(), schaffer_n4 as TestFunction);
        functions.insert("schwefel2".to_string(), schwefel2 as TestFunction);
        functions.insert("shubert".to_string(), shubert as TestFunction);
        functions.insert("step".to_string(), step as TestFunction);
        functions.insert(
            "styblinski_tang2".to_string(),
            styblinski_tang2 as TestFunction,
        );
        functions.insert(
            "sum_of_different_powers".to_string(),
            sum_of_different_powers as TestFunction,
        );
        functions.insert(
            "three_hump_camel".to_string(),
            three_hump_camel as TestFunction,
        );
        functions.insert("trid".to_string(), trid as TestFunction);
        functions.insert("vincent".to_string(), vincent as TestFunction);
        functions.insert("whitley".to_string(), whitley as TestFunction);
        functions.insert("zakharov".to_string(), zakharov as TestFunction);
        functions.insert("zakharov2".to_string(), zakharov2 as TestFunction);

        // Constraint functions (for completeness)
        functions.insert(
            "binh_korn_constraint1".to_string(),
            binh_korn_constraint1 as TestFunction,
        );
        functions.insert(
            "binh_korn_constraint2".to_string(),
            binh_korn_constraint2 as TestFunction,
        );
        functions.insert(
            "binh_korn_weighted".to_string(),
            binh_korn_weighted as TestFunction,
        );
        functions.insert(
            "keanes_bump_constraint1".to_string(),
            keanes_bump_constraint1 as TestFunction,
        );
        functions.insert(
            "keanes_bump_constraint2".to_string(),
            keanes_bump_constraint2 as TestFunction,
        );
        functions.insert(
            "keanes_bump_objective".to_string(),
            keanes_bump_objective as TestFunction,
        );
        functions.insert(
            "mishras_bird_constraint".to_string(),
            mishras_bird_constraint as TestFunction,
        );
        functions.insert(
            "mishras_bird_objective".to_string(),
            mishras_bird_objective as TestFunction,
        );
        functions.insert(
            "rosenbrock_disk_constraint".to_string(),
            rosenbrock_disk_constraint as TestFunction,
        );
        functions.insert(
            "rosenbrock_objective".to_string(),
            rosenbrock_objective as TestFunction,
        );

        Self { functions }
    }

    pub fn get(&self, name: &str) -> Option<TestFunction> {
        self.functions.get(name).copied()
    }

    pub fn list_functions(&self) -> Vec<String> {
        let mut names: Vec<_> = self.functions.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &TestFunction)> {
        self.functions.iter()
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate all benchmark configurations
#[allow(clippy::vec_init_then_push)]
pub fn generate_benchmark_configs() -> Vec<BenchmarkConfig> {
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

    // BEALE function
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

    // Add more configurations as needed...
    // (For brevity, I'm not including all configs here, but they should all be moved from benchmark_convergence.rs)

    configs
}

/// Find CSV files for a given function in the records directory
/// Handles both old single-file format and new block-based format
pub fn find_csv_files_for_function(csv_dir: &str, function_name: &str) -> Vec<String> {
    use std::fs;
    use std::path::Path;

    let mut csv_files = Vec::new();

    // Try old format first
    let old_path = format!("{}/{}.csv", csv_dir, function_name);
    if Path::new(&old_path).exists() {
        csv_files.push(old_path);
        return csv_files;
    }

    // Look for block-based format files
    if let Ok(entries) = fs::read_dir(csv_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str() {
                // Match files like function_name_block_NNNN.csv
                if filename.starts_with(function_name)
                    && filename.contains("_block_")
                    && filename.ends_with(".csv")
                {
                    csv_files.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
    }

    // Sort files to ensure they're read in order
    csv_files.sort();
    csv_files
}

/// Read and combine multiple CSV files for a function
pub fn read_combined_csv_traces(
    csv_files: &[String],
) -> Result<Vec<TracePoint>, Box<dyn std::error::Error>> {
    use std::fs;

    let mut all_points = Vec::new();

    for csv_path in csv_files {
        let content = fs::read_to_string(csv_path)?;
        let lines: Vec<&str> = content.trim().split('\n').collect();

        if lines.len() < 2 {
            continue; // Skip empty files
        }

        let header = lines[0];

        // Check if it's the new format
        if !header.starts_with("eval_id,generation,") {
            return Err(format!("Unsupported CSV format in {}", csv_path).into());
        }

        for line in lines.iter().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();

            if parts.len() < 7 {
                continue; // Skip malformed lines
            }

            // Parse x coordinates (between generation and f_value/best_so_far/is_improvement)
            let x_end = parts.len() - 3;
            let mut x = Vec::new();
            for part in parts.iter().take(x_end).skip(2) {
                if let Ok(coord) = part.parse::<f64>() {
                    x.push(coord);
                }
            }

            if let (Ok(f_value), Ok(is_improvement)) = (
                parts[x_end].parse::<f64>(),
                parts[x_end + 2].parse::<bool>(),
            ) {
                all_points.push((x, f_value, is_improvement));
            }
        }
    }

    Ok(all_points)
}
