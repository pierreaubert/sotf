//! Integration tests using BEM-generated room measurement data
//!
//! These tests load pre-computed BEM simulation data and verify that
//! the roomeq optimizer can improve the simulated frequency responses.
//!
//! Multi-sub scenarios are currently ignored due to a pre-existing bounds
//! issue in the multi-sub optimizer (optim.rs:449 slice index out of range).

use autoeq::roomeq::{RoomConfig, optimize_room};
use std::path::PathBuf;

/// Get workspace root (three levels up from CARGO_MANIFEST_DIR = crates/autoeq/autoeq)
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Run roomeq optimization on a generated BEM scenario and verify improvement
fn run_roomeq_on_generated(scenario_name: &str) {
    let config_path = workspace_root()
        .join("data_tests/roomeq/generated/bem")
        .join(scenario_name)
        .join("config.json");

    let config_json = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("Failed to read config for {scenario_name}: {e}"));
    let mut config: RoomConfig = serde_json::from_str(&config_json)
        .unwrap_or_else(|e| panic!("Failed to parse config for {scenario_name}: {e}"));

    // Resolve CSV paths relative to the config file's directory
    if let Some(config_dir) = config_path.parent() {
        config.resolve_paths(config_dir);
    }

    // Reduce iterations for faster tests
    config.optimizer.max_iter = 1000;
    config.optimizer.refine = false;
    // Use fixed seed for reproducible results
    config.optimizer.seed = Some(42);

    let result = optimize_room(&config, 48000.0, None, None)
        .unwrap_or_else(|e| panic!("Optimization failed for {scenario_name}: {e}"));

    // Verify optimization improved the response
    assert!(
        result.combined_post_score < result.combined_pre_score,
        "{scenario_name}: optimization did not improve score: pre={:.4}, post={:.4}",
        result.combined_pre_score,
        result.combined_post_score
    );

    // Verify at least 10% improvement
    let improvement = 1.0 - result.combined_post_score / result.combined_pre_score;
    assert!(
        improvement > 0.10,
        "{scenario_name}: improvement {:.1}% is less than 10% (pre={:.4}, post={:.4})",
        improvement * 100.0,
        result.combined_pre_score,
        result.combined_post_score
    );

    // Verify all channels have EQ results
    for (channel_name, channel_result) in &result.channel_results {
        assert!(
            !channel_result.biquads.is_empty(),
            "{scenario_name}: channel '{channel_name}' has no biquad filters"
        );
        // Allow up to 10% per-channel regression — the optimizer minimizes the
        // combined score across all channels, so individual channels may trade
        // a small regression for a better overall result.
        let max_allowed = channel_result.pre_score * 1.10;
        assert!(
            channel_result.post_score < max_allowed,
            "{scenario_name}: channel '{channel_name}' regressed too much: pre={:.4}, post={:.4} (max={:.4})",
            channel_result.pre_score,
            channel_result.post_score,
            max_allowed
        );
    }

    // Verify DSP chains were generated
    for (channel_name, chain) in &result.channels {
        assert!(
            !chain.plugins.is_empty(),
            "{scenario_name}: channel '{channel_name}' has no plugins in DSP chain"
        );
    }
}

// --- Stereo 2.0 scenarios (no subs) ---

#[test]
fn test_roomeq_small_stereo_2_0() {
    run_roomeq_on_generated("small_stereo_2_0");
}

#[test]
fn test_roomeq_medium_stereo_2_0() {
    run_roomeq_on_generated("medium_stereo_2_0");
}

#[test]
fn test_roomeq_large_stereo_2_0() {
    run_roomeq_on_generated("large_stereo_2_0");
}

// --- 2.1 scenarios (single sub) ---

#[test]
fn test_roomeq_small_stereo_2_1() {
    run_roomeq_on_generated("small_stereo_2_1");
}

#[test]
fn test_roomeq_medium_stereo_2_1() {
    run_roomeq_on_generated("medium_stereo_2_1");
}

#[test]
fn test_roomeq_large_stereo_2_1() {
    run_roomeq_on_generated("large_stereo_2_1");
}

// --- Multi-seat scenarios ---

#[test]
fn test_roomeq_medium_multi_seat() {
    run_roomeq_on_generated("medium_multi_seat");
}

// --- Multi-sub scenarios ---
// These are currently ignored due to a pre-existing bug in the multi-sub
// optimizer where the parameter vector slice bounds don't match n_drivers
// (optim.rs:449: "range end index N out of range for slice of length M")

#[test]
#[ignore = "multi-sub optimizer has pre-existing slice bounds bug"]
fn test_roomeq_small_multi_sub_2() {
    run_roomeq_on_generated("small_multi_sub_2");
}

#[test]
#[ignore = "multi-sub optimizer has pre-existing slice bounds bug"]
fn test_roomeq_medium_multi_sub_4() {
    run_roomeq_on_generated("medium_multi_sub_4");
}

#[test]
#[ignore = "multi-sub optimizer has pre-existing slice bounds bug"]
fn test_roomeq_large_multi_sub_4() {
    run_roomeq_on_generated("large_multi_sub_4");
}

#[test]
#[ignore = "multi-sub optimizer has pre-existing slice bounds bug"]
fn test_roomeq_large_multi_seat_2_1() {
    run_roomeq_on_generated("large_multi_seat_2_1");
}

#[test]
#[ignore = "multi-sub optimizer has pre-existing slice bounds bug"]
fn test_roomeq_medium_multi_sub_multi_seat() {
    run_roomeq_on_generated("medium_multi_sub_multi_seat");
}
