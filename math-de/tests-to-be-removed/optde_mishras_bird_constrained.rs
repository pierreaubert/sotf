use autoeq_de::{DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{mishras_bird_constraint, mishras_bird_objective};

#[test]
fn test_de_constrained_mishras_bird() {
    let b = vec![(-10.0, 0.0), (-6.5, 0.0)];
    let c = DEConfigBuilder::new()
        .seed(57)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .add_penalty_ineq(Box::new(mishras_bird_constraint), 1e6)
        .build();
    let result = run_recorded_differential_evolution(
        "constrained_mishras_bird",
        mishras_bird_objective,
        &b,
        c,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Check that solution respects constraint (x+5)^2 + (y+5)^2 <= 25
    let constraint_value = (report.x[0] + 5.0).powi(2) + (report.x[1] + 5.0).powi(2);
    assert!(constraint_value <= 25.1); // Should be inside circle
    assert!(report.fun < -50.0); // Should find good solution within constraint
}
