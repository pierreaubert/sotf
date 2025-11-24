use autoeq_de::{DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::eggholder;

#[test]
fn test_de_eggholder() {
    let b = vec![(-512.0, 512.0), (-512.0, 512.0)];
    let c = DEConfigBuilder::new()
        .seed(27)
        .maxiter(1200)
        .popsize(40)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    {
        let result = run_recorded_differential_evolution("eggholder", eggholder, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < -700.0)
    };
}
