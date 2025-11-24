use autoeq_de::{DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::epistatic_michalewicz;

#[test]
fn test_de_epistatic_michalewicz_2d() {
    // Test Epistatic Michalewicz function in 2D - modified with interactions
    let bounds = vec![(0.0, std::f64::consts::PI), (0.0, std::f64::consts::PI)];
    let config = DEConfigBuilder::new()
        .seed(240)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "epistatic_michalewicz_2d",
        epistatic_michalewicz,
        &bounds,
        config,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // Epistatic Michalewicz is very challenging - global minimum varies by dimension
    // For 2D, expect around -1.8 but accept reasonable results
    assert!(
        report.fun < -1.0,
        "Solution quality too low: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= 0.0 && xi <= std::f64::consts::PI,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}

#[test]
fn test_de_epistatic_michalewicz_5d() {
    // Test Epistatic Michalewicz function in 5D - extremely challenging with interactions
    let bounds = vec![(0.0, std::f64::consts::PI); 5];
    let config = DEConfigBuilder::new()
        .seed(241)
        .maxiter(3000)
        .popsize(150)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();

    let result = run_recorded_differential_evolution(
        "epistatic_michalewicz_5d",
        epistatic_michalewicz,
        &bounds,
        config,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // For 5D with epistatic terms, this is extremely challenging
    // Just ensure we get some reasonable negative value
    assert!(
        report.fun < -2.0,
        "Solution quality too low for 5D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= 0.0 && xi <= std::f64::consts::PI,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}

#[test]
fn test_de_epistatic_michalewicz_10d() {
    // Test Epistatic Michalewicz function in 10D - ultimate challenge
    let bounds = vec![(0.0, std::f64::consts::PI); 10];
    let config = DEConfigBuilder::new()
        .seed(242)
        .maxiter(4000)
        .popsize(200)
        .strategy(Strategy::Best2Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "epistatic_michalewicz_10d",
        epistatic_michalewicz,
        &bounds,
        config,
    );
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();

    // For 10D, just ensure we get some improvement from random
    assert!(
        report.fun < -4.0,
        "Solution quality too low for 10D: {}",
        report.fun
    );

    // Check solution is within bounds
    for &xi in report.x.iter() {
        assert!(
            xi >= 0.0 && xi <= std::f64::consts::PI,
            "Solution coordinate out of bounds: {}",
            xi
        );
    }
}

#[test]
fn test_epistatic_michalewicz_interaction_effects() {
    // Test that epistatic (interaction) terms have effect
    use ndarray::Array1;

    // Test that the function has some variability indicating interactions
    let test_points = vec![
        vec![1.0, 2.0],
        vec![2.0, 1.0], // Swapped coordinates
        vec![1.5, 1.5], // Average
    ];

    let mut values = Vec::new();
    for point in test_points {
        let x = Array1::from(point.clone());
        let f = epistatic_michalewicz(&x);
        values.push(f);
        assert!(
            f.is_finite(),
            "Function should be finite at {:?}: {}",
            point,
            f
        );
    }

    // The function should show some variation due to interactions
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    // If there are epistatic effects, we should see some variation
    let variation = max_val - min_val;
    assert!(
        variation > 1e-6,
        "Expected some variation due to epistatic terms: {}",
        variation
    );
}
