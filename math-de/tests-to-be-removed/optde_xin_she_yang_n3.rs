use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::xin_she_yang_n3;

#[test]
fn test_de_xin_she_yang_n3_2d() {
    // Test Xin-She Yang N.3 function in 2D - multimodal with parameter m
    let bounds = vec![(-20.0, 20.0), (-20.0, 20.0)];
    let config = DEConfigBuilder::new()
        .seed(190)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result =
        run_recorded_differential_evolution("xin_she_yang_n3_2d", xin_she_yang_n3, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Global minimum is at (0, 0) with f = -1
    assert!(
        report.fun > -1.01,
        "Solution too good (below theoretical minimum): {}",
        report.fun
    );
    assert!(
        report.fun < -0.5,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to known optimum (0, 0)
    for &xi in report.x.iter() {
        assert!(
            xi >= -20.0 && xi <= 20.0,
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
fn test_de_xin_she_yang_n3_5d() {
    // Test Xin-She Yang N.3 function in 5D
    let bounds = vec![(-20.0, 20.0); 5];
    let config = DEConfigBuilder::new()
        .seed(191)
        .maxiter(3000)
        .popsize(150)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();

    let result =
        run_recorded_differential_evolution("xin_she_yang_n3_5d", xin_she_yang_n3, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // For 5D, accept a higher tolerance
    assert!(
        report.fun > -1.01,
        "Solution too good (below theoretical minimum): {}",
        report.fun
    );
    assert!(
        report.fun < -0.1,
        "Solution quality too low for 5D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= -20.0 && xi <= 20.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}
