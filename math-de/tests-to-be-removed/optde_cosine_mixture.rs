use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::cosine_mixture;

#[test]
fn test_de_cosine_mixture_2d() {
    // Test 2D Cosine Mixture function
    let b = vec![(-1.0, 1.0), (-1.0, 1.0)];
    let c = DEConfigBuilder::new()
        .seed(82)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("cosine_mixture_2d", cosine_mixture, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum depends on dimension
    assert!(report.fun < 0.1, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_cosine_mixture_4d() {
    // Test 4D Cosine Mixture function
    let b = vec![(-1.0, 1.0); 4];
    let c = DEConfigBuilder::new()
        .seed(82)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("cosine_mixture_4d", cosine_mixture, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum depends on dimension
    assert!(report.fun < 0.1, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_cosine_mixture_6d() {
    // Test 6D Cosine Mixture function
    let b = vec![(-1.0, 1.0); 6];
    let c = DEConfigBuilder::new()
        .seed(83)
        .maxiter(1000)
        .popsize(80)
        .strategy(Strategy::Best1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution("cosine_mixture_6d", cosine_mixture, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum for higher dimensions
    assert!(report.fun < 0.2, "Function value too high: {}", report.fun);
}

#[test]
fn test_cosine_mixture_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // The function is: -0.1 * sum(cos(5π*xi)) + sum(xi^2)
    // Global minimum is achieved when cos terms are maximized (=1) and xi^2 terms are minimized

    // At origin
    let x_origin = Array1::from(vec![0.0, 0.0]);
    let f_origin = cosine_mixture(&x_origin);
    // f(0) = -0.1 * (cos(0) + cos(0)) + (0 + 0) = -0.1 * (1 + 1) + 0 = -0.2
    let expected_origin = -0.1 * 2.0; // 2 dimensions, cos(0) = 1
    assert!(
        (f_origin - expected_origin).abs() < 1e-15,
        "Origin value incorrect: {} vs {}",
        f_origin,
        expected_origin
    );

    // Test the cosine component behavior
    // cos(5π*xi) = 1 when 5π*xi = 2πk, i.e., xi = 2k/5
    let x_cos_max = Array1::from(vec![2.0 / 5.0, 0.0]); // cos(5π*2/5) = cos(2π) = 1
    let f_cos_max = cosine_mixture(&x_cos_max);
    let expected_cos = -0.1 * (1.0 + 1.0) + (2.0f64 / 5.0).powi(2) + 0.0;
    assert!(
        (f_cos_max - expected_cos).abs() < 1e-15,
        "Cosine max calculation incorrect: {} vs {}",
        f_cos_max,
        expected_cos
    );

    // Test that moving away from cosine maxima increases function value
    let x_cos_min = Array1::from(vec![0.1, 0.0]); // cos(5π*0.1) = cos(π/2) = 0
    let f_cos_min = cosine_mixture(&x_cos_min);
    // Should be worse than origin due to cosine term being 0 instead of 1, plus quadratic penalty
    assert!(
        f_cos_min > f_origin,
        "Moving from cosine optimum should increase function value: {} vs {}",
        f_cos_min,
        f_origin
    );

    // Test multimodal nature - multiple local optima exist
    let x_local1 = Array1::from(vec![2.0 / 5.0, 2.0 / 5.0]); // Both at cosine maxima
    let f_local1 = cosine_mixture(&x_local1);

    let x_local2 = Array1::from(vec![4.0 / 5.0, 0.0]); // cos(5π*4/5) = cos(4π) = 1
    let f_local2 = cosine_mixture(&x_local2);

    // Both should be local optima but with different quadratic penalties
    assert!(
        f_local1 < f_local2,
        "Closer to origin should be better: {} vs {}",
        f_local1,
        f_local2
    );
}
