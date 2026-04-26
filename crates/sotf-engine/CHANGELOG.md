# 1.0.26

## Added a way to cancel a running recording

- Public CancelFlag = Arc<AtomicBool> type and stable CANCELLED_ERR = "cancelled" sentinel string.
- probe_channel_delays_with_recording and run_spl_calibration gained a trailing cancel: Option<CancelFlag> parameter.
- The flag is threaded through probe_channel_delays_core → play_per_channel_and_record_mono, where it is checked:
    a. before any device setup,
    b. before kicking off playback (after the input stream is built),
	c. each iteration of the ~50 ms stability poll loop.
- Sibling probe_channel_delays and run_bass_anchor keep their existing public signatures and pass None internally.
- All three call sites (app-gpui/components/recording/probe.rs, spl_calibration.rs, and app-tui/events/conf_recordings.rs) updated to pass None for now — wiring an actual UI Stop button into a stored CancelFlag is the follow-up UI task.

# 1.0.25

## Engine threading and low-latency fixes

- Serialized `AudioEngine` command/response round trips so concurrent
  callers cannot consume each other's manager responses.
- Fixed plugin graph hot-reload acknowledgements and playback channel
  reconfiguration when graph output channel counts change.
- Moved playback underrun event reporting out of the cpal data callback
  and into the playback feeder thread.
- Reduced full-queue backoff in decoder/processing handoff paths from
  5 ms to 1 ms for lower-latency recovery from transient backpressure.
- Reused recycled buffers for HAL passthrough frames instead of
  allocating a fresh vector every frame.

# 1.0.24

## Driver HAL streaming reliability

- Driver-mode playback now retries `HalInputReader` setup after startup so a
  late-created HAL shared-memory file no longer leaves the engine permanently
  streaming silence.
- Added explicit driver-format startup helpers so HAL playback can use the
  daemon-provided sample rate and input channel count instead of assuming
  stereo 48 kHz input.
- Added regression coverage through `driver-hal`'s streaming guard tests for
  the late-HAL-reader reconnect path.

# 1.0.23

## GD-Opt v2 — Phase GD-1e.5: SPL calibration capture

Adds `run_spl_calibration` — plays a single-frequency reference tone
(1 kHz default) on one output channel while recording the mic, then
returns peak + RMS sample levels over the stable portion of the
tone. The UI uses the paired `(rms_sample_level, reported_db_spl)`
to derive `SplCalibration::spl_offset_db`, the GD-Opt v2 anchor for
the `sweep_level_db_spl` target (see `docs/gd_opt_v2_plan.md` §2.6
and §2.11 Q4).

- New public API `run_spl_calibration(output_channel, sample_rate,
  reference_freq_hz, amp, duration_s, out_dev, in_dev, input_channel)
  -> Result<SplCalibrationResult, _>`.
- New public type `SplCalibrationResult { sample_rate,
  peak_sample_level, rms_sample_level, reference_freq_hz,
  output_channel }`. Derives Debug/Clone/Serialize/Deserialize so
  sotf-player can re-export and carry it on the TUI+GPUI state
  structs.
- Reuses the existing `play_per_channel_and_record_mono` helper
  (from 1.0.22's dedup) so all device-discovery + cpal logic stays
  single-source. The calibration-specific logic is just (a) gen a
  pure tone via `gen_tone`, (b) after capture, slice a stable
  analysis window skipping 200 ms of attack/release on each end,
  (c) compute peak+RMS over that window.
- Input validation: rejects non-finite / non-positive freq, too-
  short duration (< 0.3 s), amplitude outside (0, 1].

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
