use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::xin_she_yang_n4;

#[test]
fn test_de_xin_she_yang_n4_2d() {
    // Test Xin-She Yang N.4 function in 2D - challenging multimodal
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(200)
        .maxiter(2500)
        .popsize(120)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result =
        run_recorded_differential_evolution("xin_she_yang_n4_2d", xin_she_yang_n4, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Global minimum is at (0, 0) with f = -1
    assert!(
        report.fun > -1.01,
        "Solution too good (below theoretical minimum): {}",
        report.fun
    );
    assert!(
        report.fun < -0.3,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to known optimum (0, 0)
    for &xi in report.x.iter() {
        assert!(
            xi >= -10.0 && xi <= 10.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
        assert!(
            xi.abs() < 3.0,
            "Solution not reasonably near global optimum (0, 0): {}",
            xi
        );
    }
}

#[test]
fn test_de_xin_she_yang_n4_5d() {
    // Test Xin-She Yang N.4 function in 5D - very challenging
    let bounds = vec![(-10.0, 10.0); 5];
    let config = DEConfigBuilder::new()
        .seed(201)
        .maxiter(3000)
        .popsize(150)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.8)
        .build();

    let result =
        run_recorded_differential_evolution("xin_she_yang_n4_5d", xin_she_yang_n4, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // For 5D, accept much higher tolerance due to complexity
    assert!(
        report.fun > -1.01,
        "Solution too good (below theoretical minimum): {}",
        report.fun
    );
    assert!(
        report.fun < 0.5,
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
