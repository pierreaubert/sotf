use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::ackley_n3;

#[test]
fn test_de_ackley_n3_different_strategies() {
    // Test multiple strategies to ensure robustness
    let bounds = vec![(-32.0, 32.0), (-32.0, 32.0)];

    let strategies = vec![
        Strategy::RandToBest1Bin,
        Strategy::Best2Bin,
        Strategy::Rand1Exp,
    ];

    for (i, strategy) in strategies.iter().enumerate() {
        let config = DEConfigBuilder::new()
            .seed(151 + i as u64)
            .maxiter(1500)
            .popsize(80)
            .strategy(*strategy)
            .recombination(0.8)
            .build();

        let result = run_recorded_differential_evolution(
            "ackley_n3_different_strategies",
            ackley_n3,
            &bounds,
            config,
        );
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(
            report.fun < -50.0,
            "Strategy {:?} failed: {}",
            strategy,
            report.fun
        );
    }
}

#[test]
fn test_de_ackley_n3_2d() {
    let bounds = vec![(-32.0, 32.0), (-32.0, 32.0)];
    let config = DEConfigBuilder::new()
        .seed(152)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("ackley_n3", ackley_n3, &bounds, config);

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < -100.0,
        "Recorded Ackley N.3 optimization failed: {}",
        report.fun
    );

    // Check that solution is within bounds
    for &actual in report.x.iter() {
        assert!(
            actual >= -32.0 && actual <= 32.0,
            "Solution out of bounds: {}",
            actual
        );
    }
}
