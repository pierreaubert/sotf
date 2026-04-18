//! Phase 4 option-precedence regressions.
//!
//! Pins the validator-level warnings that make overlapping options
//! explicit instead of silently letting one override the other:
//!
//! * **I1** — `target_curve` + non-flat `target_response` both set:
//!   `target_response` is baked into the measurement, `target_curve` is
//!   dropped. Validator must warn.
//! * **I1 legacy** — `target_curve` + non-flat `target_tilt`: pre-migration
//!   variant of the same collision.
//! * **B4** — legacy `mode` string disagreeing with `processing_mode`:
//!   validator emits a deprecation warning steering users toward
//!   `processing_mode`.

use autoeq::MeasurementSource;
use autoeq::roomeq::{
    OptimizerConfig, ProcessingMode, RoomConfig, SpeakerConfig, TargetCurveConfig,
    TargetResponseConfig, TargetShape, TargetTiltConfig, TiltType, UserPreference,
    default_config_version, validate_room_config,
};
use autoeq::{MeasurementRef, MeasurementSingle};
use std::collections::HashMap;
use std::path::PathBuf;

fn single_speaker_config(optimizer: OptimizerConfig, target_curve: Option<TargetCurveConfig>) -> RoomConfig {
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
        cea2034_cache: None,
    }
}

// ============================================================================
// I1 — target_curve + target_response collision
// ============================================================================

#[test]
fn i1_target_curve_plus_target_response_warns() {
    let mut opt = OptimizerConfig::default();
    opt.target_response = Some(TargetResponseConfig {
        shape: TargetShape::Harman,
        slope_db_per_octave: -0.8,
        reference_freq: 1000.0,
        curve_path: None,
        preference: UserPreference::default(),
        broadband_precorrection: false,
    });

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
    let mut opt = OptimizerConfig::default();
    opt.target_response = Some(TargetResponseConfig::default());

    let config = single_speaker_config(
        opt,
        Some(TargetCurveConfig::Predefined("flat".to_string())),
    );
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

#[test]
fn i1_target_curve_plus_legacy_target_tilt_warns() {
    // Legacy variant still caught pre-migration. Preserves the long-
    // standing warning for older configs that skip migrate_target_config.
    let mut opt = OptimizerConfig::default();
    opt.target_tilt = Some(TargetTiltConfig {
        tilt_type: TiltType::Harman,
        slope_db_per_octave: -0.8,
        reference_freq: 1000.0,
        bass_shelf_db: 0.0,
        bass_shelf_freq: 200.0,
    });

    let config = single_speaker_config(
        opt,
        Some(TargetCurveConfig::Predefined("harman".to_string())),
    );
    let result = validate_room_config(&config);

    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("target_curve") && w.contains("target_tilt")),
        "expected target_curve + target_tilt legacy warning, got: {:?}",
        result.warnings
    );
}

// ============================================================================
// B4 — legacy `mode` string deprecation
// ============================================================================

#[test]
fn b4_mode_iir_with_processing_mode_low_latency_silent() {
    // Default alignment: mode="iir" + ProcessingMode::LowLatency.
    let opt = OptimizerConfig::default();
    assert_eq!(opt.mode, "iir");
    assert_eq!(opt.processing_mode, ProcessingMode::LowLatency);

    let config = single_speaker_config(opt, None);
    let result = validate_room_config(&config);
    assert!(
        !result.warnings.iter().any(|w| w.contains("Legacy `mode`")),
        "aligned default config should not warn about legacy mode: {:?}",
        result.warnings
    );
}

#[test]
fn b4_mode_iir_with_processing_mode_phase_linear_warns() {
    // User set processing_mode to PhaseLinear but left mode at default
    // "iir" — misalignment. Should deprecate-warn.
    let mut opt = OptimizerConfig::default();
    opt.processing_mode = ProcessingMode::PhaseLinear;
    // mode stays at "iir"

    let config = single_speaker_config(opt, None);
    let result = validate_room_config(&config);
    assert!(
        result.warnings.iter().any(|w| w.contains("Legacy `mode`")),
        "mismatched mode/processing_mode should deprecate-warn: {:?}",
        result.warnings
    );
}

#[test]
fn b4_warped_iir_with_default_mode_silent() {
    // WarpedIir has no legacy equivalent; mode="iir" is the safe default
    // and must not warn. (Change mode to anything else → warn.)
    let mut opt = OptimizerConfig::default();
    opt.processing_mode = ProcessingMode::WarpedIir;
    assert_eq!(opt.mode, "iir");

    let config = single_speaker_config(opt, None);
    let result = validate_room_config(&config);
    assert!(
        !result.warnings.iter().any(|w| w.contains("Legacy `mode`")),
        "WarpedIir + default mode should stay silent: {:?}",
        result.warnings
    );
}

#[test]
fn b4_warped_iir_with_fir_mode_warns() {
    let mut opt = OptimizerConfig::default();
    opt.processing_mode = ProcessingMode::WarpedIir;
    opt.mode = "fir".to_string();

    let config = single_speaker_config(opt, None);
    let result = validate_room_config(&config);
    assert!(
        result.warnings.iter().any(|w| w.contains("no equivalent")),
        "WarpedIir + non-default mode should warn: {:?}",
        result.warnings
    );
}

#[test]
fn b4_mode_mixed_with_hybrid_silent() {
    let mut opt = OptimizerConfig::default();
    opt.processing_mode = ProcessingMode::Hybrid;
    opt.mode = "mixed".to_string();

    let config = single_speaker_config(opt, None);
    let result = validate_room_config(&config);
    assert!(
        !result.warnings.iter().any(|w| w.contains("Legacy `mode`")),
        "aligned mode=mixed/Hybrid should not warn: {:?}",
        result.warnings
    );
}
