# 0.5.4

## Fixes

- `set_parameter` (`lib.rs:218,247`): replaced hardcoded gain-range `[-100.0, 24.0]` with
  spec-derived bounds from `params::PARAMS` (`[-60.0, 20.0]`). The mismatch meant
  `set_parameter` accepted values that `parameters()` advertised as out-of-range, silently
  bypassing the documented limits. Applies to both the global `gain_db` arm and all
  per-channel `gain_db_{N}` arms.
- `from_params` (`lib.rs:127`): error message on channel-count mismatch now includes the
  expected and actual lengths (e.g. `"channel_gains length mismatch: expected 2, got 3"`)
  instead of the opaque `"Mismatch"` string.

## Deferred (from review)

- SIMD granularity (🟡): calling `apply_gain_simd` per-frame on 2-channel slices may have
  overhead vs scalar. Deferred — needs profiling; cross-crate SIMD changes are out of scope.
- Per-channel smoother SoA layout (🟡): potential cache-thrashing at high channel counts (≥32).
  Deferred — requires benchmarking on representative channel counts first.
- Pre-`initialize()` stale sample-rate in `set_channel_gain_db` (🟡): `initialize()` already
  calls `set_time()` which fully recalculates smoother coefficients at the correct rate.
  The review itself acknowledged this corrects the state. No code change needed; documented
  in CLAUDE.md.

# 0.5.3

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details
