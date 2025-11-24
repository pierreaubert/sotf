use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::booth;

#[test]
fn test_de_booth() {
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(5)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("booth", booth, &bounds, config);

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Booth function: Global minimum f(x) = 0 at x = (1, 3)
    assert!(report.fun < 1e-5);

    // Check that solution is close to expected optimum
    let expected = [1.0, 3.0];
    for (actual, expected) in report.x.iter().zip(expected.iter()) {
        assert!(
            (actual - expected).abs() < 0.1,
            "Solution component {} should be close to {}",
            actual,
            expected
        );
    }
}

#[test]
fn test_de_booth_convergence() {
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(42)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();

    let result = run_recorded_differential_evolution("booth_convergence", booth, &bounds, config);

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Should achieve high precision (keeping stricter 1e-6 criterion)
    assert!(report.fun < 1e-6);

    // Check that solution is close to expected optimum (1, 3)
    let expected = [1.0, 3.0];
    for (actual, expected) in report.x.iter().zip(expected.iter()) {
        assert!((actual - expected).abs() < 0.1);
    }
}
