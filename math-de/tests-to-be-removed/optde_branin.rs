use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::branin;

#[test]
fn test_de_branin() {
    let b = vec![(-5.0, 10.0), (0.0, 15.0)];
    let c = DEConfigBuilder::new()
        .seed(30)
        .maxiter(600)
        .popsize(30)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("branin", branin, &b, c);

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 0.4);
}
