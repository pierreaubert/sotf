use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::qing;

#[test]
fn test_de_qing_2d() {
    // Test Qing function in 2D - separable multimodal function
    let bounds = vec![(-500.0, 500.0), (-500.0, 500.0)];
    let config = DEConfigBuilder::new()
        .seed(220)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();

    let result = run_recorded_differential_evolution("qing_2d", qing, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Global minimum is at (√1, √2) = (1, 1.414...) with f = 0
    assert!(
        report.fun < 1e-2,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to known optimum (1, √2)
    assert!(
        report.x[0] >= -500.0 && report.x[0] <= 500.0,
        "x1 coordinate out of bounds: {}",
        report.x[0]
    );
    assert!(
        report.x[1] >= -500.0 && report.x[1] <= 500.0,
        "x2 coordinate out of bounds: {}",
        report.x[1]
    );

    // Check if it found the positive or negative optima
    let expected_x1 = vec![1.0, -1.0];
    let expected_x2 = vec![1.41421356, -1.41421356]; // √2

    let found_x1 = expected_x1
        .iter()
        .any(|&exp| (report.x[0] - exp).abs() < 0.1);
    let found_x2 = expected_x2
        .iter()
        .any(|&exp| (report.x[1] - exp).abs() < 0.1);

    assert!(found_x1, "x1 not near expected values ±1: {}", report.x[0]);
    assert!(found_x2, "x2 not near expected values ±√2: {}", report.x[1]);
}

#[test]
fn test_de_qing_5d() {
    // Test Qing function in 5D - should be tractable being separable
    let bounds = vec![(-500.0, 500.0); 5];
    let config = DEConfigBuilder::new()
        .seed(221)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("qing_5d", qing, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // For 5D, should still converge well being separable
    assert!(
        report.fun < 0.1,
        "Solution quality too low for 5D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= -500.0 && xi <= 500.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}

#[test]
fn test_de_qing_10d() {
    // Test Qing function in 10D - separable should scale well
    let bounds = vec![(-500.0, 500.0); 10];
    let config = DEConfigBuilder::new()
        .seed(222)
        .maxiter(2500)
        .popsize(120)
        .strategy(Strategy::Best2Bin)
        .recombination(0.8)
        .build();

    let result = run_recorded_differential_evolution("qing_10d", qing, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Should still converge for separable function
    assert!(
        report.fun < 1.0,
        "Solution quality too low for 10D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= -500.0 && xi <= 500.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}
