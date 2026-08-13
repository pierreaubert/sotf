# [Unreleased]

## Verification

- Document legacy infallible global setters' invalid-input contract: NaN,
  infinities, and out-of-range values are atomic no-ops. Focused coverage
  preserves global/per-channel mode, smoother current/target, host-visible
  values, and rendered audio; validated parametric setters remain the
  error-reporting API.

# 0.5.8

## Performance

- Apply settled global and per-channel gains with one whole-block SIMD kernel
  call. Gain ramps retain the sample-accurate per-frame path, with deterministic
  tests covering path selection and block-partition equivalence.

## Fixes

- Source the runtime plugin version from `CARGO_PKG_VERSION` and test that it
  remains identical to the crate manifest version.
- Align the executable QA scenario with the public `-60..=20 dB` gain range and
  canonical 10 ms smoothing documentation.

# 0.5.7

## Fixes

- Validate preset/config gain and smoothing values, including channel counts and
  non-finite values, before constructing DSP state.
- Preserve active gain ramps when switching between global and per-channel
  automation modes.
- Make regular processing use the same checked active-buffer contract as the
  compiled path, preserving oversized output tails and returning errors for
  undersized buffers.
- Reset gain smoothers deterministically to their declared targets.
- Align package/plugin metadata and usage documentation with the shipped 10 ms
  default smoothing behavior.

## Changed

- Default gain smoothing time is now 10 ms (from `params::PARAMS`) instead of the
  previous 20 ms hard-coded in `GainPlugin::new`.

## Added

- Backward-compatible convenience constructors: `GainPlugin::new(channels, gain_db)`
  and `GainPlugin::new_per_channel(channel_gains)`. They delegate to the
  `_with_smoothing` variants using the canonical default smoothing time.
- `GainPlugin::new_per_channel_with_smoothing(channel_gains, smoothing_ms)` for
  per-channel construction with an explicit smoothing time.
- `sotf_plugin_gain::params::default_smoothing_ms()` exposes the canonical default.

# 0.5.6

## Fixes

- **Avoid tiny per-frame SIMD calls** — global and per-channel gain now use
  scalar frame helpers for channel counts up to four, keeping the SIMD path
  for larger frames. Added `test_small_frame_gain_helpers_match_expected_scalar_results`.

# 0.5.5

## Fixes

- Added regression coverage for calling `set_channel_gain_db()` before
  `initialize()`: `initialize()` recalculates per-channel smoother coefficients
  at the real host sample rate while preserving the pre-set target. Documented
  that this call order is supported.

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
