//! Tests for PEQ model bounds computation
//!
//! This module tests that each PEQ model correctly sets up bounds
//! for the filter parameters according to its specification.

use autoeq::cli::{Args, PeqModel};
use autoeq::workflow::setup_bounds;
use clap::Parser;

/// Helper to create test args with specific PEQ model
fn create_test_args(model: PeqModel, num_filters: usize) -> Args {
    let mut args = Args::parse_from(["test"]);
    args.peq_model = model;
    args.num_filters = num_filters;
    args.min_freq = 20.0;
    args.max_freq = 20000.0;
    args.min_q = 0.5;
    args.max_q = 10.0;
    args.min_db = 0.5;
    args.max_db = 6.0;
    args
}

#[test]
fn test_pk_model_bounds() {
    // All filters should be peak filters with normal bounds
    let args = create_test_args(PeqModel::Pk, 3);
    let (lower, upper) = setup_bounds(&args);

    // Check that all filters have gain bounds
    for i in 0..3 {
        let gain_idx = i * 3 + 2;
        assert!(
            lower[gain_idx] < 0.0,
            "Filter {} should have negative lower gain bound",
            i
        );
        assert!(
            upper[gain_idx] > 0.0,
            "Filter {} should have positive upper gain bound",
            i
        );
        assert_ne!(
            upper[gain_idx], 0.0,
            "Filter {} upper gain should not be zero (not constrained)",
            i
        );
    }
}

#[test]
fn test_hp_pk_model_bounds() {
    // First filter should be highpass (zero gain), rest peak
    let args = create_test_args(PeqModel::HpPk, 3);
    let (lower, upper) = setup_bounds(&args);

    // First filter (highpass)
    assert_eq!(lower[2], 0.0, "First filter (HP) lower gain should be 0");
    assert_eq!(upper[2], 0.0, "First filter (HP) upper gain should be 0");
    // HP should have restricted frequency range
    assert!(
        10f64.powf(upper[0]) <= 120.0,
        "HP frequency should be limited to low frequencies"
    );

    // Second and third filters (peak)
    for i in 1..3 {
        let gain_idx = i * 3 + 2;
        assert!(
            lower[gain_idx] < 0.0,
            "Peak filter {} should have negative lower gain bound",
            i
        );
        assert!(
            upper[gain_idx] > 0.0,
            "Peak filter {} should have positive upper gain bound",
            i
        );
    }
}

#[test]
fn test_hp_pk_lp_model_bounds() {
    // First filter highpass, last lowpass, middle peak
    let args = create_test_args(PeqModel::HpPkLp, 4);
    let (lower, upper) = setup_bounds(&args);

    // First filter (highpass)
    assert_eq!(lower[2], 0.0, "First filter (HP) lower gain should be 0");
    assert_eq!(upper[2], 0.0, "First filter (HP) upper gain should be 0");
    assert!(
        10f64.powf(upper[0]) <= 120.0,
        "HP frequency should be limited to low frequencies"
    );

    // Middle filters (peak)
    for i in 1..3 {
        let gain_idx = i * 3 + 2;
        assert!(
            lower[gain_idx] < 0.0,
            "Peak filter {} should have negative lower gain bound",
            i
        );
        assert!(
            upper[gain_idx] > 0.0,
            "Peak filter {} should have positive upper gain bound",
            i
        );
    }

    // Last filter (lowpass)
    let last_idx = 3;
    assert_eq!(
        lower[last_idx * 3 + 2],
        0.0,
        "Last filter (LP) lower gain should be 0"
    );
    assert_eq!(
        upper[last_idx * 3 + 2],
        0.0,
        "Last filter (LP) upper gain should be 0"
    );
    assert!(
        10f64.powf(lower[last_idx * 3]) >= 5000.0,
        "LP frequency should be limited to high frequencies"
    );
}

#[test]
fn test_hp_pk_lp_model_with_insufficient_filters() {
    // With only 1 filter, hp-pk-lp should degrade gracefully
    let args = create_test_args(PeqModel::HpPkLp, 1);
    let (lower, upper) = setup_bounds(&args);

    // Should still apply HP constraints to the single filter
    assert_eq!(lower[2], 0.0, "Single filter (HP) lower gain should be 0");
    assert_eq!(upper[2], 0.0, "Single filter (HP) upper gain should be 0");
}

#[test]
fn test_frequency_bounds_progression() {
    // Test that frequency bounds progress correctly (no backward movement)
    let args = create_test_args(PeqModel::Pk, 5);
    let (lower, _upper) = setup_bounds(&args);

    for i in 1..5 {
        let prev_lower = lower[(i - 1) * 3];
        let curr_lower = lower[i * 3];
        assert!(
            curr_lower >= prev_lower,
            "Filter {} lower freq bound should be >= previous filter's",
            i
        );
    }
}

