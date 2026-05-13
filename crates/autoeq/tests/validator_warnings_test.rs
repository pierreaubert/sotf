//! Tests for new Phase 2 validator warnings:
//!
//! - **I2** schroeder_split + non-zero target slope → warning
//! - **I5** `ProcessingMode::PhaseLinear` + `max_freq > 2000 Hz` → warning
//! - **B10** `multi_measurement.weights.len()` mismatch vs speaker measurements → error
//! - **I4** `cea2034_correction.enabled` without a CEA2034/spinorama source → warning

use autoeq::roomeq::{
    Cea2034CorrectionConfig, MultiMeasurementConfig, MultiMeasurementStrategy, OptimizerConfig,
    ProcessingMode, RoomConfig, SchroederSplitConfig, SpeakerConfig, SpeakerGroup,
    TargetResponseConfig, TargetShape, default_config_version, validate_room_config,
};
use autoeq::{MeasurementMultiple, MeasurementRef, MeasurementSingle, MeasurementSource};
use std::collections::HashMap;
use std::path::PathBuf;

fn single_speaker(path: &str, speaker_name: Option<&str>) -> SpeakerConfig {
    SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
        measurement: MeasurementRef::Path(PathBuf::from(path)),
        speaker_name: speaker_name.map(str::to_string),
    }))
}

fn multi_speaker(paths: &[&str]) -> SpeakerConfig {
    SpeakerConfig::Single(MeasurementSource::Multiple(MeasurementMultiple {
        measurements: paths
            .iter()
            .map(|p| MeasurementRef::Path(PathBuf::from(p)))
            .collect(),
        speaker_name: None,
    }))
}

fn base_config(speakers: HashMap<String, SpeakerConfig>, optimizer: OptimizerConfig) -> RoomConfig {
    RoomConfig {
        version: default_config_version(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer,
        recording_config: None,
        ctc: None,
        cea2034_cache: None,
    }
}

fn one_speaker(name: &str, speaker: SpeakerConfig) -> HashMap<String, SpeakerConfig> {
    let mut m = HashMap::new();
    m.insert(name.to_string(), speaker);
    m
}

// ============================================================================
// I2 — schroeder_split + non-zero target slope emits a warning
// ============================================================================

#[test]
fn i2_schroeder_split_with_target_response_slope_warns() {
    let opt = OptimizerConfig {
        schroeder_split: Some(SchroederSplitConfig {
            enabled: true,
            ..SchroederSplitConfig::default()
        }),
        target_response: Some(TargetResponseConfig {
            shape: TargetShape::Custom,
            slope_db_per_octave: -0.8,
            ..TargetResponseConfig::default()
        }),
        ..Default::default()
    };

    let config = base_config(one_speaker("L", single_speaker("l.csv", None)), opt);
    let result = validate_room_config(&config);

    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("schroeder_split")),
        "expected schroeder_split warning, got: {:?}",
        result.warnings
    );
}

#[test]
fn i2_schroeder_split_with_flat_slope_no_warning() {
    // If both legs are flat, there's no slope to approximate, so no warning.
    let opt = OptimizerConfig {
        schroeder_split: Some(SchroederSplitConfig {
            enabled: true,
            ..SchroederSplitConfig::default()
        }),
        // Default target_response is absent.
        ..Default::default()
    };

    let config = base_config(one_speaker("L", single_speaker("l.csv", None)), opt);
    let result = validate_room_config(&config);

    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.contains("schroeder_split")),
        "unexpected schroeder_split warning on flat config: {:?}",
        result.warnings
    );
}

// ============================================================================
// I5 — PhaseLinear + max_freq > 2000 Hz emits a warning
// ============================================================================

#[test]
fn i5_phase_linear_wide_band_warns() {
    let opt = OptimizerConfig {
        processing_mode: ProcessingMode::PhaseLinear,
        max_freq: 20000.0,
        ..Default::default()
    };

    let config = base_config(one_speaker("L", single_speaker("l.csv", None)), opt);
    let result = validate_room_config(&config);

    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("phase_linear") && w.contains("max_freq")),
        "expected phase_linear + max_freq warning, got: {:?}",
        result.warnings
    );
}

#[test]
fn i5_phase_linear_bass_only_no_warning() {
    let opt = OptimizerConfig {
        processing_mode: ProcessingMode::PhaseLinear,
        max_freq: 1500.0,
        ..Default::default()
    };

    let config = base_config(one_speaker("L", single_speaker("l.csv", None)), opt);
    let result = validate_room_config(&config);

    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.contains("phase_linear") && w.contains("max_freq")),
        "unexpected warning for bass-only PhaseLinear: {:?}",
        result.warnings
    );
}

#[test]
fn i5_low_latency_mode_no_warning_even_at_20khz() {
    let opt = OptimizerConfig {
        processing_mode: ProcessingMode::LowLatency,
        max_freq: 20000.0,
        ..Default::default()
    };

    let config = base_config(one_speaker("L", single_speaker("l.csv", None)), opt);
    let result = validate_room_config(&config);

    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.contains("phase_linear") && w.contains("max_freq")),
        "unexpected PhaseLinear warning on LowLatency: {:?}",
        result.warnings
    );
}

// ============================================================================
// B10 — multi_measurement.weights length must match measurement count
// ============================================================================

#[test]
fn b10_weights_length_mismatch_is_error() {
    let opt = OptimizerConfig {
        multi_measurement: Some(MultiMeasurementConfig {
            strategy: MultiMeasurementStrategy::WeightedSum,
            weights: Some(vec![0.5, 0.5]), // 2 weights
            ..MultiMeasurementConfig::default()
        }),
        ..Default::default()
    };

    let speakers = one_speaker(
        "L",
        multi_speaker(&["m1.csv", "m2.csv", "m3.csv"]), // 3 measurements
    );
    let config = base_config(speakers, opt);
    let result = validate_room_config(&config);

    assert!(
        !result.is_valid,
        "config should be invalid: errors={:?}, warnings={:?}",
        result.errors, result.warnings
    );
    assert!(
        result
            .errors
            .iter()
            .any(|e| e.contains("multi_measurement.weights")),
        "expected weights-mismatch error, got: {:?}",
        result.errors
    );
}

