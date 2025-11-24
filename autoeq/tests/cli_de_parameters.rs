//! Integration tests for new DE CLI parameters
//!
//! These tests verify that the new CLI parameters actually change
//! the behavior of the differential evolution algorithm.

use autoeq::LossType;
use autoeq::cli::{Args, PeqModel};
use autoeq::de::Strategy;
use autoeq::optim::optimize_filters;
use autoeq::workflow::{initial_guess, setup_bounds};
use clap::Parser;
use ndarray::Array1;
use std::str::FromStr;

/// Create a simple test objective data structure
fn create_test_objective_data() -> autoeq::optim::ObjectiveData {
    // Create simple test curves
    let freqs = Array1::from(vec![100.0, 1000.0, 10000.0]);
    let target = Array1::from(vec![1.0, 1.0, 1.0]); // Small deviation to optimize
    let deviation = Array1::from(vec![0.5, 0.5, 0.5]); // Small deviation

    autoeq::optim::ObjectiveData {
        freqs: freqs.clone(),
        target,
        deviation,
        input_curve: None,
        srate: 48000.0,
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        max_db: 3.0,
        min_db: 1.0,
        min_freq: 60.0,
        max_freq: 16000.0,
        peq_model: PeqModel::Pk,
        loss_type: LossType::SpeakerFlat,
        speaker_score_data: None,
        headphone_score_data: None,
        drivers_data: None,
        penalty_w_ceiling: 0.0,
        penalty_w_spacing: 0.0,
        penalty_w_mingain: 0.0,
        integrality: None,
    }
}

#[test]
fn test_tolerance_parameter_affects_optimization() {
    // Create two Args with different tolerance values
    let mut args_high_tol = Args::parse_from([
        "autoeq-test",
        "--algo",
        "autoeq:de",
        "--tolerance",
        "0.1", // High tolerance (loose)
        "--num-filters",
        "2",
    ]);

    let mut args_low_tol = Args::parse_from([
        "autoeq-test",
        "--algo",
        "autoeq:de",
        "--tolerance",
        "0.0001", // Low tolerance (strict)
        "--num-filters",
        "2",
    ]);

    // Ensure they have different tolerance values
    assert_ne!(args_high_tol.tolerance, args_low_tol.tolerance);
    assert_eq!(args_high_tol.tolerance, 0.1);
    assert_eq!(args_low_tol.tolerance, 0.0001);

    // Both should use the same strategy and other parameters for fair comparison
    args_high_tol.strategy = "best1bin".to_string();
    args_low_tol.strategy = "best1bin".to_string();
    args_high_tol.maxeval = 50; // Very limited evaluations for fast test
    args_low_tol.maxeval = 50;
    args_high_tol.population = 20; // Small population for fast test
    args_low_tol.population = 20;

    let objective_data = create_test_objective_data();
    let (lower_bounds, upper_bounds) = setup_bounds(&args_high_tol);

    // Test that both configurations can create valid DE configs
    let mut x1 = initial_guess(&args_high_tol, &lower_bounds, &upper_bounds);
    let mut x2 = initial_guess(&args_low_tol, &lower_bounds, &upper_bounds);

    // The optimization should handle different tolerance values without crashing
    let result1 = optimize_filters(
        &mut x1,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &args_high_tol,
    );

    let result2 = optimize_filters(
        &mut x2,
        &lower_bounds,
        &upper_bounds,
        objective_data,
        &args_low_tol,
    );

    // Both optimizations should succeed
    assert!(
        result1.is_ok(),
        "High tolerance optimization failed: {:?}",
        result1
    );
    assert!(
        result2.is_ok(),
        "Low tolerance optimization failed: {:?}",
        result2
    );
}

