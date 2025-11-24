use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::pinter;

#[test]
fn test_de_pinter_2d() {
    // Test Pinter function in 2D - challenging multimodal function
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(140)
        .maxiter(2000)
        .popsize(80)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("pinter_2d", pinter, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Global minimum is at (0, 0) with f = 0
    assert!(report.fun < 1.0, "Solution quality too low: {}", report.fun);

    // Check solution is reasonably close to known optimum (0, 0)
    for &xi in report.x.iter() {
        assert!(
            xi >= -10.0 && xi <= 10.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
        assert!(
            xi.abs() < 2.0,
            "Solution not reasonably near global optimum (0, 0): {}",
            xi
        );
    }
}

#[test]
fn test_de_pinter_5d() {
    // Test Pinter function in 5D - higher dimensional challenge
    let bounds = vec![(-10.0, 10.0); 5];
    let config = DEConfigBuilder::new()
        .seed(141)
        .maxiter(3000)
        .popsize(120)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();

    let result = run_recorded_differential_evolution("pinter_5d", pinter, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // For 5D, accept a slightly higher tolerance due to increased complexity
    assert!(
        report.fun < 1e-1,
        "Solution quality too low for 5D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= -10.0 && xi <= 10.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}
