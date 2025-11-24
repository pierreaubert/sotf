use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::griewank;

#[test]
fn test_de_griewank_2d() {
    // Test 2D Griewank
    let b2 = vec![(-600.0, 600.0), (-600.0, 600.0)];
    let c2 = DEConfigBuilder::new()
        .seed(44)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    {
        let result = run_recorded_differential_evolution("griewank_2d", griewank, &b2, c2);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-2)
    };
}

#[test]
fn test_de_griewank_10d() {
    // Test 10D Griewank
    let b10 = vec![(-600.0, 600.0); 10];
    let c10 = DEConfigBuilder::new()
        .seed(45)
        .maxiter(1000)
        .popsize(80)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    {
        let result = run_recorded_differential_evolution("griewank_10d", griewank, &b10, c10);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-1)
    }; // Relaxed for 10D
}
