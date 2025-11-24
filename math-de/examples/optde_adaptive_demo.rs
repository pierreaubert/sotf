use autoeq_de::{AdaptiveConfig, DEConfigBuilder, Mutation, Strategy, differential_evolution};
use autoeq_testfunctions::{ackley, quadratic, rosenbrock};
use ndarray::Array1;

/// Adaptive Differential Evolution Demo
///
/// This example demonstrates the new adaptive features based on the SAM (Self-Adaptive Mutation)
/// and WLS (Wrapper Local Search) strategies from the paper:
/// "Enhanced Differential Evolution Based on Adaptive Mutation and Wrapper Local Search Strategies
/// for Global Optimization Problems"
fn main() {
    println!("🧬 Adaptive Differential Evolution Demo");
    println!("=====================================");
    println!();

    // Test functions to evaluate
    let test_functions = [
        (
            "Quadratic (f(x) = x₁² + x₂²)",
            quadratic as fn(&Array1<f64>) -> f64,
            [(-5.0, 5.0), (-5.0, 5.0)],
        ),
        (
            "Rosenbrock 2D",
            rosenbrock as fn(&Array1<f64>) -> f64,
            [(-5.0, 5.0), (-5.0, 5.0)],
        ),
        ("Ackley", ackley, [(-32.0, 32.0), (-32.0, 32.0)]),
    ];

    for (name, func, bounds) in test_functions.iter() {
        println!("🎯 Function: {}", name);
        println!(
            "   Bounds: [{:.1}, {:.1}] × [{:.1}, {:.1}]",
            bounds[0].0, bounds[0].1, bounds[1].0, bounds[1].1
        );

        // Traditional DE
        println!("   📊 Traditional DE:");
        let traditional_result = run_traditional_de(*func, bounds);

        // Adaptive DE with SAM only
        println!("   🧬 Adaptive DE (SAM only):");
        let sam_result = run_adaptive_de(*func, bounds, false);

        // Adaptive DE with SAM + WLS
        println!("   🔧 Adaptive DE (SAM + WLS):");
        let sam_wls_result = run_adaptive_de(*func, bounds, true);

        // Compare results
        println!("   🏆 Comparison:");
        println!(
            "      Traditional: f = {:.6e}, {} iterations",
            traditional_result.fun, traditional_result.nit
        );
        println!(
            "      SAM only:    f = {:.6e}, {} iterations",
            sam_result.fun, sam_result.nit
        );
        println!(
            "      SAM + WLS:   f = {:.6e}, {} iterations",
            sam_wls_result.fun, sam_wls_result.nit
        );

        let improvement_sam =
            ((traditional_result.fun - sam_result.fun) / traditional_result.fun * 100.0).max(0.0);
        let improvement_wls =
            ((traditional_result.fun - sam_wls_result.fun) / traditional_result.fun * 100.0)
                .max(0.0);

        println!("      📈 Improvement with SAM: {:.1}%", improvement_sam);
        println!("      📈 Improvement with WLS: {:.1}%", improvement_wls);
        println!();
    }

    // Demonstrate parameter adaptation tracking
    println!("🔄 Parameter Adaptation Demo");
    println!("===========================");

    // Use a recording callback to track parameter evolution
    let bounds = [(-5.0, 5.0), (-5.0, 5.0)];

    let adaptive_config = AdaptiveConfig {
        adaptive_mutation: true,
        wls_enabled: true,
        w_max: 0.9,     // Start with 90% of population for selection
        w_min: 0.1,     // End with 10% of population
        w_f: 0.9,       // F parameter adaptation rate
        w_cr: 0.9,      // CR parameter adaptation rate
        f_m: 0.5,       // Initial F location parameter
        cr_m: 0.6,      // Initial CR location parameter
        wls_prob: 0.2,  // Apply WLS to 20% of population
        wls_scale: 0.1, // WLS perturbation scale
    };

    let config = DEConfigBuilder::new()
        .seed(42)
        .maxiter(50)
        .popsize(40)
        .strategy(Strategy::AdaptiveBin)
        .mutation(Mutation::Adaptive { initial_f: 0.8 })
        .adaptive(adaptive_config)
        .disp(true) // Enable progress display
        .build();

    println!("Running adaptive DE on Rosenbrock function with progress display...");
    let result = differential_evolution(&rosenbrock, &bounds, config);

    println!(
        "Final result: f = {:.6e} at x = [{:.4}, {:.4}]",
        result.fun, result.x[0], result.x[1]
    );
    println!(
        "Converged in {} iterations with {} function evaluations",
        result.nit, result.nfev
    );

    if result.success {
        println!("✅ Optimization succeeded: {}", result.message);
    } else {
        println!("⚠️ Optimization status: {}", result.message);
    }
}

fn run_traditional_de(func: fn(&Array1<f64>) -> f64, bounds: &[(f64, f64)]) -> autoeq_de::DEReport {
    let config = DEConfigBuilder::new()
        .seed(42)
        .maxiter(100)
        .popsize(30)
        .strategy(Strategy::Best1Bin)
        .mutation(Mutation::Factor(0.8))
        .recombination(0.7)
        .build();

    differential_evolution(&func, bounds, config)
}

fn run_adaptive_de(
    func: fn(&Array1<f64>) -> f64,
    bounds: &[(f64, f64)],
    enable_wls: bool,
) -> autoeq_de::DEReport {
    let adaptive_config = AdaptiveConfig {
        adaptive_mutation: true,
        wls_enabled: enable_wls,
        w_max: 0.9,
        w_min: 0.1,
        wls_prob: 0.15,
        wls_scale: 0.1,
        ..AdaptiveConfig::default()
    };

    let config = DEConfigBuilder::new()
        .seed(42)
        .maxiter(100)
        .popsize(30)
        .strategy(Strategy::AdaptiveBin)
        .mutation(Mutation::Adaptive { initial_f: 0.8 })
        .adaptive(adaptive_config)
        .build();

    differential_evolution(&func, bounds, config)
}
