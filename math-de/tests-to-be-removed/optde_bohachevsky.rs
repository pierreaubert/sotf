use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{bohachevsky1, bohachevsky2, bohachevsky3};

#[test]
fn test_de_bohachevsky1() {
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("bohachevsky1", bohachevsky1, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-4);
}

#[test]
fn test_de_bohachevsky2() {
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .build();
    let result = run_recorded_differential_evolution("bohachevsky2", bohachevsky2, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-4);
}

#[test]
fn test_de_bohachevsky3() {
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .build();
    let result = run_recorded_differential_evolution("bohachevsky3", bohachevsky3, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-4);
}