#[test]
fn b10_weights_length_match_no_error() {
    let opt = OptimizerConfig {
        multi_measurement: Some(MultiMeasurementConfig {
            strategy: MultiMeasurementStrategy::WeightedSum,
            weights: Some(vec![0.4, 0.3, 0.3]),
            ..MultiMeasurementConfig::default()
        }),
        ..Default::default()
    };

    let speakers = one_speaker("L", multi_speaker(&["m1.csv", "m2.csv", "m3.csv"]));
    let config = base_config(speakers, opt);
    let result = validate_room_config(&config);

    assert!(
        !result
            .errors
            .iter()
            .any(|e| e.contains("multi_measurement.weights")),
        "unexpected mismatch error on matching lengths: {:?}",
        result.errors
    );
}

#[test]
fn b10_single_measurement_source_ignored() {
    // A Single source doesn't have a count to compare against; no error even
    // if weights is populated (it's harmless until the channel actually has
    // multiple measurements).
    let opt = OptimizerConfig {
        multi_measurement: Some(MultiMeasurementConfig {
            strategy: MultiMeasurementStrategy::WeightedSum,
            weights: Some(vec![0.5, 0.5]),
            ..MultiMeasurementConfig::default()
        }),
        ..Default::default()
    };

    let config = base_config(one_speaker("L", single_speaker("l.csv", None)), opt);
    let result = validate_room_config(&config);

    assert!(
        !result
            .errors
            .iter()
            .any(|e| e.contains("multi_measurement.weights")),
        "single source should not trigger weights mismatch: {:?}",
        result.errors
    );
}

// ============================================================================
// I4 — cea2034_correction requires a CEA2034/spinorama-shaped source
// ============================================================================

#[test]
fn i4_cea2034_without_spinorama_source_warns() {
    let opt = OptimizerConfig {
        cea2034_correction: Some(Cea2034CorrectionConfig {
            enabled: true,
            ..Cea2034CorrectionConfig::default()
        }),
        ..Default::default()
    };

    let config = base_config(
        one_speaker("L", single_speaker("plain_room_measurement.csv", None)),
        opt,
    );
    let result = validate_room_config(&config);

    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("cea2034_correction")),
        "expected cea2034 source warning, got: {:?}",
        result.warnings
    );
}

#[test]
fn i4_cea2034_with_speaker_name_no_warning() {
    let opt = OptimizerConfig {
        cea2034_correction: Some(Cea2034CorrectionConfig {
            enabled: true,
            ..Cea2034CorrectionConfig::default()
        }),
        ..Default::default()
    };

    let config = base_config(
        one_speaker("L", single_speaker("l.csv", Some("KEF R3"))),
        opt,
    );
    let result = validate_room_config(&config);

    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.contains("cea2034_correction")),
        "unexpected cea2034 warning when speaker_name is set: {:?}",
        result.warnings
    );
}

#[test]
fn i4_cea2034_with_path_hint_no_warning() {
    let opt = OptimizerConfig {
        cea2034_correction: Some(Cea2034CorrectionConfig {
            enabled: true,
            ..Cea2034CorrectionConfig::default()
        }),
        ..Default::default()
    };

    let config = base_config(
        one_speaker("L", single_speaker("speakers/KEF_R3_cea2034_asr.csv", None)),
        opt,
    );
    let result = validate_room_config(&config);

    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.contains("cea2034_correction")),
        "unexpected cea2034 warning when path contains 'cea2034': {:?}",
        result.warnings
    );
}

#[test]
fn i4_cea2034_disabled_no_warning() {
    let opt = OptimizerConfig {
        cea2034_correction: Some(Cea2034CorrectionConfig {
            enabled: false,
            ..Cea2034CorrectionConfig::default()
        }),
        ..Default::default()
    };

    let config = base_config(one_speaker("L", single_speaker("plain.csv", None)), opt);
    let result = validate_room_config(&config);

    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.contains("cea2034_correction")),
        "disabled cea2034 should not warn: {:?}",
        result.warnings
    );
}

// ============================================================================
// SpeakerGroup variant: B10 + I4 should still kick in on groups
// ============================================================================

#[test]
fn b10_weights_mismatch_inside_speaker_group() {
    let opt = OptimizerConfig {
        multi_measurement: Some(MultiMeasurementConfig {
            strategy: MultiMeasurementStrategy::WeightedSum,
            weights: Some(vec![0.5, 0.5]),
            ..MultiMeasurementConfig::default()
        }),
        ..Default::default()
    };

    let group = SpeakerConfig::Group(SpeakerGroup {
        name: "mains".to_string(),
        speaker_name: None,
        measurements: vec![MeasurementSource::Multiple(MeasurementMultiple {
            measurements: vec![
                MeasurementRef::Path(PathBuf::from("a.csv")),
                MeasurementRef::Path(PathBuf::from("b.csv")),
                MeasurementRef::Path(PathBuf::from("c.csv")),
            ],
            speaker_name: None,
        })],
        crossover: None,
    });
    let speakers = one_speaker("mains", group);
    let config = base_config(speakers, opt);
    let result = validate_room_config(&config);

    assert!(
        result
            .errors
            .iter()
            .any(|e| e.contains("multi_measurement.weights")),
        "expected weights error on group-wrapped Multiple, got: {:?}",
        result.errors
    );
}
