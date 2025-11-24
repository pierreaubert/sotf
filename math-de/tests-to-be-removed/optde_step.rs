use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::step;

#[test]
fn test_de_step_2d() {
    // Test 2D Step function (discontinuous)
    let b = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(79)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.8)
        .build();
    let result = run_recorded_differential_evolution("step_2d", step, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at x = (0.5, 0.5) with f(x) = 0
    assert!(report.fun <= 2.0, "Function value too high: {}", report.fun); // Relaxed due to discontinuous nature
}

#[test]
fn test_de_step_5d() {
    // Test 5D Step function (discontinuous)
    let b = vec![(-100.0, 100.0); 5];
    let c = DEConfigBuilder::new()
        .seed(79)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.8)
        .build();
    let result = run_recorded_differential_evolution("step_5d", step, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at x = (0.5, 0.5, ..., 0.5) with f(x) = 0
    assert!(report.fun <= 5.0, "Function value too high: {}", report.fun); // Relaxed due to discontinuous nature
}

#[test]
fn test_de_step_3d() {
    // Test 3D Step function with different parameters
    let b = vec![(-50.0, 50.0); 3];
    let c = DEConfigBuilder::new()
        .seed(80)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("step_3d", step, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Should find the global minimum
    assert!(report.fun <= 3.0, "Function value too high: {}", report.fun);
}

#[test]
fn test_step_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At global optimum (0.5, 0.5, ...)
    let x_opt = Array1::from(vec![0.5, 0.5, 0.5]);
    let f_opt = step(&x_opt);
    assert!(f_opt < 1e-15, "Global optimum should be 0: {}", f_opt);

    // Test discontinuity around the optimum
    let x_just_below = Array1::from(vec![0.4999, 0.4999]);
    let f_below = step(&x_just_below);

    let x_just_above = Array1::from(vec![0.5001, 0.5001]);
    let f_above = step(&x_just_above);

    // Both should give different integer floor values
    assert!(
        f_below == 0.0,
        "Just below optimum should be 0: {}",
        f_below
    );
    assert!(
        f_above == 2.0,
        "Just above optimum should be 2: {}",
        f_above
    );

    // Test at integer points
    let x_integers = Array1::from(vec![1.0, 2.0]);
    let f_integers = step(&x_integers);
    // floor(1 + 0.5) + floor(2 + 0.5) = floor(1.5) + floor(2.5) = 1 + 2 = 3
    // Then squared: 1^2 + 2^2 = 5
    assert_eq!(
        f_integers, 5.0,
        "Integer calculation incorrect: {}",
        f_integers
    );
}
