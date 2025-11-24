use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::chung_reynolds;

#[test]
fn test_de_chung_reynolds_2d() {
    // Test Chung Reynolds function in 2D - unimodal quadratic function
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(160)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::Best1Bin)
        .recombination(0.7)
        .build();

    let result =
        run_recorded_differential_evolution("chung_reynolds_2d", chung_reynolds, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Global minimum is at (0, 0) with f = 0
    assert!(
        report.fun < 1e-6,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is very close to known optimum (0, 0)
    for &xi in report.x.iter() {
        assert!(
            xi >= -100.0 && xi <= 100.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
        assert!(
            xi.abs() < 1e-3,
            "Solution not near global optimum (0, 0): {}",
            xi
        );
    }
}

#[test]
fn test_de_chung_reynolds_5d() {
    // Test Chung Reynolds function in 5D - should still be easy to optimize being unimodal
    let bounds = vec![(-100.0, 100.0); 5];
    let config = DEConfigBuilder::new()
        .seed(161)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Rand1Bin)
        .recombination(0.8)
        .build();

    let result =
        run_recorded_differential_evolution("chung_reynolds_5d", chung_reynolds, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Should still achieve high precision for unimodal function
    assert!(
        report.fun < 1e-4,
        "Solution quality too low for 5D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= -100.0 && xi <= 100.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
        assert!(xi.abs() < 0.1, "Solution not near global optimum: {}", xi);
    }
}

#[test]
fn test_de_chung_reynolds_10d() {
    // Test Chung Reynolds function in 10D - higher dimensional unimodal
    let bounds = vec![(-100.0, 100.0); 10];
    let config = DEConfigBuilder::new()
        .seed(162)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Best2Bin)
        .recombination(0.9)
        .build();

    let result =
        run_recorded_differential_evolution("chung_reynolds_10d", chung_reynolds, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Still should converge well for unimodal function
    assert!(
        report.fun < 1e-2,
        "Solution quality too low for 10D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= -100.0 && xi <= 100.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}