#[test]
fn test_q_bounds_consistency() {
    // Test that Q bounds are consistent across all models
    for model in PeqModel::all() {
        let args = create_test_args(model, 3);
        let (lower, upper) = setup_bounds(&args);

        for i in 0..3 {
            // Calculate Q index based on model (3 vs 4 parameters per filter)
            let (q_lower_idx, q_upper_idx) = match model {
                PeqModel::FreePkFree | PeqModel::Free => {
                    // Free models: [type, freq, Q, gain]
                    (i * 4 + 2, i * 4 + 2)
                }
                _ => {
                    // Fixed models: [freq, Q, gain]
                    (i * 3 + 1, i * 3 + 1)
                }
            };

            // Q bounds should respect the args unless it's a special filter
            if model == PeqModel::HpPk && i == 0 {
                // HP filter has special Q bounds
                assert_eq!(
                    lower[q_lower_idx], 1.0,
                    "HP filter Q lower bound should be 1.0"
                );
                assert_eq!(
                    upper[q_upper_idx], 1.5,
                    "HP filter Q upper bound should be 1.5"
                );
            } else if model == PeqModel::HpPkLp && (i == 0 || i == 2) {
                // HP/LP filters have special Q bounds
                assert_eq!(
                    lower[q_lower_idx], 1.0,
                    "HP/LP filter Q lower bound should be 1.0"
                );
                assert_eq!(
                    upper[q_upper_idx], 1.5,
                    "HP/LP filter Q upper bound should be 1.5"
                );
            } else {
                // Normal peak filters
                assert!(
                    lower[q_lower_idx] >= args.min_q.max(0.1),
                    "Q lower bound should respect min_q for model {:?}",
                    model
                );
                assert_eq!(
                    upper[q_upper_idx], args.max_q,
                    "Q upper bound should equal max_q for model {:?}",
                    model
                );
            }
        }
    }
}

#[test]
fn test_gain_bounds_for_special_filters() {
    // Test that highpass and lowpass filters have zero gain
    let test_cases = vec![
        (PeqModel::HpPk, 3, vec![0]),      // First filter is HP
        (PeqModel::HpPkLp, 3, vec![0, 2]), // First is HP, last is LP
    ];

    for (model, num_filters, zero_gain_indices) in test_cases {
        let args = create_test_args(model, num_filters);
        let (lower, upper) = setup_bounds(&args);

        for i in 0..num_filters {
            // Calculate gain index based on model (3 vs 4 parameters per filter)
            let gain_idx = match model {
                PeqModel::FreePkFree | PeqModel::Free => i * 4 + 3, // [type, freq, Q, gain]
                _ => i * 3 + 2,                                     // [freq, Q, gain]
            };
            if zero_gain_indices.contains(&i) {
                assert_eq!(
                    lower[gain_idx], 0.0,
                    "Filter {} should have zero lower gain for model {:?}",
                    i, model
                );
                assert_eq!(
                    upper[gain_idx], 0.0,
                    "Filter {} should have zero upper gain for model {:?}",
                    i, model
                );
            } else {
                assert_ne!(
                    upper[gain_idx], 0.0,
                    "Filter {} should have non-zero upper gain for model {:?}",
                    i, model
                );
            }
        }
    }
}

#[test]
fn test_model_effective_peq_model() {
    // Test that effective_peq_model returns the correct model
    let mut args = Args::parse_from(["test"]);

    // Default should be Pk
    assert_eq!(args.effective_peq_model(), PeqModel::Pk);

    // Setting peq_model directly
    args.peq_model = PeqModel::HpPkLp;
    assert_eq!(args.effective_peq_model(), PeqModel::HpPkLp);

    // Test various models
    args.peq_model = PeqModel::HpPk;
    assert_eq!(args.effective_peq_model(), PeqModel::HpPk);
}

#[test]
fn test_uses_highpass_first() {
    let mut args = Args::parse_from(["test"]);

    // Pk model should not use highpass
    args.peq_model = PeqModel::Pk;
    assert!(!args.uses_highpass_first());

    // HpPk model should use highpass
    args.peq_model = PeqModel::HpPk;
    assert!(args.uses_highpass_first());

    // HpPkLp model should use highpass
    args.peq_model = PeqModel::HpPkLp;
    assert!(args.uses_highpass_first());

    // FreePkFree should not use highpass (it's "free")
    args.peq_model = PeqModel::FreePkFree;
    assert!(!args.uses_highpass_first());

    // Free should not use highpass
    args.peq_model = PeqModel::Free;
    assert!(!args.uses_highpass_first());
}
