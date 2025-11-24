use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::schwefel2;

#[test]
fn test_de_schwefel2() {
    let b = vec![(-500.0, 500.0), (-500.0, 500.0)];
    let c = DEConfigBuilder::new()
        .seed(23)
        .maxiter(800)
        .popsize(35)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    {
        let result = run_recorded_differential_evolution("schwefel2", schwefel2, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e2)
    };
}
