use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::powell;

#[test]
fn test_de_powell_4d() {
    // Test 4D Powell
    let b4 = vec![(-4.0, 5.0); 4];
    let c4 = DEConfigBuilder::new()
        .seed(54)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    {
        let result = run_recorded_differential_evolution("powell_4d", powell, &b4, c4);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-3)
    };
}

#[test]
fn test_de_powell_8d() {
    // Test 8D Powell
    let b8 = vec![(-4.0, 5.0); 8];
    let c8 = DEConfigBuilder::new()
        .seed(55)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    {
        let result = run_recorded_differential_evolution("powell_8d", powell, &b8, c8);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-2)
    };
}
