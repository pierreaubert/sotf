# 0.5.2

## Fixes

- **Per-sample frequency smoothing** (`src/lib.rs:process`): Moved crossover
  frequency updates from block-wise (`smoother.next_n(num_frames)` once per
  block) to per-sample (`smoother.advance()` per frame). The old code applied
  a single frequency value across the entire block, causing audible step
  discontinuities (clicks, warbling) when the crossover frequency was
  automated. Fixed by advancing each `LogSmoother` and calling
  `set_frequency` inside the per-frame loop.

- **Per-band gain smoothing** (`src/lib.rs`): Instant gain changes caused
  zipper noise when band gains were automated. Added a `LinearSmoother` for
  each band (20 ms ramp time, matching frequency smoothers). `set_parameter`
  for `band_N_gain_db` now calls `set_target` on the smoother; `process`
  calls `smoother.advance()` per frame. `initialize` and `reset` reinit/reset
  the gain smoothers from the current gain values.

- **Tightened DC unity-sum test tolerance** (`src/lib.rs:test_band_split_dc_sums_to_unity`):
  Tightened from 5% to 1%. The `MultibandLr4Crossover` steady-state is
  accurate to better than 0.1%, so the 5% tolerance was masking nothing
  useful. Added an additional `test_band_split_dc_sums_to_unity_tight` test
  that also uses 20 000 frames to verify full settling.

## Deferred

- **LR48 parameter is a no-op** (review issue #6): `crossover_type` accepts
  `"LR48"` but only `LR24` is instantiated in `MultibandLr4Crossover`.
  Implementing LR48 (cascaded biquad, 8th order) requires changes to
  `math-iir-fir::lr4_crossover` — deferred as a cross-crate change.

- **`LogSmoother` recreated in `initialize`** (review issue #7): The smoother
  has no `set_sample_rate` method. Recreation from the same target value is
  equivalent in behaviour; no state is lost. No fix needed.

---

# 0.5.1

## New

- Added missing qa_*.rs files for some plugins
- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Did a round of test fixing
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details
