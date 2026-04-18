//! Pin the invariant that `OptimizerConfig::migrate_target_config` is idempotent.
//!
//! Several entry points now call `migrate_target_config` up-front (`optimize_room`,
//! `optimize_speaker`, JSON loading). If any one of them is composed with another
//! that also migrates — or if a user runs migration explicitly before calling a
//! public API — the second call must be a no-op. Otherwise the legacy fields
//! would re-materialise or `target_response` would be overwritten.

use autoeq::roomeq::{
    OptimizerConfig, TargetResponseConfig, TargetShape, TargetTiltConfig, TiltType,
    UserPreference,
};

fn base_config() -> OptimizerConfig {
    OptimizerConfig::default()
}

#[test]
fn migrate_is_noop_on_default_config() {
    let mut cfg = base_config();
    let before = serde_json::to_value(&cfg).unwrap();
    cfg.migrate_target_config();
    let after = serde_json::to_value(&cfg).unwrap();
    assert_eq!(
        before, after,
        "migrate_target_config on a default OptimizerConfig must not change anything",
    );
}

#[test]
fn migrate_is_noop_when_target_response_already_set() {
    let mut cfg = base_config();
    cfg.target_response = Some(TargetResponseConfig {
        shape: TargetShape::Harman,
        slope_db_per_octave: -0.8,
        reference_freq: 1000.0,
        curve_path: None,
        preference: UserPreference {
            bass_shelf_db: 3.0,
            bass_shelf_freq: 120.0,
            treble_shelf_db: 0.0,
            treble_shelf_freq: 8000.0,
        },
        broadband_precorrection: true,
    });

    cfg.migrate_target_config();
    let after_first = serde_json::to_value(&cfg).unwrap();

    cfg.migrate_target_config();
    let after_second = serde_json::to_value(&cfg).unwrap();

    assert_eq!(
        after_first, after_second,
        "second migrate_target_config call must be a no-op once target_response is set",
    );
    assert!(
        cfg.target_tilt.is_none(),
        "legacy target_tilt must remain cleared",
    );
    assert!(
        cfg.broadband_target_matching.is_none(),
        "legacy broadband_target_matching must remain cleared",
    );
}

#[test]
fn migrate_then_migrate_again_folds_legacy_fields() {
    // Fresh legacy config: only `target_tilt` + `broadband_target_matching` set.
    let mut cfg = base_config();
    cfg.target_tilt = Some(TargetTiltConfig {
        tilt_type: TiltType::Custom,
        slope_db_per_octave: -1.2,
        reference_freq: 1000.0,
        bass_shelf_db: 4.0,
        bass_shelf_freq: 120.0,
    });
    cfg.broadband_target_matching =
        Some(autoeq::roomeq::BroadbandTargetMatchingConfig { enabled: true });

    // First migration folds legacy → target_response.
    cfg.migrate_target_config();

    let after_first = serde_json::to_value(&cfg).unwrap();
    assert!(cfg.target_response.is_some(), "target_response populated");
    assert!(cfg.target_tilt.is_none(), "target_tilt cleared");
    assert!(
        cfg.broadband_target_matching.is_none(),
        "broadband_target_matching cleared",
    );

    // Second migration must be a no-op: target_response is already set, the
    // legacy fields are None, so the early-return path fires.
    cfg.migrate_target_config();
    let after_second = serde_json::to_value(&cfg).unwrap();
    assert_eq!(
        after_first, after_second,
        "migrate_target_config must be idempotent across legacy-fold and re-invocation",
    );

    // Sanity: the migrated content carries the custom slope.
    let tr = cfg.target_response.as_ref().unwrap();
    assert_eq!(tr.shape, TargetShape::Custom);
    assert!((tr.slope_db_per_octave - (-1.2)).abs() < 1e-9);
    assert!(tr.broadband_precorrection);
    assert!((tr.preference.bass_shelf_db - 4.0).abs() < 1e-9);
}
