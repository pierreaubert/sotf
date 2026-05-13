//! Phase 4 option-precedence regressions.
//!
//! Pins the validator-level warning that makes overlapping options
//! explicit instead of silently letting one override the other:
//!
//! * **I1** — `target_curve` + non-flat `target_response` both set:
//!   `target_response` is baked into the measurement, `target_curve` is
//!   dropped. Validator must warn.

use autoeq::MeasurementSource;
use autoeq::roomeq::{
    OptimizerConfig, RoomConfig, SpeakerConfig, TargetCurveConfig, TargetResponseConfig,
    TargetShape, UserPreference, default_config_version, validate_room_config,
};
use autoeq::{MeasurementRef, MeasurementSingle};
use std::collections::HashMap;
use std::path::PathBuf;

fn single_speaker_config(
    optimizer: OptimizerConfig,
    target_curve: Option<TargetCurveConfig>,
) -> RoomConfig {
    let mut speakers = HashMap::new();
    speakers.insert(
        "left".to_string(),
        SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
            measurement: MeasurementRef::Path(PathBuf::from("left.csv")),
            speaker_name: None,
        })),
    );
    RoomConfig {
        version: default_config_version(),
        system: None,
        speakers,
        crossovers: None,
        target_curve,
        optimizer,
        recording_config: None,
        ctc: None,
        cea2034_cache: None,
    }
}

// ============================================================================
// I1 — target_curve + target_response collision
// ============================================================================

#[test]
fn i1_target_curve_plus_target_response_warns() {
    let opt = OptimizerConfig {
        target_response: Some(TargetResponseConfig {
            shape: TargetShape::Harman,
            slope_db_per_octave: -0.8,
            reference_freq: 1000.0,
            curve_path: None,
            preference: UserPreference::default(),
            broadband_precorrection: false,
            role_targets: None,
        }),
        ..Default::default()
    };

    let config = single_speaker_config(
        opt,
        Some(TargetCurveConfig::Predefined("harman".to_string())),
    );
    let result = validate_room_config(&config);

    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("target_curve") && w.contains("target_response")),
        "expected target_curve + target_response warning, got: {:?}",
        result.warnings
    );
}

#[test]
fn i1_target_curve_plus_flat_target_response_silent() {
    // A default/Flat target_response carries no information — don't warn.
    let opt = OptimizerConfig {
        target_response: Some(TargetResponseConfig::default()),
        ..Default::default()
    };

    let config =
        single_speaker_config(opt, Some(TargetCurveConfig::Predefined("flat".to_string())));
    let result = validate_room_config(&config);

    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.contains("target_response takes precedence")),
        "flat target_response should not trigger the precedence warning: {:?}",
        result.warnings
    );
}
