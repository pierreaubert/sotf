use autoeq_de::{DEConfig, Mutation, ParallelConfig, Strategy, differential_evolution};
use ndarray::Array1;
use std::time::Instant;

fn main() {
    // Rastrigin function with artificial compute delay to simulate expensive evaluations
    let dimension = 10;
    let rastrigin = move |x: &Array1<f64>| -> f64 {
        // Add some compute-intensive work to make parallelization beneficial
        let mut sum = 0.0;
        for _ in 0..1000 {
            for &xi in x.iter() {
                sum += xi.sin().cos().exp().ln_1p();
            }
        }

        // Actual Rastrigin function
        let a = 10.0;
        let n = x.len() as f64;
        let result = a * n
            + x.iter()
                .map(|&xi| xi * xi - a * (2.0 * std::f64::consts::PI * xi).cos())
                .sum::<f64>();
        result + sum * 1e-10 // Add tiny contribution from expensive computation
    };

    let bounds: Vec<(f64, f64)> = vec![(-5.12, 5.12); dimension];

    // Test sequential evaluation
    println!("Testing Sequential Evaluation:");
    let mut cfg_seq = DEConfig::default();
    cfg_seq.maxiter = 100;
    cfg_seq.popsize = 15;
    cfg_seq.strategy = Strategy::Best1Bin;
    cfg_seq.mutation = Mutation::Factor(0.8);
    cfg_seq.recombination = 0.9;
    cfg_seq.seed = Some(42);
    cfg_seq.disp = true;
    cfg_seq.parallel.enabled = false; // Disable parallel evaluation

    let start_seq = Instant::now();
    let report_seq = differential_evolution(&rastrigin, &bounds, cfg_seq);
    let duration_seq = start_seq.elapsed();

    println!("\nSequential Results:");
    println!("  Success: {}", report_seq.success);
    println!("  Best f: {:.6e}", report_seq.fun);
    println!("  Iterations: {}", report_seq.nit);
    println!("  Function evaluations: {}", report_seq.nfev);
    println!("  Time: {:.3} seconds", duration_seq.as_secs_f64());

    // Test parallel evaluation
    println!("\n\nTesting Parallel Evaluation:");
    let mut cfg_par = DEConfig::default();
    cfg_par.maxiter = 100;
    cfg_par.popsize = 15;
    cfg_par.strategy = Strategy::Best1Bin;
    cfg_par.mutation = Mutation::Factor(0.8);
    cfg_par.recombination = 0.9;
    cfg_par.seed = Some(42);
    cfg_par.disp = true;
    cfg_par.parallel = ParallelConfig {
        enabled: true,
        num_threads: None, // Use all available cores
    };

    let start_par = Instant::now();
    let report_par = differential_evolution(&rastrigin, &bounds, cfg_par);
    let duration_par = start_par.elapsed();

    println!("\nParallel Results:");
    println!("  Success: {}", report_par.success);
    println!("  Best f: {:.6e}", report_par.fun);
    println!("  Iterations: {}", report_par.nit);
    println!("  Function evaluations: {}", report_par.nfev);
    println!("  Time: {:.3} seconds", duration_par.as_secs_f64());

    // Compare results
    println!("\n\nComparison:");
    println!(
        "  Speedup: {:.2}x",
        duration_seq.as_secs_f64() / duration_par.as_secs_f64()
    );
    println!(
        "  Result difference: {:.6e}",
        (report_seq.fun - report_par.fun).abs()
    );

    // Test with different thread counts
    println!("\n\nTesting with different thread counts:");
    for num_threads in [1, 2, 4, 8] {
        let mut cfg_threads = DEConfig::default();
        cfg_threads.maxiter = 50;
        cfg_threads.popsize = 15;
        cfg_threads.strategy = Strategy::Best1Bin;
        cfg_threads.mutation = Mutation::Factor(0.8);
        cfg_threads.recombination = 0.9;
        cfg_threads.seed = Some(42);
        cfg_threads.disp = false;
        cfg_threads.parallel = ParallelConfig {
            enabled: true,
            num_threads: Some(num_threads),
        };

        let start = Instant::now();
        let _ = differential_evolution(&rastrigin, &bounds, cfg_threads);
        let duration = start.elapsed();

        println!(
            "  {} thread(s): {:.3} seconds",
            num_threads,
            duration.as_secs_f64()
        );
    }
}
