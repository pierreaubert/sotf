use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::happy_cat;

#[test]
fn test_de_happy_cat_2d() {
    // Test Happy Cat function in 2D
    let bounds = vec![(-2.0, 2.0), (-2.0, 2.0)];
    let config = DEConfigBuilder::new()
        .seed(120)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("happy_cat_2d", happy_cat, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 0.1, "Solution quality too low: {}", report.fun);

    // Check solution is close to one of the global minima (±1, ±1)
    let found_minimum = report.x.iter().all(|&xi| (xi.abs() - 1.0).abs() < 0.2);
    assert!(
        found_minimum,
        "Solution not near global minima: {:?}",
        report.x
    );
}

#[test]
fn test_de_happy_cat_5d() {
    // Test Happy Cat function in 5D
    let bounds = vec![(-2.0, 2.0); 5];
    let config = DEConfigBuilder::new()
        .seed(121)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("happy_cat_5d", happy_cat, &bounds, config);
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 0.5, "Solution quality too low: {}", report.fun);

    // Check solution is reasonably close to some optimum
    for &xi in report.x.iter() {
        assert!(
            xi.abs() <= 2.5,
            "Solution coordinate out of expected range: {}",
            xi
        );
    }
}

#[test]
fn test_happy_cat_known_minimum() {
    // Test that the known global minima give the expected value
    use ndarray::Array1;
    let x_star1 = Array1::from(vec![1.0, 1.0]);
    let f_star1 = happy_cat(&x_star1);

    let x_star2 = Array1::from(vec![-1.0, -1.0]);
    let f_star2 = happy_cat(&x_star2);

    // Both should be approximately 0
    assert!(
        f_star1 < 0.01,
        "Known minimum (1,1) doesn't match expected value: {}",
        f_star1
    );
    assert!(
        f_star2 < 0.01,
        "Known minimum (-1,-1) doesn't match expected value: {}",
        f_star2
    );
}
