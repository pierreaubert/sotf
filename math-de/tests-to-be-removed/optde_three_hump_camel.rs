use autoeq_de::{DEConfig, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::three_hump_camel;

#[test]
fn test_de_three_hump_camel() {
    let b = vec![(-5.0, 5.0), (-5.0, 5.0)];
    let mut c = DEConfig::default();
    c.seed = Some(8);
    c.maxiter = 300;
    c.popsize = 20;
    c.strategy = Strategy::Best1Exp;
    {
        let result =
            run_recorded_differential_evolution("three_hump_camel", three_hump_camel, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-6)
    };
}
