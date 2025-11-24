use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::alpine_n1;

#[test]
fn test_de_alpine_n1_2d() {
    // Test Alpine N.1 in 2D
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(42)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("alpine_n1_2d", alpine_n1, &bounds, config);

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < 1e-2,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to global minimum (0, 0)
    for &xi in report.x.iter() {
        assert!(xi.abs() < 0.2, "Solution coordinate too far from 0: {}", xi);
    }
}

#[test]
fn test_de_alpine_n1_5d() {
    // Test Alpine N.1 in 5D
    let bounds = vec![(-10.0, 10.0); 5];
    let config = DEConfigBuilder::new()
        .seed(43)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("alpine_n1_5d", alpine_n1, &bounds, config);

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < 1e-2,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to global minimum (0, 0, 0, 0, 0)
    for &xi in report.x.iter() {
        assert!(xi.abs() < 0.1, "Solution coordinate too far from 0: {}", xi);
    }
}
