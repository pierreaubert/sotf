# 0.5.11

## Fixes (2026-08-12 review closure)

- Made band count, channel linking, filter frequency/Q/gain and active/solo
  topology structural, eliminating destructive live filter resets and stale
  linked/unlinked or bypassed detector state transitions.
- Added exact settled-dry and zero-target-gain fast paths. Wet re-entry resets
  state deterministically before the mix ramp.
- Reset now publishes cleared monitoring data immediately; plugin metadata uses
  the package version.
- Added structural-transition, fast-path state, monitoring publication, and
  full 1/2/8/16/32-channel × 1/4/8-band × linked/unlinked × block-size QA.

# 0.5.10

## Fixes (2026-08-12 factory validation follow-up)

- Added a fallible, sample-rate-aware constructor for serialized Dynamic EQ
  state. The canonical facade and bridge factories now reject invalid global
  and per-band values, malformed band counts, non-finite numbers, and bands
  whose detector edge cannot be represented at the host rate.

# 0.5.9

## Fixes

- Live per-band gain changes now rebuild the peaking filter so reported state and audio agree.
- Dynamic depth blending now converts the requested dB depth to a linear-amplitude interpolation
  coefficient, giving symmetric and accurate boost/cut magnitude at the filter centre.
- Per-band updates now share schema validation, reject malformed IDs, wrong types, inactive band
  indices, and non-finite values, and validate bulk updates before any mutation.
- Filter centres and sidechain upper edges are constrained below Nyquist at the active sample rate;
  invalid near-zero sample rates are rejected.
- Reset now snaps mix and threshold smoothers to their stored targets and resets cache throttling.

## Tests

- Added public live-gain response parity, signed dB-depth interpolation, invalid band update,
  Nyquist-safe initialization, and smoother-reset regressions.

# 0.5.8

## Fixes

- **Exact sidechain bandpass edge math** — `bandpass_edges` now uses the
  exact peaking-EQ Q-to-octave-bandwidth relation instead of the `1/Q`
  approximation. Added `test_bandpass_edges_use_exact_q_to_octave_bandwidth`.

# 0.5.7

## Fixes

- **Per-band override flags can clear again** — setting a band threshold/ratio
  equal to the current global value now disables that override, and setting a
  global threshold/ratio equal to an existing band value clears matching band
  overrides. Added `test_band_threshold_ratio_overrides_can_return_to_global`.

# 0.5.6

## Fixes

- **Critical (acoustics): sidechain reads dry buffer** — all bands now detect from the
  pre-EQ dry buffer (`dry_buf`) instead of the in-place modified output buffer. Previously
  band N's sidechain saw the EQ output of bands 0..N-1, causing unpredictable inter-band
  modulation. Regression test added: `test_sidechain_reads_dry_buffer_not_modified_output`.

- **Critical (performance): eliminated per-sample biquad coefficient recomputation** — the
  `update_eq_gain` path called `update_params` (which evaluates sin/cos/tan) on nearly
  every sample during attack, making the CPU cost roughly O(attack_samples) transcendentals.
  Replaced with a fixed-coefficient biquad held at `target_gain_db` and a per-sample
  dry/wet blend: `out = dry + (eq_out - dry) * proportion`, where `proportion` derives
  from the smoothed dynamics envelope. No coefficient changes occur in the audio loop.
  Removed `current_gain_db` field, `update_eq_gain`, and `compute_modulated_gain`.
  Regression test added: `test_eq_gain_uses_proportion_blend_not_coefficient_update`.

- **High (performance): eliminated f32↔f64 round-trips in sidechain hot path** —
  `apply_sidechain_bp` now accepts and returns `f64` (matching the internal biquad
  precision), removing two unnecessary casts per sidechain sample.

- **Fix duplicate assignment in `test_reset_rebuilds_eq_filters_at_zero_gain`** — the test
  had `target_gain_db = 0.0` immediately overwritten by `target_gain_db = 12.0` (copy-paste
  artifact from when the field was `current_gain_db`). Rewritten to correctly simulate stale
  biquad state and verify `reset()` rebuilds at the current `target_gain_db`.

## Deferred

- 🟡 Sidechain bandpass imprecise approximation (`1/Q ≈ BW_oct`) — mathematically imprecise
  at Q extremes but not a correctness bug. Deferred: would require changing the sidechain
  filter topology, which is a more invasive change.
- 🟡 Sidechain cascaded HP+LP vs single Bandpass biquad — functional, passband error is
  small for typical Q values. Deferred: topology change outside current scope.
- 🟡 `use_band_threshold`/`use_band_ratio` flags never reset to false — cosmetic UX issue,
  no audio bug. Deferred: needs design decision on "reset to global" semantics.
- 🟡 Loop order `frame × band × ch` (cache locality) — profiling required before
  restructuring. Deferred.
- 🟡 Block-constant mix/threshold smoothing (zipper noise on automation) — deferred: would
  need per-sample smoother API changes in `sotf-host`.
- 🟢 All nit-level suggestions skipped per workflow.

# 0.5.5

- Band frequency / q live updates now rebuild sidechain and EQ filters immediately.
- Reset() now clears current_gain_db before rebuilding EQ filters, so reset returns the band to neutral gain.
- Process_in_place now checks frame/channel overflow and exact buffer length, returning Err instead of panicking.
# 0.5.12

- Mark rebuild-only band-count and channel-link controls structural in host-visible metadata, with regression coverage.
- Align the player parameter mapper with that host metadata: structural globals
  no longer have a stale realtime test expectation, realtime globals retain
  canonical engine updates, and encoded band rows stay on the validated
  dynamic-band adjust/set path.
