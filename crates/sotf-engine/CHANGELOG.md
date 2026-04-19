# 1.0.22

## GD-Opt v2 — Phase GD-1e dedup: shared playback scaffolding

Refactor-only, no behaviour change. The ~400-line device-discovery +
playback + mic-capture block that `probe_channel_delays_core` and
`run_bass_anchor_core` had each been carrying is now a single
`play_per_channel_and_record_mono` helper.

- New private helper: `play_per_channel_and_record_mono(channel_indices,
  sample_rate, signal, silence_duration_ms, output_device_name,
  input_device_name, input_channel, log_tag)` returns a
  `PlayPerChannelOutput` with `recorded`, `input_sr`, per-channel
  `analysis_offsets` at `input_sr`, plus `analysis_signal_samples` and
  `analysis_silence_samples`. `log_tag` prefixes log lines so probe and
  bass-anchor captures stay distinguishable in logs.
- `probe_channel_delays_core` now delegates everything up to the
  cross-correlation analysis to the helper. Went from ~440 lines to
  ~170 lines. Regenerates the narrowband probe at `input_sr` when cpal
  negotiated a different rate — same logic, same behaviour, just
  relocated below the helper call.
- `run_bass_anchor_core` is similarly thinned. Went from ~265 lines
  to ~90 lines. Feeds the helper output straight into
  `analyze_bass_anchor_recording`.
- Net: `signal_recorder.rs` drops 142 lines (354 insertions, 496
  deletions for a single commit).

Verified:
- `cargo test -p sotf-engine --lib -- bass_anchor` — 3 passed.
- `cargo test -p sotf-engine --lib` — 165 passed (only the pre-existing
  `preflight::tests::test_get_username` sandbox env failure remains,
  present on master).
- `cargo check -p sotf-gpui -p sotf-tui` clean.

Cargo version 1.0.21 → 1.0.22.

# 1.0.21

## GD-Opt v2 — Phase GD-1e: bass anchor capture

Implements the BassAnchor wizard step runner per
`docs/gd_opt_v2_plan.md` §2.6. The step plays a low-frequency tone
burst (20 Hz × 5 cycles by default) per channel, records the mic,
and extracts per-channel phase + half-split stability of the
burst's fundamental. GD-Opt v2 uses this as a hard anchor on the
first measured bin of the sweep phase to eliminate the 2π
wraparound ambiguity at sub-100 Hz.

- New `run_bass_anchor` / `run_bass_anchor_with_recording` entry
  points (public, non-iOS). Playback scaffolding mirrors
  `probe_channel_delays` — sequential single-channel bursts with
  silence gaps, mic recording to a mono `f32` WAV, single DFT-based
  per-channel phase extraction.
- New `analyze_bass_anchor_recording` helper — the pure analysis
  core, used both by the live runner and by the replay tests. Can
  be called offline against a persisted bass-anchor WAV to
  re-derive per-channel phase without replay.
- New `BassAnchorResults` / `BassAnchorChannelResult` types. Each
  channel carries `bass_anchor_phase_deg`, `bass_anchor_magnitude`,
  and `bass_anchor_stability_deg`. Values ≥ 20° on the stability
  metric trigger the `"bass_anchor_unreliable"` advisory
  (`docs/gd_opt_v2_plan.md` §2.8).
- Three new replay tests pin behaviour:
  `bass_anchor_replay_recovers_known_phase_shifts` (phase recovery
  within ±2° across three synthetic channels),
  `bass_anchor_replay_rejects_length_mismatch`, and
  `bass_anchor_replay_errors_when_start_past_eof`.

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
