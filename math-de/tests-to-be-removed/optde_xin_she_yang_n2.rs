use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::xin_she_yang_n2;

#[test]
fn test_de_xin_she_yang_n2_2d() {
    // Test Xin-She Yang N.2 function in 2D - newer benchmark function
    let bounds = vec![(-6.28, 6.28), (-6.28, 6.28)]; // -2π to 2π
    let config = DEConfigBuilder::new()
        .seed(180)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();

    let result =
        run_recorded_differential_evolution("xin_she_yang_n2_2d", xin_she_yang_n2, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Global minimum is at (0, 0) with f = 0
    assert!(
        report.fun < 1e-3,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to known optimum (0, 0)
    for &xi in report.x.iter() {
        assert!(
            xi >= -6.28 && xi <= 6.28,
            "Solution coordinate out of bounds: {}",
            xi
        );
        assert!(
            xi.abs() < 0.5,
            "Solution not near global optimum (0, 0): {}",
            xi
        );
    }
}

#[test]
fn test_de_xin_she_yang_n2_5d() {
    // Test Xin-She Yang N.2 function in 5D - higher dimensional challenge
    let bounds = vec![(-6.28, 6.28); 5];
    let config = DEConfigBuilder::new()
        .seed(181)
        .maxiter(2500)
        .popsize(120)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result =
        run_recorded_differential_evolution("xin_she_yang_n2_5d", xin_she_yang_n2, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // For 5D, accept a slightly higher tolerance
    assert!(
        report.fun < 1e-1,
        "Solution quality too low for 5D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= -6.28 && xi <= 6.28,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}

#[test]
fn test_xin_she_yang_n2_multimodal_behavior() {
    // Test that function has multiple local optima
    use ndarray::Array1;

    let test_points = vec![
        vec![0.0, 0.0],   // Global optimum
        vec![6.28, 0.0],  // Potential local behavior
        vec![0.0, 6.28],  // Due to periodic nature
        vec![-6.28, 0.0], // And symmetry
    ];

    for point in test_points {
        let x = Array1::from(point.clone());
        let f = xin_she_yang_n2(&x);

        assert!(
            f.is_finite(),
            "Function should be finite at {:?}: {}",
            point,
            f
        );
        assert!(
            f >= 0.0,
            "Function should be non-negative at {:?}: {}",
            point,
            f
        );
    }
}
