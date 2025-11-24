use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::colville;

#[test]
fn test_de_colville_4d() {
    // Test 4D Colville function (multimodal, non-separable)
    let b4 = vec![(-10.0, 10.0); 4];
    let c4 = DEConfigBuilder::new()
        .seed(85)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution("colville_4d", colville, &b4, c4);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum f(x) = 0 at x = (1, 1, 1, 1)
    assert!(report.fun < 1e-2, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_colville_focused_bounds() {
    // Test with bounds closer to the known optimum
    let b = vec![(0.0, 2.0), (0.0, 2.0), (0.0, 2.0), (0.0, 2.0)]; // Focused around (1,1,1,1)
    let c = DEConfigBuilder::new()
        .seed(86)
        .maxiter(1200)
        .popsize(100)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("colville_focused", colville, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Should find better solution with focused bounds
    assert!(report.fun < 1e-3, "Function value too high: {}", report.fun);
    // Check solution is close to (1, 1, 1, 1)
    for (i, &xi) in report.x.iter().enumerate() {
        assert!(
            (xi - 1.0).abs() < 0.1,
            "x[{}] should be close to 1: {}",
            i,
            xi
        );
    }
}

#[test]
fn test_de_colville_2d() {
    // Test 2D version of Colville (function adapts to dimension)
    let b2 = vec![(-5.0, 5.0); 2];
    let c2 = DEConfigBuilder::new()
        .seed(87)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.8)
        .build();
    let result = run_recorded_differential_evolution("colville_2d", colville, &b2, c2);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Function should adapt to 2D and use default values for missing dimensions
    assert!(
        report.fun < 5e-2,
        "Function value too high for 2D: {}",
        report.fun
    );
}

#[test]
fn test_de_colville_multistart() {
    // Test with multiple random starts to handle multimodality
    let b = vec![(-8.0, 8.0); 4];
    let seeds = [400, 500, 600];

    let mut best_result = f64::INFINITY;
    for (i, &seed) in seeds.iter().enumerate() {
        let c = DEConfigBuilder::new()
            .seed(seed)
            .maxiter(1500)
            .popsize(100)
            .strategy(Strategy::Best1Bin)
            .recombination(0.9)
            .build();
        let result = run_recorded_differential_evolution(
            &format!("colville_multistart_{}", i),
            colville,
            &b,
            c,
        );
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        best_result = best_result.min(report.fun);
        println!("Run {} (seed {}): f = {}", i, seed, report.fun);
    }

    // At least one run should find a good solution
    assert!(
        best_result < 0.1,
        "Best result across runs too high: {}",
        best_result
    );
}

#[test]
fn test_colville_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At global optimum (1, 1, 1, 1)
    let x_opt = Array1::from(vec![1.0, 1.0, 1.0, 1.0]);
    let f_opt = colville(&x_opt);
    // Should be exactly 0
    assert!(f_opt < 1e-15, "Global optimum should be 0: {}", f_opt);

    // Test the function structure - it's a complex quartic function
    // f = 100*(x1^2 - x2)^2 + (x1-1)^2 + (x3-1)^2 + 90*(x3^2 - x4)^2 + 10.1*((x2-1)^2 + (x4-1)^2) + 19.8*(x2-1)*(x4-1)

    // At origin (should be worse)
    let x_origin = Array1::from(vec![0.0, 0.0, 0.0, 0.0]);
    let f_origin = colville(&x_origin);

    // Manual calculation for (0,0,0,0):
    // f = 100*(0^2 - 0)^2 + (0-1)^2 + (0-1)^2 + 90*(0^2 - 0)^2 + 10.1*((0-1)^2 + (0-1)^2) + 19.8*(0-1)*(0-1)
    // f = 100*0 + 1 + 1 + 90*0 + 10.1*(1 + 1) + 19.8*(-1)*(-1) = 0 + 1 + 1 + 0 + 20.2 + 19.8 = 42.0
    let expected_origin = 1.0 + 1.0 + 10.1 * 2.0 + 19.8;
    assert!(
        (f_origin - expected_origin).abs() < 1e-10,
        "Origin calculation incorrect: {} vs {}",
        f_origin,
        expected_origin
    );

    // Test adaptation to different dimensions
    let x_2d = Array1::from(vec![1.0, 1.0]);
    let f_2d = colville(&x_2d);
    // With 2D, x3 and x4 default to 1.0, so should be close to 0
    assert!(f_2d < 1e-10, "2D optimum should be near 0: {}", f_2d);

    let x_3d = Array1::from(vec![1.0, 1.0, 1.0]);
    let f_3d = colville(&x_3d);
    // With 3D, x4 defaults to 1.0, so should be close to 0
    assert!(f_3d < 1e-10, "3D optimum should be near 0: {}", f_3d);

    // Test non-separable nature - changing one variable affects multiple terms
    let x_perturb = Array1::from(vec![1.1, 1.0, 1.0, 1.0]);
    let f_perturb = colville(&x_perturb);
    assert!(
        f_perturb > f_opt,
        "Perturbation should increase function value: {} vs {}",
        f_perturb,
        f_opt
    );
}
