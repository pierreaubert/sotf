# 0.5.6

## Added

- **Per-channel delay mode** for the RoomEQ factored graph: new constructor
  `DelayPlugin::new_per_channel(channel_delays_ms: Vec<f32>) -> Result<Self, String>`
  builds a plugin with an independent delay time per channel. JSON params
  gain an optional `channel_delays_ms: Vec<f32>` field which, when non-empty,
  takes precedence over the scalar `delay_ms` and switches the plugin into
  per-channel mode. Per-channel `delay_ms_{N}` parameter ids are exposed via
  `parameters()` for runtime tweaks. `is_per_channel()` reports the mode.

## Breaking

- `DelayPlugin::from_params(channels, params)` now returns
  `Result<Self, String>` instead of `Self`. The added error case is a
  channel-count mismatch when `params.channel_delays_ms` is non-empty and
  its length disagrees with the `channels` argument — previously this
  silently sized from the array (producing buffer-size drift downstream).
  Six call sites (the universal factory, the AB-compare sub-factory, the
  plugins-bridge factory, the QA harness, the plugin fuzzer, and the
  in-crate test) updated.

## Fixes

- `reset()` now snaps `delay_smoother`, `feedback_smoother`, `mix_smoother`,
  and every per-channel smoother to their current targets. Previously a
  reset-after-target-change left the smoother mid-ramp at the next process
  call, causing ~50 ms of pitch glitch on the first block.
- `debug_assert_eq!` on `channel_delays_ms.len() == channel_delay_smoothers.len()`
  at the top of `process_in_place` surfaces invariant drift before it manifests
  as a less-helpful indexing panic in release builds.

# 0.5.5

## Fixes

- `src/lib.rs:321-336` — Per-sample smoother advance (Issue 1): `delay_smoother`,
  `feedback_smoother`, and `mix_smoother` now call `advance()` once per sample
  inside the processing loop instead of `next_n(num_frames)` before the loop.
  Block-constant smoothing caused discontinuous jumps at block boundaries when
  delay time was automated — producing doppler pitch glitches — and zipper noise
  on mix automation. Added `test_mix_smoother_per_sample_ramp` to guard regressions.

- `src/lib.rs:340-343` — LFO phase wrapping (Issue 3): replaced conditional
  `if phase >= 1.0 { phase -= 1.0; }` with `phase = phase.fract()`. Prevents
  incorrect phase wrap when `lfo_rate_hz / sample_rate > 1.0`.

- `src/lib.rs:110-111`, `src/lib.rs:290-291` — Power-of-two buffer (Issue 5):
  `max_samples` is now rounded up to the next power of two (`next_power_of_two()`).
  This enables the compiler to replace the four `% max_samples` operations per
  sample in `process_in_place` with fast bitwise AND instructions in release builds.

## Deferred

- Issue 2 (allpass coeff parameter): exposing the allpass coefficient as a UI
  parameter requires adding a new `ParamSpec` entry and wiring it through three
  places (`rebuild_cached_parameters`, `set_parameter`, `get_parameter`). Deferred
  as a future enhancement — no correctness impact.

- Issue 4 (LFO clamping asymmetry): the asymmetric clamp at 1 sample minimum is
  intentional (interpolation guard). Documented in `effective_delay_samples`.
  No code change needed.

- Issue 6 (deinterleaved buffer layout): would require changing the flat
  interleaved `buffer[pos * channels + ch]` layout. Cross-crate refactor, deferred.

# 0.5.4

## Fixes

- Fixed a lot of tests and then the corresponing code

## Changes

- Factor out roomeq interaction from the app and simplify the code
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details
