use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::vincent;

#[test]
fn test_de_vincent_2d() {
    // Test Vincent function in 2D
    let bounds = vec![(0.25, 10.0), (0.25, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(130)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("vincent_2d", vincent, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < -1.8,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is a valid global optimum: any xi such that sin(10*xi) ≈ 1 within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= 0.25 && xi <= 10.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
        let s = (10.0 * xi).sin();
        assert!(
            (s - 1.0).abs() < 1e-3,
            "sin(10*x) not near 1 (global optimum condition): x={}, sin(10x)={}",
            xi,
            s
        );
    }
}

#[test]
fn test_de_vincent_5d() {
    // Test Vincent function in 5D
    let bounds = vec![(0.25, 10.0); 5];
    let config = DEConfigBuilder::new()
        .seed(131)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("vincent_5d", vincent, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(
        report.fun < -4.5,
        "Solution quality too low for 5D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= 0.25 && xi <= 10.0,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}

#[test]
fn test_vincent_multimodal_behavior() {
    // Test that Vincent function has multiple local optima
    use ndarray::Array1;

    let test_points = vec![
        vec![1.57, 1.57], // Around π/2
        vec![4.71, 4.71], // Around 3π/2
        vec![7.85, 7.85], // Around 5π/2
    ];

    for point in test_points {
        let x = Array1::from(point.clone());
        let f = vincent(&x);

        // All these points should give reasonable (negative) values
        assert!(
            f < 0.0,
            "Vincent function should be negative at local optima: {:?} -> {}",
            point,
            f
        );
        assert!(f.is_finite(), "Function should be finite");
    }
}
