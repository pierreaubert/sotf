use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::exponential;

#[test]
fn test_de_exponential_2d() {
    // Test exponential function in 2D - unimodal function
    let bounds = vec![(-1.0, 1.0), (-1.0, 1.0)];
    let config = DEConfigBuilder::new()
        .seed(170)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.7)
        .build();

    let result =
        run_recorded_differential_evolution("exponential_2d", exponential, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Global minimum is at (0, 0) with f = -1
    assert!(
        report.fun > -1.001,
        "Solution too good (below theoretical minimum): {}",
        report.fun
    );
    assert!(
        report.fun < -0.9,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to known optimum (0, 0)
    for &xi in report.x.iter() {
        assert!(
            xi >= -1.0 && xi <= 1.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
        assert!(
            xi.abs() < 1e-2,
            "Solution not near global optimum (0, 0): {}",
            xi
        );
    }
}

#[test]
fn test_de_exponential_5d() {
    // Test exponential function in 5D - should still be easy to optimize being unimodal
    let bounds = vec![(-1.0, 1.0); 5];
    let config = DEConfigBuilder::new()
        .seed(171)
        .maxiter(1200)
        .popsize(70)
        .strategy(Strategy::Rand1Bin)
        .recombination(0.8)
        .build();

    let result =
        run_recorded_differential_evolution("exponential_5d", exponential, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Should still achieve good precision for unimodal function
    assert!(
        report.fun > -1.001,
        "Solution too good (below theoretical minimum): {}",
        report.fun
    );
    assert!(
        report.fun < -0.8,
        "Solution quality too low for 5D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= -1.0 && xi <= 1.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
        assert!(xi.abs() < 0.1, "Solution not near global optimum: {}", xi);
    }
}

#[test]
fn test_de_exponential_10d() {
    // Test exponential function in 10D - higher dimensional unimodal
    let bounds = vec![(-1.0, 1.0); 10];
    let config = DEConfigBuilder::new()
        .seed(172)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::Best2Bin)
        .recombination(0.9)
        .build();

    let result =
        run_recorded_differential_evolution("exponential_10d", exponential, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Still should converge well for unimodal function
    assert!(
        report.fun > -1.001,
        "Solution too good (below theoretical minimum): {}",
        report.fun
    );
    assert!(
        report.fun < -0.5,
        "Solution quality too low for 10D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= -1.0 && xi <= 1.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}
