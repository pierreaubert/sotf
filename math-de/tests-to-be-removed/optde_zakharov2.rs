use autoeq_de::{DEConfigBuilder, run_recorded_differential_evolution};
use autoeq_testfunctions::zakharov2;

#[test]
fn test_de_zakharov2() {
    let b = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(22)
        .maxiter(300)
        .popsize(25)
        .build();
    {
        let result = run_recorded_differential_evolution("zakharov2", zakharov2, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-4)
    };
}
