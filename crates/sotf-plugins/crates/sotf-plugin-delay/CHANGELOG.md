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
