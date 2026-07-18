//! End-to-end test for the autoeq room-EQ optimizer using synthetic data.
//!
//! This test builds a minimal `autoeq::RoomConfig` from in-memory frequency
//! response curves, runs the optimizer, and verifies that the returned DSP
//! graph contains valid input and final curves for every channel.

use std::collections::HashMap;

use ndarray::Array1;
use sotf_audio_player::autoeq::{MeasurementSource, RoomConfig, SpeakerConfig, optimize_room};
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

    for (name, chain) in &result.channels {
        assert_eq!(&chain.channel, name, "channel identity must be preserved");
        assert!(
            chain.initial_curve.is_some(),
            "channel '{name}' needs an input curve"
        );
        assert!(
            chain.final_curve.is_some(),
            "channel '{name}' needs a final curve"
        );
    }
}
