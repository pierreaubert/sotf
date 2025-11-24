use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::lampinen_simplified;

#[test]
fn test_de_lampinen_simplified() {
    // Test Lampinen simplified using direct DE interface
    let b6 = vec![(0.0, 1.0); 6]; // Simplified bounds
    let c6 = DEConfigBuilder::new()
        .seed(50)
        .maxiter(500)
        .popsize(60)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();

    let result =
        run_recorded_differential_evolution("lampinen_simplified", lampinen_simplified, &b6, c6);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // For this simplified version, optimum should be at bounds
    // x[0..4] should be around 2.5 (but clamped to 1.0 by bounds)
    // x[4..] should be at 0.0
    for i in 0..4.min(report.x.len()) {
        assert!(
            report.x[i] > 0.5,
            "First 4 variables should be large: x[{}] = {}",
            i,
            report.x[i]
        );
    }
    for i in 4..report.x.len() {
        assert!(
            report.x[i] < 0.5,
            "Last variables should be small: x[{}] = {}",
            i,
            report.x[i]
        );
    }

    // Should reach the optimal value close to -16.0
    assert!(
        report.fun < -15.0,
        "Function value should be good: {}",
        report.fun
    );
}

#[test]
fn test_lampinen_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected

    // At optimal solution (all first 4 variables at 1.0, last 2 at 0.0)
    let x_optimal = Array1::from(vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    let f_optimal = lampinen_simplified(&x_optimal);

    // Should be -(4 * 5 * 1 - 4 * 1 + 0) = -(20 - 4) = -16
    assert!(
        (f_optimal - (-16.0)).abs() < 1e-10,
        "Optimal value should be -16.0: {}",
        f_optimal
    );

    // Test suboptimal solution
    let x_suboptimal = Array1::from(vec![0.5, 0.5, 0.5, 0.5, 0.5, 0.5]);
    let f_suboptimal = lampinen_simplified(&x_suboptimal);

    // Should be worse than optimal
    assert!(
        f_suboptimal > f_optimal,
        "Suboptimal should be worse: {} vs {}",
        f_suboptimal,
        f_optimal
    );

    println!(
        "Lampinen function test: optimal = {:.6}, suboptimal = {:.6}",
        f_optimal, f_suboptimal
    );
}
