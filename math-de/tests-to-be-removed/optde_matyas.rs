use autoeq_de::{DEConfig, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::matyas;

#[test]
fn test_de_matyas() {
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let mut config = DEConfig::default();
    config.seed = Some(5);
    config.maxiter = 800;
    config.popsize = 40;
    config.recombination = 0.9;
    config.strategy = Strategy::RandToBest1Exp;

    let result = run_recorded_differential_evolution("matyas", matyas, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Matyas function: Global minimum f(x) = 0 at x = (0, 0)
    assert!(report.fun < 1e-5);

    // Check that solution is close to expected optimum
    let expected = vec![0.0, 0.0];
    for (actual, expected) in report.x.iter().zip(expected.iter()) {
        assert!(
            (actual - expected).abs() < 0.1,
            "Solution component {} should be close to {}",
            actual,
            expected
        );
    }
}

#[test]
fn test_de_matyas_different_strategy() {
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let mut config = DEConfig::default();
    config.seed = Some(123);
    config.maxiter = 500;
    config.popsize = 30;
    config.recombination = 0.7;
    config.strategy = Strategy::CurrentToBest1Bin;

    let result =
        run_recorded_differential_evolution("matyas_different_strategy", matyas, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Should still converge to global minimum
    assert!(report.fun < 1e-4);
}
