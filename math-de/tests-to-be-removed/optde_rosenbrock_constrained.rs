use autoeq_de::{
    DEConfigBuilder, NonlinearConstraintHelper, Strategy, run_recorded_differential_evolution,
};
use autoeq_testfunctions::{rosenbrock_disk_constraint, rosenbrock_objective};
use ndarray::Array1;
use std::sync::Arc;

#[test]
fn test_de_constrained_rosenbrock_disk() {
    let b = vec![(-1.5, 1.5), (-1.5, 1.5)];
    let c = DEConfigBuilder::new()
        .seed(56)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .add_penalty_ineq(Box::new(rosenbrock_disk_constraint), 1e6)
        .build();
    let result = run_recorded_differential_evolution(
        "constrained_rosenbrock_disk",
        rosenbrock_objective,
        &b,
        c,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Check that solution respects constraint x^2 + y^2 <= 2
    let constraint_value = report.x[0].powi(2) + report.x[1].powi(2);
    assert!(constraint_value <= 2.01); // Small tolerance for numerical errors
    assert!(report.fun < 0.5); // Should find good solution within constraint
}

#[test]
fn test_de_nonlinear_constraint_helper() {
    // Test using NonlinearConstraintHelper for a more complex constraint
    let objective = |x: &Array1<f64>| (x[0] - 1.0).powi(2) + (x[1] - 2.0).powi(2);

    // Constraint: x[0]^2 + x[1]^2 <= 4 (circle constraint)
    let constraint_fn = Arc::new(|x: &Array1<f64>| Array1::from(vec![x[0].powi(2) + x[1].powi(2)]));

    let constraint = NonlinearConstraintHelper {
        fun: constraint_fn,
        lb: Array1::from(vec![-f64::INFINITY]), // no lower bound
        ub: Array1::from(vec![4.0]),            // upper bound: <= 4
    };

    let b = vec![(-3.0, 3.0), (-3.0, 3.0)];
    let mut c = DEConfigBuilder::new()
        .seed(60)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();

    // Apply nonlinear constraint
    constraint.apply_to(&mut c, 1e6, 1e6);

    let result =
        run_recorded_differential_evolution("nonlinear_constraint_helper", objective, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Check that solution respects constraint x^2 + y^2 <= 4
    let constraint_value = report.x[0].powi(2) + report.x[1].powi(2);
    assert!(constraint_value <= 4.01); // Should be inside circle

    // The unconstrained optimum is at (1, 2), but constraint forces it to circle boundary
    assert!(report.fun < 2.0); // Should find good feasible solution
}
