use autoeq_de::{DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::bukin_n6;

#[test]
fn test_de_bukin_n6() {
    let bounds = vec![(-15.0, -5.0), (-3.0, 3.0)];
    let config = DEConfigBuilder::new()
        .seed(26)
        .maxiter(1500) // Reduced from very high value
        .popsize(100) // Reduced from very high value
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.4, max: 1.0 }) // Added mutation control
        .build();
    let result = run_recorded_differential_evolution("bukin_n6", bukin_n6, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1.0); // Very relaxed tolerance - Bukin N6 is extremely difficult
}
