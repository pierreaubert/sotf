use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{schaffer_n2, schaffer_n4};

#[test]
fn test_de_schaffer_n2() {
    let b = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(25)
        .maxiter(300)
        .popsize(25)
        .strategy(Strategy::Best1Exp)
        .build();
    {
        let result = run_recorded_differential_evolution("schaffer_n2", schaffer_n2, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-3)
    };
}

#[test]
fn test_de_schaffer_n4() {
    let b = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(32)
        .maxiter(800)
        .popsize(35)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    {
        let result = run_recorded_differential_evolution("schaffer_n4", schaffer_n4, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 0.35)
    };
}
