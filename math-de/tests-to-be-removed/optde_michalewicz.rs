use autoeq_de::{DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::michalewicz;

#[test]
fn test_de_michalewicz_2d() {
    // Test 2D Michalewicz
    let b2 = vec![(0.0, std::f64::consts::PI), (0.0, std::f64::consts::PI)];
    let c2 = DEConfigBuilder::new()
        .seed(74)
        .maxiter(1200)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("michalewicz_2d", michalewicz, &b2, c2);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // 2D Michalewicz has global minimum f(x)=-1.8013 at x*=(2.20,1.57)
    assert!(report.fun < -1.5); // Should find good solution
}

#[test]
fn test_de_michalewicz_5d() {
    // Test 5D Michalewicz
    let b5 = vec![(0.0, std::f64::consts::PI); 5];
    let c5 = DEConfigBuilder::new()
        .seed(75)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.3, max: 1.0 })
        .build();
    let result = run_recorded_differential_evolution("michalewicz_5d", michalewicz, &b5, c5);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Higher dimensional Michalewicz becomes increasingly difficult
    assert!(report.fun < -2.0); // Should find reasonable solution
}
