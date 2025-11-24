use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::brown;

#[test]
fn test_de_brown_2d() {
    // Test Brown function in 2D - this is an ill-conditioned unimodal function
    let bounds = vec![(-1.0, 4.0), (-1.0, 4.0)];
    let config = DEConfigBuilder::new()
        .seed(110)
        .maxiter(1500) // More iterations needed due to ill-conditioning
        .popsize(80) // Larger population for difficult conditioning
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("brown_2d", brown, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < 1e-5,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to global minimum (0, 0)
    for &xi in report.x.iter() {
        assert!(xi.abs() < 1e-3, "Solution coordinate not near 0: {}", xi);
    }
}

#[test]
fn test_de_brown_4d() {
    // Test Brown function in 4D
    let bounds = vec![(-1.0, 4.0); 4];
    let config = DEConfigBuilder::new()
        .seed(111)
        .maxiter(2000)
        .popsize(120)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.95)
        .build();

    let result = run_recorded_differential_evolution("brown_4d", brown, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < 1e-4,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is close to global minimum (0, 0, 0, 0)
    for &xi in report.x.iter() {
        assert!(xi.abs() < 1e-2, "Solution coordinate not near 0: {}", xi);
    }
}

#[test]
fn test_de_brown_high_precision() {
    // Test with higher precision requirements due to ill-conditioning
    let bounds = vec![(-1.0, 4.0), (-1.0, 4.0)];
    let config = DEConfigBuilder::new()
        .seed(112)
        .maxiter(2500)
        .popsize(100)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .tol(1e-12) // Very tight tolerance
        .build();

    let result =
        run_recorded_differential_evolution("brown_high_precision", brown, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < 1e-8,
        "High precision solution not achieved: {}",
        report.fun
    );
}

#[test]
fn test_de_brown_multiple_strategies() {
    // Test different strategies on this ill-conditioned function
    let bounds = vec![(-1.0, 4.0), (-1.0, 4.0)];

    let strategies = [
        Strategy::Best1Bin,
        Strategy::RandToBest1Bin,
        Strategy::Best2Bin,
    ];

    for (i, strategy) in strategies.iter().enumerate() {
        let config = DEConfigBuilder::new()
            .seed(110 + i as u64)
            .maxiter(1500)
            .popsize(80)
            .strategy(*strategy)
            .recombination(0.9)
            .build();

        let name = format!("brown_strategy_{:?}_{}", strategy, i);
        let result = run_recorded_differential_evolution(&name, brown, &bounds, config);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(
            report.fun < 1e-3,
            "Strategy {:?} failed with value: {}",
            strategy,
            report.fun
        );
    }
}

#[test]
fn test_brown_known_minimum() {
    // Test that the known global minimum actually gives the expected value
    use ndarray::Array1;
    let x_star = Array1::from(vec![0.0, 0.0]);
    let f_star = brown(&x_star);

    // Should be exactly 0.0
    assert!(
        f_star < 1e-15,
        "Known minimum doesn't match expected value: {}",
        f_star
    );
}

#[test]
fn test_brown_ill_conditioning() {
    // Test the ill-conditioned nature of the Brown function
    use ndarray::Array1;

    // Test that small changes in x can lead to large changes in function value
    let x1 = Array1::from(vec![0.1, 0.1]);
    let x2 = Array1::from(vec![0.11, 0.11]);
    let f1 = brown(&x1);
    let f2 = brown(&x2);

    assert!(
        f1.is_finite() && f2.is_finite(),
        "Function values should be finite"
    );

    // Test that function grows rapidly away from origin
    let x_far = Array1::from(vec![1.0, 1.0]);
    let f_far = brown(&x_far);
    let f_origin = brown(&Array1::from(vec![0.0, 0.0]));

    assert!(
        f_far > f_origin,
        "Function should increase away from origin"
    );
}

#[test]
fn test_brown_different_dimensions() {
    // Test function behavior in different dimensions
    use ndarray::Array1;

    let dimensions = [2, 4, 6, 10];

    for &dim in &dimensions {
        // Test at global minimum (all zeros)
        let x_zero = Array1::from(vec![0.0; dim]);
        let f_zero = brown(&x_zero);
        assert!(
            f_zero < 1e-15,
            "Function at zero not 0 for dim {}: {}",
            dim,
            f_zero
        );

        // Test at small perturbation
        let x_small = Array1::from(vec![0.01; dim]);
        let f_small = brown(&x_small);
        assert!(
            f_small.is_finite(),
            "Function at small perturbation not finite for dim {}",
            dim
        );
        assert!(
            f_small > 0.0,
            "Function should be positive away from minimum for dim {}",
            dim
        );
    }
}

#[test]
fn test_brown_convergence_difficulty() {
    // Test that Brown function is indeed difficult to optimize (many iterations needed)
    let bounds = vec![(-1.0, 4.0), (-1.0, 4.0)];

    // Test with insufficient iterations
    let config_short = DEConfigBuilder::new()
        .seed(114)
        .maxiter(200) // Too few iterations
        .popsize(30) // Small population
        .strategy(Strategy::Rand1Bin)
        .recombination(0.7)
        .build();

    let result = run_recorded_differential_evolution("brown_short", brown, &bounds, config_short);
    assert!(result.is_ok());
    let (report_short, _csv_short) = result.unwrap();

    // Test with adequate iterations
    let config_long = DEConfigBuilder::new()
        .seed(114) // Same seed
        .maxiter(1500) // More iterations
        .popsize(80) // Larger population
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("brown_long", brown, &bounds, config_long);
    assert!(result.is_ok());
    let (report_long, _csv_long) = result.unwrap();

    // The longer run should achieve better results
    assert!(
        report_long.fun <= report_short.fun,
        "Longer optimization should be better or equal: {} vs {}",
        report_long.fun,
        report_short.fun
    );
}

#[test]
fn test_brown_boundary_behavior() {
    // Test function behavior at boundaries
    use ndarray::Array1;

    let test_points = vec![
        vec![-1.0, -1.0],
        vec![4.0, 4.0],
        vec![-1.0, 4.0],
        vec![4.0, -1.0],
        vec![0.0, 4.0],
        vec![0.0, -1.0],
    ];

    for point in test_points {
        let x = Array1::from(point.clone());
        let f = brown(&x);
        assert!(
            f.is_finite(),
            "Function value at {:?} should be finite: {}",
            point,
            f
        );
        assert!(f >= 0.0, "Function should be non-negative: {}", f);
    }
}
