use autoeq_de::{DEConfig, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{levi13, levy_n13};

#[test]
fn test_de_levi13_basic() {
    // Test Lévy N.13 function using basic DE config
    let b = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let mut c = DEConfig::default();
    c.seed = Some(12);
    c.maxiter = 600;
    c.popsize = 25;
    {
        let result = run_recorded_differential_evolution("levi13_basic", levi13, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-3)
    };
}

#[test]
fn test_de_levi13_advanced() {
    // Test Lévy N.13 function with advanced parameters
    let b = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(83)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = run_recorded_differential_evolution("levi13_advanced", levy_n13, &b, c);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    // Global minimum f(x) = 0 at x = (1, 1)
    assert!(report.fun < 1e-2, "Function value too high: {}", report.fun);
    // Check solution is close to (1, 1)
    assert!(
        (report.x[0] - 1.0).abs() < 0.1,
        "x[0] should be close to 1: {}",
        report.x[0]
    );
    assert!(
        (report.x[1] - 1.0).abs() < 0.1,
        "x[1] should be close to 1: {}",
        report.x[1]
    );
}

#[test]
fn test_de_levi13_multistart() {
    // Test with multiple random starts to verify robustness
    let b = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let seeds = vec![42, 123, 456];

    for (i, &seed) in seeds.iter().enumerate() {
        let c = DEConfigBuilder::new()
            .seed(seed)
            .maxiter(1000)
            .popsize(60)
            .strategy(Strategy::RandToBest1Exp)
            .recombination(0.95)
            .build();
        let result = run_recorded_differential_evolution("levi13_multistart", levy_n13, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(
            report.fun < 5e-2,
            "Run {} (seed {}) failed: f = {}",
            i,
            seed,
            report.fun
        );
    }
}

#[test]
fn test_levi13_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At global optimum (1, 1)
    let x_opt = Array1::from(vec![1.0, 1.0]);
    let f_opt = levy_n13(&x_opt);
    // Should be very close to 0
    assert!(f_opt < 1e-10, "Global optimum should be near 0: {}", f_opt);

    // Test the function structure
    // levy_n13 is same as levi13 - they should be identical
    let x_test = Array1::from(vec![0.5, -0.5]);
    let f_levy = levy_n13(&x_test);
    let f_levi = levi13(&x_test);
    assert!(
        (f_levy - f_levi).abs() < 1e-15,
        "levy_n13 and levi13 should be identical: {} vs {}",
        f_levy,
        f_levi
    );

    // Test at origin - should be worse than optimum
    let x_origin = Array1::from(vec![0.0, 0.0]);
    let f_origin = levy_n13(&x_origin);
    assert!(
        f_origin > f_opt,
        "Origin should be worse than optimum: {} vs {}",
        f_origin,
        f_opt
    );
}
