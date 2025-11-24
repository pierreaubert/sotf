use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::six_hump_camel;

#[test]
fn test_de_six_hump_camel() {
    let bounds = vec![(-3.0, 3.0), (-2.0, 2.0)];
    let config = DEConfigBuilder::new()
        .seed(9)
        .maxiter(500)
        .popsize(30)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();

    let result =
        run_recorded_differential_evolution("six_hump_camel", six_hump_camel, &bounds, config);

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -1.0);

    // Six-hump camel has two global minima at (0.0898, -0.7126) and (-0.0898, 0.7126) with f = -1.0316
    let is_near_min1 = (report.x[0] - 0.0898).abs() < 0.2 && (report.x[1] + 0.7126).abs() < 0.2;
    let is_near_min2 = (report.x[0] + 0.0898).abs() < 0.2 && (report.x[1] - 0.7126).abs() < 0.2;
    assert!(
        is_near_min1 || is_near_min2,
        "Solution should be close to one of the global minima"
    );
}
