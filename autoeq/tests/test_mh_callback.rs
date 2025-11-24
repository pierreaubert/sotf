//! Test metaheuristics callback support

use autoeq::LossType;
use autoeq::cli::{Args, PeqModel};
use autoeq::de::CallbackAction;
use autoeq::optim_mh::{MHIntermediate, create_mh_callback, optimize_filters_mh_with_callback};
use autoeq::workflow::{initial_guess, setup_bounds};
use clap::Parser;
use ndarray::Array1;
use std::sync::{Arc, Mutex};

/// Create a simple test objective data structure
fn create_test_objective_data() -> autoeq::optim::ObjectiveData {
    // Create simple test curves
    let freqs = Array1::from(vec![100.0, 1000.0, 10000.0]);
    let target = Array1::from(vec![1.0, 1.0, 1.0]);
    let deviation = Array1::from(vec![0.5, 0.5, 0.5]);

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
fn test_mh_callback_is_invoked() {
    // Create args with a metaheuristics algorithm
    let mut args = Args::parse_from([
        "autoeq-test",
        "--algo",
        "mh:pso",
        "--num-filters",
        "2",
        "--maxeval",
        "200", // Small for fast test
    ]);
    args.population = 20; // Small population

    let objective_data = create_test_objective_data();
    let (lower_bounds, upper_bounds) = setup_bounds(&args);
    let mut x = initial_guess(&args, &lower_bounds, &upper_bounds);

    // Track callback invocations
    let callback_count = Arc::new(Mutex::new(0));
    let callback_count_clone = Arc::clone(&callback_count);

    // Create callback that counts invocations
    let callback = Box::new(move |_intermediate: &MHIntermediate| -> CallbackAction {
        if let Ok(mut count) = callback_count_clone.lock() {
            *count += 1;
        }
        CallbackAction::Continue
    });

    // Run optimization with callback
    let result = optimize_filters_mh_with_callback(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data,
        "pso",
        args.population,
        args.maxeval,
        callback,
    );

    // Check that optimization succeeded
    assert!(result.is_ok(), "Optimization should succeed: {:?}", result);

    // Check that callback was invoked at least once
    let count = *callback_count.lock().unwrap();
    assert!(
        count > 0,
        "Callback should have been invoked at least once, got {} invocations",
        count
    );

    println!(
        "✅ Callback was invoked {} times during optimization",
        count
    );
}

#[test]
fn test_mh_callback_receives_progress_data() {
    let mut args = Args::parse_from([
        "autoeq-test",
        "--algo",
        "mh:de",
        "--num-filters",
        "2",
        "--maxeval",
        "200",
    ]);
    args.population = 15;

    let objective_data = create_test_objective_data();
    let (lower_bounds, upper_bounds) = setup_bounds(&args);
    let mut x = initial_guess(&args, &lower_bounds, &upper_bounds);

    // Track best fitness seen in callback
    let best_fitness = Arc::new(Mutex::new(f64::INFINITY));
    let best_fitness_clone = Arc::clone(&best_fitness);

    let callback = Box::new(move |intermediate: &MHIntermediate| -> CallbackAction {
        if let Ok(mut best) = best_fitness_clone.lock() {
            // Track best fitness
            if intermediate.fun < *best {
                *best = intermediate.fun;
            }

            // Verify data looks reasonable
            assert!(intermediate.iter > 0, "Iteration should be positive");
            assert!(intermediate.x.len() > 0, "Parameters should not be empty");
            assert!(intermediate.fun.is_finite(), "Fitness should be finite");
        }
        CallbackAction::Continue
    });

    let result = optimize_filters_mh_with_callback(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data,
        "de",
        args.population,
        args.maxeval,
        callback,
    );

    assert!(result.is_ok(), "Optimization should succeed");

    let final_best = *best_fitness.lock().unwrap();
    assert!(
        final_best < f64::INFINITY,
        "Should have recorded at least one fitness value"
    );

    println!("✅ Best fitness observed via callback: {:.6e}", final_best);
}

#[test]
fn test_default_mh_callback_works() {
    // Test that the default callback doesn't crash
    let mut callback = create_mh_callback("test_algo");

    let intermediate = MHIntermediate {
        x: Array1::from(vec![1.0, 2.0, 3.0]),
        fun: 0.5,
        iter: 10,
    };

    let result = callback(&intermediate);
    assert!(matches!(result, CallbackAction::Continue));

    println!("✅ Default callback works without crashing");
}
