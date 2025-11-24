use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::ackley_n2;

#[test]
fn test_de_ackley_n2_basic() {
    // Test Ackley N.2 function - challenging multimodal function
    let b = vec![(-32.0, 32.0), (-32.0, 32.0)];
    let c = DEConfigBuilder::new()
        .seed(87)
        .maxiter(1200)
        .popsize(70)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution("ackley_n2_basic", ackley_n2, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum f(x*) = -200 at x = (0, 0)
    assert!(
        report.fun < -190.0,
        "Function value too high: {}",
        report.fun
    ); // Should find solution close to global minimum
}

#[test]
fn test_de_ackley_n2_focused_bounds() {
    // Test with bounds closer to the optimum
    let b = vec![(-5.0, 5.0), (-5.0, 5.0)]; // Smaller bounds around the global optimum
    let c = DEConfigBuilder::new()
        .seed(88)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("ackley_n2_focused", ackley_n2, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Should find better solution with focused bounds
    assert!(
        report.fun < -195.0,
        "Function value too high with focused bounds: {}",
        report.fun
    );
    // Check solution is close to (0, 0)
    for (i, &xi) in report.x.iter().enumerate() {
        assert!(xi.abs() < 0.5, "x[{}] should be close to 0: {}", i, xi);
    }
}

#[test]
fn test_de_ackley_n2_multistart() {
    // Test with multiple random starts to verify robustness
    let b = vec![(-20.0, 20.0), (-20.0, 20.0)];
    let seeds = [700, 800, 900];

    let mut best_result = f64::INFINITY;
    let mut results = Vec::new();

    for (i, &seed) in seeds.iter().enumerate() {
        let c = DEConfigBuilder::new()
            .seed(seed)
            .maxiter(1500)
            .popsize(100)
            .strategy(Strategy::Rand1Bin)
            .recombination(0.8)
            .build();
        let result = run_recorded_differential_evolution(
            &format!("ackley_n2_multistart_{}", i),
            ackley_n2,
            &b,
            c,
        );
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        best_result = best_result.min(report.fun);
        results.push(report.fun);
        println!("Run {} (seed {}): f = {}", i, seed, report.fun);
    }

    // At least one run should find a good solution
    assert!(
        best_result < -180.0,
        "Best result across runs too high: {}",
        best_result
    );

    // Most runs should find reasonable solutions (multimodal, but should converge)
    let good_runs = results.iter().filter(|&&f| f < -150.0).count();
    assert!(
        good_runs >= 2,
        "Too few good runs: {} out of {}",
        good_runs,
        results.len()
    );
}

#[test]
fn test_de_ackley_n2_large_bounds() {
    // Test with the standard large bounds
    let b = vec![(-32.0, 32.0), (-32.0, 32.0)];
    let c = DEConfigBuilder::new()
        .seed(89)
        .maxiter(2000)
        .popsize(150)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("ackley_n2_large", ackley_n2, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Should still find decent solution even with large bounds
    assert!(
        report.fun < -150.0,
        "Function value too high with large bounds: {}",
        report.fun
    );
}