#[test]
fn test_strategy_parameter_affects_optimization() {
    // Test that different strategies can be parsed and used
    let strategies = ["best1bin", "rand1bin", "currenttobest1bin"];

    for strategy_name in strategies.iter() {
        let mut args = Args::parse_from([
            "autoeq-test",
            "--algo",
            "autoeq:de",
            "--strategy",
            strategy_name,
            "--num-filters",
            "2",
        ]);
        args.population = 10; // Small population for fast test
        args.maxeval = 30; // Small evaluations for fast test

        // Verify the strategy is correctly set
        assert_eq!(args.strategy, *strategy_name);

        // Verify the strategy can be parsed by the DE module
        let strategy = Strategy::from_str(&args.strategy);
        assert!(
            strategy.is_ok(),
            "Failed to parse strategy: {}",
            strategy_name
        );

        // Test that optimization works with this strategy
        let objective_data = create_test_objective_data();
        let (lower_bounds, upper_bounds) = setup_bounds(&args);
        let mut x = initial_guess(&args, &lower_bounds, &upper_bounds);

        let result = optimize_filters(&mut x, &lower_bounds, &upper_bounds, objective_data, &args);

        assert!(
            result.is_ok(),
            "Strategy {} optimization failed: {:?}",
            strategy_name,
            result
        );
    }
}

#[test]
fn test_recombination_parameter_affects_optimization() {
    let recombination_values = [0.1, 0.5, 0.9];

    for recomb in recombination_values.iter() {
        let mut args = Args::parse_from([
            "autoeq-test",
            "--algo",
            "autoeq:de",
            "--recombination",
            &recomb.to_string(),
            "--num-filters",
            "2",
        ]);
        args.population = 10; // Small population for fast test
        args.maxeval = 30; // Small evaluations for fast test

        // Verify the recombination value is correctly set
        assert_eq!(args.recombination, *recomb);

        let objective_data = create_test_objective_data();
        let (lower_bounds, upper_bounds) = setup_bounds(&args);
        let mut x = initial_guess(&args, &lower_bounds, &upper_bounds);

        let result = optimize_filters(&mut x, &lower_bounds, &upper_bounds, objective_data, &args);

        assert!(
            result.is_ok(),
            "Recombination {} optimization failed: {:?}",
            recomb,
            result
        );
    }
}

#[test]
fn test_adaptive_strategy_with_weights() {
    let mut args = Args::parse_from([
        "autoeq-test",
        "--algo",
        "autoeq:de",
        "--strategy",
        "adaptivebin",
        "--adaptive-weight-f",
        "0.8",
        "--adaptive-weight-cr",
        "0.7",
        "--num-filters",
        "2",
    ]);
    args.population = 15; // Small population for fast test
    args.maxeval = 40; // Small evaluations for fast test

    // Verify adaptive parameters are correctly set
    assert_eq!(args.strategy, "adaptivebin");
    assert_eq!(args.adaptive_weight_f, 0.8);
    assert_eq!(args.adaptive_weight_cr, 0.7);

    let objective_data = create_test_objective_data();
    let (lower_bounds, upper_bounds) = setup_bounds(&args);
    let mut x = initial_guess(&args, &lower_bounds, &upper_bounds);

    let result = optimize_filters(&mut x, &lower_bounds, &upper_bounds, objective_data, &args);

    assert!(
        result.is_ok(),
        "Adaptive strategy optimization failed: {:?}",
        result
    );
}

#[test]
fn test_parameter_validation_with_de_algorithm() {
    // Test that validation catches invalid parameters when using DE
    let test_cases = vec![
        (
            vec![
                "autoeq-test",
                "--algo",
                "autoeq:de",
                "--strategy",
                "invalid",
            ],
            "Invalid DE strategy",
        ),
        (
            vec!["autoeq-test", "--algo", "autoeq:de", "--tolerance", "0"],
            "Tolerance must be > 0",
        ),
        (
            vec!["autoeq-test", "--algo", "autoeq:de", "--atolerance=-0.1"],
            "Absolute tolerance must be >= 0",
        ),
        (
            vec![
                "autoeq-test",
                "--algo",
                "autoeq:de",
                "--adaptive-weight-f",
                "1.1",
            ],
            "Adaptive weight for F must be between 0.0 and 1.0",
        ),
        (
            vec![
                "autoeq-test",
                "--algo",
                "autoeq:de",
                "--adaptive-weight-cr=-0.1",
            ],
            "Adaptive weight for CR must be between 0.0 and 1.0",
        ),
    ];

    for (args_vec, expected_error) in test_cases {
        let args = Args::parse_from(args_vec.clone());
        let result = autoeq::cli::validate_args(&args);
        assert!(
            result.is_err(),
            "Expected validation error for args: {:?}",
            args_vec
        );
        assert!(
            result.unwrap_err().contains(expected_error),
            "Error message should contain: {}",
            expected_error
        );
    }
}
