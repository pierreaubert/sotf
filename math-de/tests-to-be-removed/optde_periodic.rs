use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::periodic;

#[test]
fn test_de_periodic_2d() {
    // Test Periodic function in 2D
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(100)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("periodic_2d", periodic, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1.0, "Solution quality too low: {}", report.fun);

    // Check solution is close to global minimum (0, 0) with f = 0.9
    for &xi in report.x.iter() {
        assert!(xi.abs() < 0.1, "Solution coordinate not near 0: {}", xi);
    }
}

#[test]
fn test_de_periodic_5d() {
    // Test Periodic function in 5D
    let bounds = vec![(-10.0, 10.0); 5];
    let config = DEConfigBuilder::new()
        .seed(101)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("periodic_5d", periodic, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1.0, "Solution quality too low: {}", report.fun);

    // Check solution is close to global minimum (0, 0, 0, 0, 0)
    for &xi in report.x.iter() {
        assert!(xi.abs() < 0.2, "Solution coordinate not near 0: {}", xi);
    }
}

#[test]
fn test_de_periodic_multimodal_behavior() {
    // Test that the function can find the global minimum from different starting points
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let mut results = Vec::new();

    for seed in 100..105 {
        let config = DEConfigBuilder::new()
            .seed(seed)
            .maxiter(1000)
            .popsize(60)
            .strategy(Strategy::Best1Bin)
            .recombination(0.9)
            .build();

        let result = run_recorded_differential_evolution(
            "periodic_multimodal_behavior",
            periodic,
            &bounds,
            config,
        );
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        results.push(report.fun);
    }

    // All runs should find the global minimum region (around 0.9)
    for (i, &f) in results.iter().enumerate() {
        assert!(f < 1.1, "Run {} failed to find good solution: {}", i, f);
    }
}

#[test]
fn test_periodic_known_minimum() {
    // Test that the known global minimum actually gives the expected value
    use ndarray::Array1;
    let x_star = Array1::from(vec![0.0, 0.0]);
    let f_star = periodic(&x_star);

    // Should be exactly 0.9
    assert!(
        (f_star - 0.9).abs() < 1e-10,
        "Known minimum doesn't match expected value: {}",
        f_star
    );
}

#[test]
fn test_periodic_different_dimensions() {
    // Test function behavior in different dimensions
    use ndarray::Array1;

    let dimensions = vec![2, 3, 5, 10];

    for &dim in &dimensions {
        // Test at global minimum (all zeros)
        let x_zero = Array1::from(vec![0.0; dim]);
        let f_zero = periodic(&x_zero);
        assert!(
            (f_zero - 0.9).abs() < 1e-10,
            "Function at zero not 0.9 for dim {}: {}",
            dim,
            f_zero
        );

        // Test at small perturbation
        let x_small = Array1::from(vec![0.1; dim]);
        let f_small = periodic(&x_small);
        assert!(
            f_small.is_finite(),
            "Function at small perturbation not finite for dim {}",
            dim
        );
        assert!(
            f_small > 0.9,
            "Function should be higher than minimum for dim {}",
            dim
        );
    }
}

#[test]
fn test_periodic_properties() {
    // Test some mathematical properties of the periodic function
    use ndarray::Array1;

    // Test symmetry around origin
    let x1 = Array1::from(vec![1.0, 2.0]);
    let x2 = Array1::from(vec![-1.0, -2.0]);
    let f1 = periodic(&x1);
    let f2 = periodic(&x2);

    // Should be the same due to sin^2 and exp(-x^2) symmetry
    assert!(
        (f1 - f2).abs() < 1e-10,
        "Function should be symmetric: f({:?}) = {}, f({:?}) = {}",
        x1,
        f1,
        x2,
        f2
    );

    // Test that the function increases as we move away from origin
    let x_far = Array1::from(vec![5.0, 5.0]);
    let f_far = periodic(&x_far);
    let f_origin = periodic(&Array1::from(vec![0.0, 0.0]));

    assert!(
        f_far > f_origin,
        "Function should increase away from origin"
    );
}

#[test]
fn test_periodic_boundary_behavior() {
    // Test function behavior at boundaries
    use ndarray::Array1;

    let test_points = vec![
        vec![-10.0, -10.0],
        vec![10.0, 10.0],
        vec![-10.0, 10.0],
        vec![10.0, -10.0],
        vec![0.0, 10.0],
        vec![0.0, -10.0],
    ];

    for point in test_points {
        let x = Array1::from(point.clone());
        let f = periodic(&x);
        assert!(
            f.is_finite(),
            "Function value at {:?} should be finite: {}",
            point,
            f
        );
        assert!(f > 0.9, "Function should be >= 0.9 everywhere: {}", f);
    }
}
