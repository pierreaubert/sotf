use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::quadratic;

#[test]
fn test_de_quadratic_2d() {
    // Test 2D quadratic function using direct DE interface
    let b2 = vec![(-5.0, 5.0), (-5.0, 5.0)];
    let c2 = DEConfigBuilder::new()
        .seed(10)
        .maxiter(300)
        .popsize(20)
        .strategy(Strategy::Rand1Bin)
        .recombination(0.8)
        .build();
    {
        let result = run_recorded_differential_evolution("quadratic_2d", quadratic, &b2, c2);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-8)
    };
}

#[test]
fn test_de_quadratic_5d() {
    // Test 5D quadratic function
    let b5 = vec![(-5.0, 5.0); 5];
    let c5 = DEConfigBuilder::new()
        .seed(11)
        .maxiter(500)
        .popsize(40)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    {
        let result = run_recorded_differential_evolution("quadratic_5d", quadratic, &b5, c5);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-7)
    };
}
