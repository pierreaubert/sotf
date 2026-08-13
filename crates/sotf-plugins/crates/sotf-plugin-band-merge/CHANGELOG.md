# 0.5.6

## Exhaustive review follow-up

- Require initialization and a callback sample rate matching initialization
  before processing, with deterministic lifecycle regressions.
- Reject unknown fields in both canonical and facade preset state, and add public
  factory coverage for channel/band topology and gain validation.
- Keep host-visible cached gain/mute values synchronized after successful live
  updates without rebuilding or allocating parameter schemas.
- Use checked input-channel reporting and tie plugin version to crate metadata.

# 0.5.5

## Fixes

- The armed reconstruction diagnostic is covered by allocation counting and logger interception;
  its realtime callback path performs neither heap allocation nor logging.
- `reconstruction_error_db` now reports normalized RMS of the actual output-minus-reference error,
  with finite -60 dB and +60 dB bounds for exact reconstruction and a cancelled reference.
- Supported 2–8-band scalar reductions use explicit unrolled kernels. Deterministic dispatch and
  scalar-reference tests cover every supported band count and 1/2/6/8-channel layouts, with
  Criterion coverage for 2x2, 2x4, 6x4, and 8x8 channel-by-band configurations.

# 0.5.4

## Fixes

- Reset now preserves each band's mute target instead of restoring muted bands to their configured
  gain; transport reset cannot unexpectedly re-enable a muted band.

# 0.5.3

## Fixes

- Gain automation now advances and applies the one-pole smoother per sample, making output independent of host callback partitioning.
- Mute and unmute transitions share the configured-gain smoother instead of stepping immediately to zero or unity.
- Construction and runtime gain updates reject non-finite or out-of-range values; zero channels, zero sample rate, arithmetic overflow, and inexact buffers return errors before processing state changes.
- Band count now follows the canonical 2–8 schema and live structural mutation is rejected with a rebuild-required error.
- Removed realtime diagnostic logging from `process`; the numeric diagnostic remains available through the parameter cache.

# 0.5.2

## Fixes

- **Issue #2 (review): zipper noise on gain automation** — Added per-band one-pole gain
  smoother (`sotf_host::smoothing::Smoother`, 10 ms time constant) to `BandMergePlugin`.
  `set_parameter` now updates the smoother target instead of snapping the linear gain
  directly; `initialize()` stores the sample rate and sets the smoother coefficient;
  `reset()` snaps all smoothers to their current target so playback resumes at the
  correct gain without a ramp artefact.
  Files: `src/lib.rs` (lines 14–75, 170–180, 221–240, 260–285).

- **Issue #3 (review): branch in inner process loop prevents vectorization** — Pre-computed
  an `effective_gains` array (muted bands get 0.0, active bands get the smoothed linear
  value) before the frame loop. The inner `sum += sample * effective_gains[band]` is now
  branch-free and can be auto-vectorized by LLVM.
  File: `src/lib.rs` (lines 260–285).

- **Issue #4 (review): skip reconstruction metric accumulation unless requested** — Added
  an on-demand diagnostic path for `reconstruction_error_db` using an internal request flag.
  The reconstruction/reference energies are now accumulated and the error is recomputed only
  when the host reads `reconstruction_error_db`, reducing per-frame overhead for normal
  processing.
  File: `src/lib.rs` (lines 214–304).

## Deferred

- **Issue #1 (review): rename `reconstruction_error_db` to `reconstruction_level_diff_db`**
  — Cross-crate rename (parameter ID strings appear in host serialization and UI).
  Deferred to avoid a breaking preset-format change.

# 0.5.1

## New

- Added missing qa_*.rs files for some plugins
- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details (wave 2)
- Massive update to plugins, see individual markdown plan for details
