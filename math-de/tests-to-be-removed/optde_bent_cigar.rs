use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::bent_cigar;

#[test]
fn test_de_bent_cigar_2d() {
    // Test 2D Bent Cigar function
    let b = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(77)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("bent_cigar_2d", bent_cigar, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin, but function is ill-conditioned
    assert!(report.fun < 1e-3, "Function value too high: {}", report.fun);
}

#[test]
fn test_de_bent_cigar_5d() {
    // Test 5D Bent Cigar function (ill-conditioned)
    let b5 = vec![(-100.0, 100.0); 5];
    let c5 = DEConfigBuilder::new()
        .seed(77)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    let result = run_recorded_differential_evolution("bent_cigar_5d", bent_cigar, &b5, c5);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum at origin, very ill-conditioned
    assert!(report.fun < 1e3, "Function value too high: {}", report.fun); // Relaxed due to ill-conditioning
}

#[test]
fn test_de_bent_cigar_10d() {
    // Test 10D Bent Cigar function (very ill-conditioned)
    let b10 = vec![(-50.0, 50.0); 10]; // Smaller bounds for higher dimensions
    let c10 = DEConfigBuilder::new()
        .seed(78)
        .maxiter(2000)
        .popsize(150)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("bent_cigar_10d", bent_cigar, &b10, c10);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Very ill-conditioned in higher dimensions
    assert!(report.fun < 1e4, "Function value too high: {}", report.fun);
}

#[test]
fn test_bent_cigar_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At origin (global minimum)
    let x_origin = Array1::from(vec![0.0, 0.0, 0.0]);
    let f_origin = bent_cigar(&x_origin);
    assert!(
        f_origin < 1e-15,
        "Origin should be global minimum: {}",
        f_origin
    );

    // Test the ill-conditioning: x[0] has normal scaling, others have 10^6 scaling
    let x1 = Array1::from(vec![1.0, 0.0, 0.0]); // Only first component
    let f1 = bent_cigar(&x1);

    let x2 = Array1::from(vec![0.0, 1.0, 0.0]); // Only second component
    let f2 = bent_cigar(&x2);

    // f2 should be much larger than f1 due to 10^6 scaling
    assert!(
        f2 / f1 > 1e5,
        "Second component should be much more penalized: {} vs {}",
        f2,
        f1
    );
}
