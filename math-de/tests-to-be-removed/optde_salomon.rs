use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::salomon;

#[test]
fn test_de_salomon_2d() {
    // Test 2D Salomon function (multimodal)
    let b = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(81)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution("salomon_2d", salomon, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin with f(x) = 0
    assert!(report.fun < 1e-2, "Function value too high: {}", report.fun); // Relaxed due to multimodal nature
}

#[test]
fn test_de_salomon_3d() {
    // Test 3D Salomon function (multimodal)
    let b = vec![(-100.0, 100.0); 3];
    let c = DEConfigBuilder::new()
        .seed(81)
        .maxiter(1200)
        .popsize(60)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution("salomon_3d", salomon, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin with f(x) = 0
    assert!(report.fun < 1e-1, "Function value too high: {}", report.fun); // Relaxed due to multimodal nature
}

#[test]
fn test_de_salomon_5d() {
    // Test 5D Salomon function
    let b = vec![(-50.0, 50.0); 5]; // Smaller bounds for higher dimensions
    let c = DEConfigBuilder::new()
        .seed(82)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("salomon_5d", salomon, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin, but multimodal makes it challenging
    assert!(report.fun < 5e-1, "Function value too high: {}", report.fun);
}

#[test]
fn test_salomon_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At origin (global minimum)
    let x_origin = Array1::from(vec![0.0, 0.0]);
    let f_origin = salomon(&x_origin);
    // f(0) = 1 - cos(2π*0) + 0.1*0 = 1 - 1 + 0 = 0
    assert!(
        f_origin < 1e-15,
        "Origin should be global minimum: {}",
        f_origin
    );

    // Test the function structure: 1 - cos(2π*||x||) + 0.1*||x||
    let x_test = Array1::from(vec![1.0, 0.0]);
    let f_test = salomon(&x_test);
    let norm = 1.0;
    let expected = 1.0 - (2.0 * std::f64::consts::PI * norm).cos() + 0.1 * norm;
    assert!(
        (f_test - expected).abs() < 1e-15,
        "Function calculation incorrect: {} vs {}",
        f_test,
        expected
    );

    // Test multimodal nature - there should be local minima at multiples where cos term = 1
    // At norm = 1, cos(2π) = 1, so f = 1 - 1 + 0.1 = 0.1
    // At norm = 2, cos(4π) = 1, so f = 1 - 1 + 0.2 = 0.2
    let x_norm1 = Array1::from(vec![1.0, 0.0]);
    let f_norm1 = salomon(&x_norm1);
    assert!(
        (f_norm1 - 0.1).abs() < 1e-10,
        "f at norm=1 should be 0.1: {}",
        f_norm1
    );

    let x_norm2 = Array1::from(vec![2.0, 0.0]);
    let f_norm2 = salomon(&x_norm2);
    assert!(
        (f_norm2 - 0.2).abs() < 1e-10,
        "f at norm=2 should be 0.2: {}",
        f_norm2
    );

    // The global minimum should be better than local minima
    assert!(
        f_origin < f_norm1,
        "Global minimum should be better than local minima"
    );
    assert!(f_norm1 < f_norm2, "Closer local minima should be better");
}
