//! Pin the invariant behind B1: when `channel_matching` is omitted from
//! `OptimizerConfig`, `compute_and_correct_icd` must treat it as
//! `ChannelMatchingConfig::default()` — same `enabled`, `threshold_db`, and
//! `max_filters` as if the user had passed `Some(ChannelMatchingConfig::default())`.
//!
//! Before the fix, the fallback hard-coded `enabled=false`, `threshold_db=1.5`,
//! `max_filters=3` — all three diverging from the public default. That meant
//! `channel_matching: None` silently disabled correction while
//! `channel_matching: Some(ChannelMatchingConfig::default())` ran with different
//! bounds than advertised.
//!
//! These tests do not exercise the full correction pipeline (which requires
//! synthetic multi-channel curves and a `RoomOptimizationResult` fixture) —
//! instead they pin the lower-level invariant that the fix relies on.

use autoeq::roomeq::ChannelMatchingConfig;

#[test]
fn channel_matching_default_values_are_frozen() {
    // These values are the contract between the validator, the UI, and the
    // optimizer. Changing them is a user-visible behaviour change and should
    // force a deliberate CHANGELOG entry, not a silent drift.
    let cfg = ChannelMatchingConfig::default();
    assert!(cfg.enabled, "channel matching is enabled by default");
    assert!(
        (cfg.threshold_db - 0.75).abs() < 1e-9,
        "default threshold_db must be 0.75 dB, got {}",
        cfg.threshold_db,
    );
    assert_eq!(
        cfg.max_filters, 5,
        "default max_filters must be 5, got {}",
        cfg.max_filters,
    );
}

#[test]
fn channel_matching_none_and_some_default_are_equivalent() {
    // The exact idiom used in `compute_and_correct_icd` after B1 fix:
    //     let cfg = config.optimizer.channel_matching.clone().unwrap_or_default();
    // A caller that omits `channel_matching` must end up with the same
    // `enabled`/`threshold_db`/`max_filters` values as a caller that sets
    // `Some(ChannelMatchingConfig::default())`.
    let omitted: Option<ChannelMatchingConfig> = None;
    let explicit: Option<ChannelMatchingConfig> = Some(ChannelMatchingConfig::default());

    let from_omitted = omitted.clone().unwrap_or_default();
    let from_explicit = explicit.clone().unwrap_or_default();

    assert_eq!(from_omitted.enabled, from_explicit.enabled);
    assert!((from_omitted.threshold_db - from_explicit.threshold_db).abs() < 1e-9);
    assert_eq!(from_omitted.max_filters, from_explicit.max_filters);
}
