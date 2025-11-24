use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::trid;

#[test]
fn test_de_trid_2d() {
    // Test 2D Trid function
    let b = vec![(-4.0, 4.0), (-4.0, 4.0)]; // bounds: [-d^2, d^2]
    let c = DEConfigBuilder::new()
        .seed(76)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("trid_2d", trid, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // 2D Trid has global minimum f(x) = -2 at x = (2, 2)
    assert!(report.fun < -1.8, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_trid_3d() {
    // Test 3D Trid function
    let b = vec![(-9.0, 9.0); 3]; // bounds: [-d^2, d^2] where d=3
    let c = DEConfigBuilder::new()
        .seed(77)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("trid_3d", trid, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // 3D Trid has global minimum f(x) = -6 at x = (2, 3, 3)
    assert!(report.fun < -5.5, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_trid_4d() {
    // Test 4D Trid function
    let b = vec![(-16.0, 16.0); 4]; // bounds: [-d^2, d^2] where d=4
    let c = DEConfigBuilder::new()
        .seed(78)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::Best1Bin)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution("trid_4d", trid, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // 4D Trid has global minimum f(x) = -12 at x = (2, 3, 4, 4)
    assert!(
        report.fun < -11.0,
        "Function value too high: {}",
        report.fun
    );
}

#[test]
fn test_trid_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At 2D optimum (2, 2)
    let x_2d = Array1::from(vec![2.0, 2.0]);
    let f_2d = trid(&x_2d);
    // Should be close to -2
    assert!(
        (f_2d - (-2.0)).abs() < 1e-10,
        "2D optimum value incorrect: {}",
        f_2d
    );

    // At origin (should be higher)
    let x_origin = Array1::from(vec![0.0, 0.0]);
    let f_origin = trid(&x_origin);
    assert!(
        f_origin > f_2d,
        "Origin should be worse than optimum: {} vs {}",
        f_origin,
        f_2d
    );
}
