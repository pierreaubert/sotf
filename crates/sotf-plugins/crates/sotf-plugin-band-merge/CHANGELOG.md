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

## Deferred

- **Issue #1 (review): rename `reconstruction_error_db` to `reconstruction_level_diff_db`**
  — Cross-crate rename (parameter ID strings appear in host serialization and UI).
  Deferred to avoid a breaking preset-format change.

- **Issue #4 (review): skip `ref_sum` when diagnostic is not queried** — Minor overhead
  (one f64 add per band per sample). Not fixed: adding a dirty-flag or feature gate would
  complicate the code for negligible real-world gain. Stays simple.

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
