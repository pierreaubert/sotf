use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{binh_korn_constraint1, binh_korn_constraint2, binh_korn_weighted};

#[test]
fn test_de_constrained_binh_korn() {
    // Test Binh-Korn constrained multi-objective problem as single objective
    let b = vec![(0.0, 5.0), (0.0, 3.0)];
    let c = DEConfigBuilder::new()
        .seed(59)
        .maxiter(1200)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .add_penalty_ineq(Box::new(binh_korn_constraint1), 1e6)
        .add_penalty_ineq(Box::new(binh_korn_constraint2), 1e6)
        .build();
    let result =
        run_recorded_differential_evolution("constrained_binh_korn", binh_korn_weighted, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Check constraints
    let g1 = (report.x[0] - 5.0).powi(2) + report.x[1].powi(2);
    let g2 = (report.x[0] - 8.0).powi(2) + (report.x[1] + 3.0).powi(2);
    assert!(g1 <= 25.1); // g1 <= 25
    assert!(g2 >= 7.6); // g2 >= 7.7
    assert!(report.fun < 50.0); // Should find reasonable objective value
}
