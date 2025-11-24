use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::quartic;

#[test]
fn test_de_quartic_2d() {
    // Test 2D Quartic function
    let b = vec![(-1.28, 1.28), (-1.28, 1.28)];
    let c = DEConfigBuilder::new()
        .seed(80)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("quartic_2d", quartic, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin
    assert!(report.fun < 1e-6, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_quartic_5d() {
    // Test 5D Quartic function (high-order polynomial)
    let b = vec![(-1.28, 1.28); 5];
    let c = DEConfigBuilder::new()
        .seed(80)
        .maxiter(1000)
        .popsize(80)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("quartic_5d", quartic, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin
    assert!(report.fun < 1e-3, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_quartic_3d() {
    // Test 3D Quartic function
    let b = vec![(-1.28, 1.28); 3];
    let c = DEConfigBuilder::new()
        .seed(81)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution("quartic_3d", quartic, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin
    assert!(report.fun < 1e-4, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_quartic_10d() {
    // Test 10D Quartic function
    let b = vec![(-1.0, 1.0); 10]; // Slightly smaller bounds for higher dimensions
    let c = DEConfigBuilder::new()
        .seed(82)
        .maxiter(1500)
        .popsize(120)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("quartic_10d", quartic, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin, harder in higher dimensions
    assert!(report.fun < 1e-1, "Function value too high: {}", report.fun);
}

#[test]
fn test_quartic_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At origin (global minimum)
    let x_origin = Array1::from(vec![0.0, 0.0, 0.0]);
    let f_origin = quartic(&x_origin);
    assert!(
        f_origin < 1e-15,
        "Origin should be global minimum: {}",
        f_origin
    );

    // Test the weighted quartic: (i+1) * x[i]^4
    let x_test = Array1::from(vec![0.1, 0.1, 0.1]);
    let f_test = quartic(&x_test);

    // Manual calculation: 1*(0.1)^4 + 2*(0.1)^4 + 3*(0.1)^4 = (1+2+3)*(0.1)^4 = 6*0.0001 = 0.0006
    let expected = 1.0 * 0.1_f64.powi(4) + 2.0 * 0.1_f64.powi(4) + 3.0 * 0.1_f64.powi(4);
    assert!(
        (f_test - expected).abs() < 1e-15,
        "Function calculation incorrect: {} vs {}",
        f_test,
        expected
    );

    // Test that higher-index components are weighted more heavily
    let x1 = Array1::from(vec![0.5, 0.0, 0.0]);
    let f1 = quartic(&x1);

    let x2 = Array1::from(vec![0.0, 0.0, 0.5]);
    let f2 = quartic(&x2);

    // f2 should be larger because 3*x[2]^4 > 1*x[0]^4 for same |x|
    assert!(
        f2 > f1,
        "Higher indexed components should be weighted more: {} vs {}",
        f2,
        f1
    );
    assert!(
        (f2 / f1 - 3.0).abs() < 1e-10,
        "Weight ratio should be 3: {}",
        f2 / f1
    );
}
