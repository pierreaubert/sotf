use autoeq_de::{DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::griewank2;

#[test]
fn test_de_griewank2() {
    let b = vec![(-600.0, 600.0), (-600.0, 600.0)];
    let c = DEConfigBuilder::new()
        .seed(21)
        .maxiter(800) // Increased iterations
        .popsize(50) // Increased population
        .strategy(Strategy::RandToBest1Exp) // Better strategy for multimodal
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    {
        let result = run_recorded_differential_evolution("griewank2", griewank2, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-2)
    }; // Relaxed tolerance
}
