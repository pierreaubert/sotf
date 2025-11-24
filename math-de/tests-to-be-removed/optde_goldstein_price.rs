use autoeq_de::{DEConfig, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::goldstein_price;

#[test]
fn test_de_goldstein_price() {
    let b = vec![(-2.0, 2.0), (-2.0, 2.0)];
    let mut c = DEConfig::default();
    c.seed = Some(7);
    c.maxiter = 600;
    c.popsize = 30;
    c.strategy = Strategy::Rand1Exp;
    {
        let result = run_recorded_differential_evolution("goldstein_price", goldstein_price, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 3.01)
    };
}
