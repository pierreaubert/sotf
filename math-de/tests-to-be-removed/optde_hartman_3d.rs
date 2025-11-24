use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::hartman_3d;

#[test]
fn test_de_hartman_3d() {
    // Test Hartman 3D function
    let bounds = vec![(0.0, 1.0), (0.0, 1.0), (0.0, 1.0)];
    let config = DEConfigBuilder::new()
        .seed(60)
        .maxiter(1500)
        .popsize(60)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("hartman_3d", hartman_3d, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < -3.8,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to global minimum (0.114614, 0.555649, 0.852547)
    let expected = vec![0.114614, 0.555649, 0.852547];
    for (i, &xi) in report.x.iter().enumerate() {
        assert!(
            (xi - expected[i]).abs() < 0.1,
            "Solution coordinate {} not near expected {}: {}",
            i,
            expected[i],
            xi
        );
    }
}

#[test]
fn test_de_hartman_3d_multiple_strategies() {
    // Test with different DE strategies to ensure robustness
    let bounds = vec![(0.0, 1.0), (0.0, 1.0), (0.0, 1.0)];

    let strategies = vec![
        Strategy::Rand1Bin,
        Strategy::Best1Exp,
        Strategy::RandToBest1Bin,
    ];

    for (i, strategy) in strategies.iter().enumerate() {
        let config = DEConfigBuilder::new()
            .seed(60 + i as u64)
            .maxiter(1200)
            .popsize(50)
            .strategy(*strategy)
            .recombination(0.9)
            .build();

        let result = run_recorded_differential_evolution(
            "hartman_3d_multiple_strategies",
            hartman_3d,
            &bounds,
            config,
        );
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(
            report.fun < -3.5,
            "Strategy {:?} failed with value: {}",
            strategy,
            report.fun
        );
    }
}

#[test]
fn test_hartman_3d_known_minimum() {
    // Test that the known global minimum actually gives the expected value
    use ndarray::Array1;
    let x_star = Array1::from(vec![0.114614, 0.555649, 0.852547]);
    let f_star = hartman_3d(&x_star);

    // Should be approximately -3.86278
    assert!(
        (f_star - (-3.86278)).abs() < 0.01,
        "Known minimum doesn't match expected value: {}",
        f_star
    );
}
