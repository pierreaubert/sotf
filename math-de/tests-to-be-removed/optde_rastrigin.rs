use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::rastrigin;

#[test]
fn test_de_rastrigin_2d() {
    // Test 2D Rastrigin
    let b2 = vec![(-5.12, 5.12), (-5.12, 5.12)];
    let c2 = DEConfigBuilder::new()
        .seed(40)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    {
        let result = run_recorded_differential_evolution("rastrigin_2d", rastrigin, &b2, c2);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-2)
    };
}

#[test]
fn test_de_rastrigin_5d() {
    // Test 5D Rastrigin
    let b5 = vec![(-5.12, 5.12); 5];
    let c5 = DEConfigBuilder::new()
        .seed(41)
        .maxiter(1500)
        .popsize(75)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    {
        let result = run_recorded_differential_evolution("rastrigin_5d", rastrigin, &b5, c5);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-1)
    };
}
