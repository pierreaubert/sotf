use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::ackley;

#[test]
fn test_de_ackley_10d() {
    // Test 10D Ackley
    let b10 = vec![(-32.768, 32.768); 10];
    let c10 = DEConfigBuilder::new()
        .seed(43)
        .maxiter(1200)
        .popsize(100)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution("ackley_10d", ackley, &b10, c10);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-2);
}

#[test]
fn test_de_ackley_2d() {
    let bounds = vec![(-32.768, 32.768), (-32.768, 32.768)];
    let config = DEConfigBuilder::new()
        .seed(42)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("ackley", ackley, &bounds, config);

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-3);

    // Check that solution is close to global optimum (0, 0)
    for &actual in report.x.iter() {
        assert!(actual.abs() < 0.5);
    }
}
