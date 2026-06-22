//! End-to-end test for the autoeq room-EQ optimizer using synthetic data.
//!
//! This test builds a minimal `autoeq::RoomConfig` from in-memory frequency
//! response curves, runs the optimizer, and verifies that every returned
//! channel contains at least one EQ or broadband filter.

use std::collections::HashMap;

use ndarray::Array1;
use sotf_audio_player::autoeq::{
    MeasurementSource, RoomConfig, RoomOptimizationResult, SpeakerConfig, optimize_room,
};
use sotf_audio_player::room_eq_types::RoomEqOptimizerConfig;

/// Build a synthetic measurement curve: flat response with a +3 dB bump at 1 kHz.
fn synthetic_bump_curve() -> autoeq::Curve {
    let n = 100;
    let f_min = 20.0_f64;
    let f_max = 16_000.0_f64;
    let log_step = (f_max / f_min).ln() / (n as f64 - 1.0);

    let mut freq = Vec::with_capacity(n);
    let mut spl = Vec::with_capacity(n);
    for i in 0..n {
        let f = f_min * (i as f64 * log_step).exp();
        let bump = if (f - 1_000.0).abs() < 150.0 {
            3.0
        } else {
            0.0
        };
        freq.push(f);
        spl.push(bump);
    }

    autoeq::Curve {
        freq: Array1::from_vec(freq),
        spl: Array1::from_vec(spl),
        ..Default::default()
    }
}

/// Build a minimal room configuration with one or two synthetic channels.
fn build_room_config() -> RoomConfig {
    let mut speakers = HashMap::new();
    speakers.insert(
        "L".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(synthetic_bump_curve())),
    );
    speakers.insert(
        "R".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(synthetic_bump_curve())),
    );

    // Use the UI-facing default config and convert it to the backend type so
    // the test exercises the same code path as the TUI/GPUI apps.
    // Keep the run fast and deterministic; the synthetic bump is simple enough
    // that a small budget still produces a correction filter.
    let ui_config = RoomEqOptimizerConfig {
        max_iter: 500,
        population: 50,
        refine: false,
        seed: Some(42),
        ..Default::default()
    };

    RoomConfig {
        version: "2.0.0".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: ui_config.to_optimizer_config(),
        recording_config: None,
        ctc: None,
        cea2034_cache: None,
    }
}

/// Count the number of EQ/broadband filters present in a channel DSP chain.
fn count_channel_filters(result: &RoomOptimizationResult) -> HashMap<String, usize> {
    result
        .channels
        .iter()
        .map(|(name, chain)| {
            let mut count = 0usize;
            for plugin in &chain.plugins {
                if plugin.plugin_type == "eq" {
                    let is_broadband = plugin.parameters.get("label").and_then(|v| v.as_str())
                        == Some("broadband");
                    let filter_count = plugin
                        .parameters
                        .get("filters")
                        .and_then(|v| v.as_array())
                        .map_or(0, |a| a.len());
                    // Broadband filters are still biquad-based EQ corrections,
                    // so they satisfy the "at least one EQ or broadband filter"
                    // requirement.
                    if is_broadband || filter_count > 0 {
                        count += filter_count.max(1);
                    }
                }
            }
            (name.clone(), count)
        })
        .collect()
}

#[test]
fn room_eq_optimizer_produces_non_empty_channel_dsp_graph() {
    let config = build_room_config();
    let temp_dir = tempfile::tempdir().expect("create temp dir for artifacts");

    let result = optimize_room(&config, 48_000.0, None, Some(temp_dir.path()))
        .expect("room optimizer should not fail on synthetic data");

    assert!(
        !result.channels.is_empty(),
        "optimizer must return at least one channel"
    );

    let filter_counts = count_channel_filters(&result);
    for (name, count) in &filter_counts {
        assert!(
            *count > 0,
            "channel '{name}' must contain at least one EQ or broadband filter, got {count}"
        );
    }
}
