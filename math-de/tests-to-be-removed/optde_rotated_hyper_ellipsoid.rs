use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::rotated_hyper_ellipsoid;

#[test]
fn test_de_rotated_hyper_ellipsoid_2d() {
    // Test 2D Rotated Hyper-Ellipsoid function
    let b = vec![(-65.536, 65.536), (-65.536, 65.536)];
    let c = DEConfigBuilder::new()
        .seed(86)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution(
        "rotated_hyper_ellipsoid_2d",
        rotated_hyper_ellipsoid,
        &b,
        c,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum f(x) = 0 at origin
    assert!(report.fun < 1e-6, "Function value too high: {}", report.fun);
    // Check solution is close to origin
    for (i, &xi) in report.x.iter().enumerate() {
        assert!(xi.abs() < 1e-3, "x[{}] should be close to 0: {}", i, xi);
    }
}

#[test]
fn test_de_rotated_hyper_ellipsoid_5d() {
    // Test 5D Rotated Hyper-Ellipsoid function (non-separable)
    let b5 = vec![(-65.536, 65.536); 5];
    let c5 = DEConfigBuilder::new()
        .seed(86)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution(
        "rotated_hyper_ellipsoid_5d",
        rotated_hyper_ellipsoid,
        &b5,
        c5,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum f(x) = 0 at origin
    assert!(report.fun < 1e-3, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_rotated_hyper_ellipsoid_10d() {
    // Test 10D Rotated Hyper-Ellipsoid function
    let b10 = vec![(-30.0, 30.0); 10]; // Smaller bounds for higher dimensions
    let c10 = DEConfigBuilder::new()
        .seed(87)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution(
        "rotated_hyper_ellipsoid_10d",
        rotated_hyper_ellipsoid,
        &b10,
        c10,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin, gets harder in higher dimensions due to non-separability
    assert!(report.fun < 1e-1, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_rotated_hyper_ellipsoid_large_bounds() {
    // Test with the standard large bounds to verify robustness
    let b = vec![(-65.536, 65.536); 3];
    let c = DEConfigBuilder::new()
        .seed(88)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.8)
        .build();
    let result = run_recorded_differential_evolution(
        "rotated_hyper_ellipsoid_large_bounds",
        rotated_hyper_ellipsoid,
        &b,
        c,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < 5e-2,
        "Function value too high with large bounds: {}",
        report.fun
    );
}

#[test]
fn test_rotated_hyper_ellipsoid_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At origin (global minimum)
    let x_origin = Array1::from(vec![0.0, 0.0, 0.0]);
    let f_origin = rotated_hyper_ellipsoid(&x_origin);
    assert!(
        f_origin < 1e-15,
        "Origin should be global minimum: {}",
        f_origin
    );

    // Test the function structure: sum_{i=0}^{n-1} sum_{j=0}^{i} x[j]^2
    // For 3D: x[0]^2 + (x[0]^2 + x[1]^2) + (x[0]^2 + x[1]^2 + x[2]^2)
    //        = 3*x[0]^2 + 2*x[1]^2 + x[2]^2

    let x_test = Array1::from(vec![1.0, 1.0, 1.0]);
    let f_test = rotated_hyper_ellipsoid(&x_test);
    // Manual calculation: 3*1^2 + 2*1^2 + 1*1^2 = 3 + 2 + 1 = 6
    let expected = 3.0 + 2.0 + 1.0;
    assert!(
        (f_test - expected).abs() < 1e-15,
        "3D test calculation incorrect: {} vs {}",
        f_test,
        expected
    );

    // Test for 2D case: x[0]^2 + (x[0]^2 + x[1]^2) = 2*x[0]^2 + x[1]^2
    let x_2d = Array1::from(vec![1.0, 2.0]);
    let f_2d = rotated_hyper_ellipsoid(&x_2d);
    // Manual calculation: 2*1^2 + 1*2^2 = 2 + 4 = 6
    let expected_2d = 2.0 * 1.0 + 1.0 * 4.0;
    assert!(
        (f_2d - expected_2d).abs() < 1e-15,
        "2D test calculation incorrect: {} vs {}",
        f_2d,
        expected_2d
    );

    // Test non-separability - earlier variables are weighted more heavily
    let x_first = Array1::from(vec![1.0, 0.0, 0.0]);
    let f_first = rotated_hyper_ellipsoid(&x_first);

    let x_last = Array1::from(vec![0.0, 0.0, 1.0]);
    let f_last = rotated_hyper_ellipsoid(&x_last);

    // First variable appears in all terms, last variable only in its own term
    // f_first = 3*1^2 + 0 + 0 = 3, f_last = 0 + 0 + 1*1^2 = 1
    assert!(
        f_first > f_last,
        "Earlier variables should be weighted more heavily: {} vs {}",
        f_first,
        f_last
    );
    assert!(
        (f_first - 3.0).abs() < 1e-15,
        "First variable test incorrect: {}",
        f_first
    );
    assert!(
        (f_last - 1.0).abs() < 1e-15,
        "Last variable test incorrect: {}",
        f_last
    );

    // Test scaling behavior
    let x_scaled = Array1::from(vec![2.0, 2.0, 2.0]);
    let f_scaled = rotated_hyper_ellipsoid(&x_scaled);
    // Should be 4 times the original since it's quadratic: 4*(3 + 2 + 1) = 24
    let expected_scaled = 4.0 * expected;
    assert!(
        (f_scaled - expected_scaled).abs() < 1e-15,
        "Scaling test incorrect: {} vs {}",
        f_scaled,
        expected_scaled
    );
}
