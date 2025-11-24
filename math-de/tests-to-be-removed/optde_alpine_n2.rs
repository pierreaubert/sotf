use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::alpine_n2;

#[test]
fn test_de_alpine_n2_2d() {
    // Test Alpine N.2 in 2D
    let bounds = vec![(0.0, 10.0), (0.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(50)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("alpine_n2_2d", alpine_n2, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < -7.0,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to global minimum (2.808, 2.808)
    for &xi in report.x.iter() {
        assert!(
            (xi - 2.808).abs() < 0.5,
            "Solution coordinate not near 2.808: {}",
            xi
        );
    }
}

#[test]
fn test_de_alpine_n2_3d() {
    // Test Alpine N.2 in 3D
    let bounds = vec![(0.0, 10.0); 3];
    let config = DEConfigBuilder::new()
        .seed(51)
        .maxiter(1500)
        .popsize(75)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.95)
        .build();

    let result = run_recorded_differential_evolution("alpine_n2_3d", alpine_n2, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // For 3D: expected minimum is approximately -2.808^3 ≈ -22.2
    assert!(
        report.fun < -20.0,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to global minimum (2.808, 2.808, 2.808)
    for &xi in report.x.iter() {
        assert!(
            (xi - 2.808).abs() < 0.5,
            "Solution coordinate not near 2.808: {}",
            xi
        );
    }
}
