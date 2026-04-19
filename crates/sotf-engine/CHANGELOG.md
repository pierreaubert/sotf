# 1.0.20

## GD-Opt v2 — Phase GD-1a.1: Drop legacy recording migration

Removes the V1 → V2 recording migration path per the "no back-compat"
decision recorded in
[`crates/autoeq/docs/gd_opt_v2_plan.md`](../autoeq/docs/gd_opt_v2_plan.md)
§2.10 (row **GD-1a.1**) and §2.11 Q6.

**Breaking:** the following public items are removed from
`sotf_audio::signal_recorder` and from the top-level `lib.rs`
re-exports:

- `LegacyRecordingResult` (struct)
- `LegacyChannelRecording` (struct)
- `LegacyRecordingSession` (struct)
- `migrate_legacy_recording` (function)
- `write_extended_csv` (private helper; had no callers after the
  migration function was deleted)

Pre-GD-Opt-v2 `recordings.json` files can no longer be loaded by
sotf-engine. A typed error variant
(`AutoeqError::UnsupportedRecordingFormat`) is reserved in the
`autoeq` crate (v0.4.33) for the loader integration that lands in a
later GD-Opt v2 phase; users with legacy sessions must re-record.

A pre-removal sweep (`grep -rn "migrate_legacy_recording|Legacy\
RecordingSession" crates/ --include="*.rs"`) confirmed no external
callers outside sotf-engine itself. The local struct of the same
name in `crates/autoeq/bin/convert_recording.rs` is unrelated — it
defines its own `LegacyRecordingResult` in the converter binary and
never imported the engine's version.

Verified clean: `cargo check -p sotf-engine --all-targets`.
