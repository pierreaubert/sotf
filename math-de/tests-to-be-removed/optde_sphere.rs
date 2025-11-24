use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::sphere;

#[test]
fn test_de_sphere_2d() {
    // Test 2D Sphere function using direct DE interface
    let b2 = vec![(-5.0, 5.0), (-5.0, 5.0)];
    let c2 = DEConfigBuilder::new()
        .seed(30)
        .maxiter(500)
        .popsize(30)
        .strategy(Strategy::Rand1Bin)
        .recombination(0.8)
        .build();
    {
        let result = run_recorded_differential_evolution("sphere_2d", sphere, &b2, c2);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-6)
    };
}

#[test]
fn test_de_sphere_5d() {
    // Test 5D Sphere function
    let b5 = vec![(-5.0, 5.0); 5];
    let c5 = DEConfigBuilder::new()
        .seed(31)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    {
        let result = run_recorded_differential_evolution("sphere_5d", sphere, &b5, c5);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-5)
    };
}
