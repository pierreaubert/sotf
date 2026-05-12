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
