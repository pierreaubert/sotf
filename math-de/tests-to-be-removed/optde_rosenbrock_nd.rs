use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::rosenbrock;

#[test]
fn test_de_rosenbrock_10d() {
    // Test 10D Rosenbrock
    let b10 = vec![(-2.048, 2.048); 10];
    let c10 = DEConfigBuilder::new()
        .seed(49)
        .maxiter(2000)
        .popsize(150)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    {
        let result = run_recorded_differential_evolution("rosenbrock_10d", rosenbrock, &b10, c10);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-1)
    };
}

#[test]
fn test_de_rosenbrock_2d() {
    // Test Rosenbrock with recording (2D version)
    let b2 = vec![(-2.048, 2.048), (-2.048, 2.048)];
    let config = DEConfigBuilder::new()
        .seed(48)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("rosenbrock_2d", rosenbrock, &b2, config);
    assert!(result.is_ok(), "Recorded optimization should succeed");

    let (solution, _csv_path) = result.unwrap();
    assert!(
        solution.fun < 1e-4,
        "Solution quality should be good: {}",
        solution.fun
    );

    // Check that solution is close to (1, 1)
    assert!(
        (solution.x[0] - 1.0).abs() < 1e-2,
        "x[0] should be close to 1.0: {}",
        solution.x[0]
    );
    assert!(
        (solution.x[1] - 1.0).abs() < 1e-2,
        "x[1] should be close to 1.0: {}",
        solution.x[1]
    );
}
