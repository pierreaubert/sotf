use autoeq_de::{DEConfig, run_recorded_differential_evolution};
use autoeq_testfunctions::mccormick;

#[test]
fn test_de_mccormick() {
    let b = vec![(-1.5, 4.0), (-3.0, 4.0)];
    let mut c = DEConfig::default();
    c.seed = Some(11);
    c.maxiter = 500;
    c.popsize = 30;
    {
        let result = run_recorded_differential_evolution("mccormick", mccormick, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < -1.7)
    };
}
