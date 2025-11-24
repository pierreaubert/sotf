use autoeq_de::{DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{
    keanes_bump_constraint1, keanes_bump_constraint2, keanes_bump_objective,
};

#[test]
fn test_de_constrained_keanes_bump() {
    // Test 2D Keane's bump function
    let b = vec![(0.1, 9.9), (0.1, 9.9)];
    let c = DEConfigBuilder::new()
        .seed(58)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.3, max: 1.0 })
        .add_penalty_ineq(Box::new(keanes_bump_constraint1), 1e6)
        .add_penalty_ineq(Box::new(keanes_bump_constraint2), 1e6)
        .build();
    let result = run_recorded_differential_evolution(
        "constrained_keanes_bump",
        keanes_bump_objective,
        &b,
        c,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Check constraints: product > 0.75 and sum < 15.0 (for 2D)
    let product = report.x.iter().product::<f64>();
    let sum = report.x.iter().sum::<f64>();
    assert!(product > 0.749); // Should satisfy product constraint
    assert!(sum < 15.1); // Should satisfy sum constraint
    assert!(report.fun < -0.1); // Should find feasible solution with negative objective
}
