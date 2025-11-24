use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::langermann;

#[test]
fn test_de_langermann_2d() {
    // Test Langermann function in 2D - complex multimodal with parameters
    let bounds = vec![(0.0, 10.0), (0.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(210)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("langermann_2d", langermann, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Global minimum is approximately -5.1621
    assert!(
        report.fun < -3.0,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= 0.0 && xi <= 10.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}

#[test]
fn test_de_langermann_different_strategies() {
    // Test multiple strategies since this is a complex multimodal function
    let bounds = vec![(0.0, 10.0), (0.0, 10.0)];

    let strategies = vec![
        Strategy::RandToBest1Bin,
        Strategy::Best2Bin,
        Strategy::Rand1Exp,
    ];

    for (i, strategy) in strategies.iter().enumerate() {
        let config = DEConfigBuilder::new()
            .seed(211 + i as u64)
            .maxiter(1500)
            .popsize(80)
            .strategy(*strategy)
            .recombination(0.8)
            .build();

        let result = run_recorded_differential_evolution(
            "langermann_different_strategies",
            langermann,
            &bounds,
            config,
        );
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(
            report.fun < -1.0,
            "Strategy {:?} failed to find reasonable solution: {}",
            strategy,
            report.fun
        );
    }
}
