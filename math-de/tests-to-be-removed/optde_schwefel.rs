use autoeq_de::{DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::schwefel;

#[test]
fn test_de_schwefel_2d() {
    // Test 2D Schwefel
    let b2 = vec![(-500.0, 500.0), (-500.0, 500.0)];
    let c2 = DEConfigBuilder::new()
        .seed(46)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    {
        let result = run_recorded_differential_evolution("schwefel_2d", schwefel, &b2, c2);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-2)
    };
}

#[test]
fn test_de_schwefel_5d() {
    // Test 5D Schwefel
    let b5 = vec![(-500.0, 500.0); 5];
    let c5 = DEConfigBuilder::new()
        .seed(47)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .build();
    {
        let result = run_recorded_differential_evolution("schwefel_5d", schwefel, &b5, c5);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-1)
    };
}
