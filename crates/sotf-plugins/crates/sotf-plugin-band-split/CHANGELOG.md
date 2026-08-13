# 0.5.5

## Fixes (2026-08-12 review closure)

- Bound frequency-dependent IIR redesign to a persistent 6 kHz control rate
  while retaining audio-rate logarithmic smoothing and exact callback-partition
  invariance. The LR24/LR48 regression requires under 2% relative RMS error
  against audio-rate coefficient design and under 0.01 RMS zipper residual.
- Make successful live frequency/gain setters allocation-free, keep LR24/LR48
  structural, and align parameter update metadata with those contracts.
- Add independent LR24/LR48 slope/complementarity, impulse magnitude/phase,
  deterministic noise reconstruction, multiband/sample-rate, 12-channel
  isolation, reset, malformed-context, and worst-case deadline coverage.
- Reject unknown persisted fields/crossover types and require initialized,
  sample-rate-matched processing.
- Keep engine channel topology in the graph contract rather than serializing it as an unknown Band Split runtime parameter.

# 0.5.4

## Fixes

- Reject channel-count multiplication overflow before allocating split-band scratch storage and use checked output sizing in processing.

# 0.5.3

## Fixes (from code review 2026-08-12)

- Validate crossover topology at construction, initialization, and parameter updates: values must
  be finite, within the declared/sample-rate-aware range, and strictly ascending.
- Validate exact checked input/output sample counts before processing, returning errors instead of
  panicking on malformed host buffers.
- Treat LR24/LR48 selection as structural after initialization, avoiding allocation and filter-state
  reset on a live parameter path.
- Reject non-finite/out-of-range dynamic gain and frequency values transactionally.
- Reset frequency smoothers and crossover coefficients to their targets for deterministic transport
  resets while retaining allocation-free steady-state processing.
- Qualify documentation for cascaded multiband phase/reconstruction behavior.

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

- **Fixed crossover-type dead parameter** (`src/lib.rs`):
  `crossover_type` is now honored in both constructor and runtime parameter updates.
  The plugin now uses `MultibandLr4Crossover` for `LR24` and `MultibandLr8Crossover`
  for `LR48`, rebuilding the active crossover when the type parameter changes.
  This resolves the previously unimplemented "LR48 dead path".

- **Used geometric defaults for legacy `num_bands` expansion** (`src/lib.rs:from_params`):
  Replaced arbitrary 8×/4×/3× multipliers with consistent octave-like spread
  (×4 per step): 3-band defaults now `[f, 4f]`; 4-band defaults now
  `[f, 4f, 16f]` (clamped to 20 kHz). This matches the intended geometric
  behavior and keeps bands better separated.

- **Removed per-frame split helper allocation overhead** (`src/lib.rs:process`):
  Replaced `split_at_mut` loop-based band slicing with direct fixed-index
  slicing over the preallocated `band_flat` buffer (`band_idx * in_ch .. (band_idx + 1) * in_ch`),
  reducing bounds checks and keeping the operation fully branch-light.

## Deferred

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
