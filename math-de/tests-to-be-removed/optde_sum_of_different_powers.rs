use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::sum_of_different_powers;

#[test]
fn test_de_sum_of_different_powers_2d() {
    // Test 2D Sum of Different Powers function
    let b = vec![(-1.0, 1.0), (-1.0, 1.0)];
    let c = DEConfigBuilder::new()
        .seed(78)
        .maxiter(500)
        .popsize(30)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution(
        "sum_of_different_powers_2d",
        sum_of_different_powers,
        &b,
        c,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin
    assert!(report.fun < 1e-6, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_sum_of_different_powers_5d() {
    // Test 5D Sum of Different Powers function
    let b = vec![(-1.0, 1.0); 5];
    let c = DEConfigBuilder::new()
        .seed(78)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution(
        "sum_of_different_powers_5d",
        sum_of_different_powers,
        &b,
        c,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin
    assert!(report.fun < 1e-2, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_sum_of_different_powers_10d() {
    // Test 10D Sum of Different Powers function
    let b = vec![(-1.0, 1.0); 10];
    let c = DEConfigBuilder::new()
        .seed(79)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::Best1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution(
        "sum_of_different_powers_10d",
        sum_of_different_powers,
        &b,
        c,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin, but gets harder in higher dimensions
    assert!(report.fun < 1e-1, "Function value too high: {}", report.fun);
}

#[test]
fn test_sum_of_different_powers_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At origin (global minimum)
    let x_origin = Array1::from(vec![0.0, 0.0, 0.0, 0.0]);
    let f_origin = sum_of_different_powers(&x_origin);
    assert!(
        f_origin < 1e-15,
        "Origin should be global minimum: {}",
        f_origin
    );

    // Test the increasing powers: x[i]^(i+2)
    let x_test = Array1::from(vec![0.1, 0.1, 0.1]);
    let f_test = sum_of_different_powers(&x_test);

    // Manual calculation: |0.1|^2 + |0.1|^3 + |0.1|^4 = 0.01 + 0.001 + 0.0001 = 0.0111
    let expected = 0.1_f64.powi(2) + 0.1_f64.powi(3) + 0.1_f64.powi(4);
    assert!(
        (f_test - expected).abs() < 1e-15,
        "Function calculation incorrect: {} vs {}",
        f_test,
        expected
    );

    // Test that higher-index components are more sensitive
    let x1 = Array1::from(vec![0.5, 0.0, 0.0]);
    let f1 = sum_of_different_powers(&x1);

    let x2 = Array1::from(vec![0.0, 0.0, 0.5]);
    let f2 = sum_of_different_powers(&x2);

    // f2 should be larger because x[2]^4 > x[0]^2 for |x| = 0.5
    assert!(f2 > f1, "Higher powers should dominate: {} vs {}", f2, f1);
}
