# 0.5.7

## Fixes

- **Document and test soft-knee shape** — `USAGE.md` now describes the Knee
  parameter as a quadratic soft-knee transition around threshold, the code
  documents the boundary behavior, and
  `test_soft_knee_curve_is_continuous_at_boundaries` verifies continuity.

# 0.5.6

## Fixes

- Precompute hold time in samples at initialization/sample-rate changes and when the Hold parameter is
  updated, using rounded sample conversion instead of truncating every audio block.

# 0.5.5

## Fixes

- `src/lib.rs:555-560, 600-605`: Attack/release coefficient selection was reversed. `target > envelope` means
  attenuation is increasing (gate *closing*) and must use `release_coeff`; decreasing attenuation (gate
  *opening*) must use `attack_coeff`. Previously, attack controlled closing speed and release controlled
  opening speed — the opposite of every DAW convention.
- `src/lib.rs:622`: In linked-channel mode `is_open` was always `true`. `envelope[1..]` retain their
  initialized value of `0.0`, so `iter().any(|&a| a < 0.1)` never returned `false` even when the gate was
  fully closed. Fixed by checking only `envelope[0]` (the linked master) when `link_channels` is set.
- `src/lib.rs:633`: `flush_denormals_inplace` was called on the full buffer including the sidechain region
  when external sidechain is active. Now restricted to the audio output region (`num_frames * channels`).

## Deferred (noted from code review)

- Performance: envelope follower in dB domain (per-sample `fast_log10`/`fast_pow10`). Requires cross-crate
  redesign with `sotf-host::LevelDetector`. Deferred.
- Performance: SIMD gain application. Deferred.
- Performance: mono HPF for linked channels. Deferred.
- New parameter: `rms_window_ms` to expose the RMS detection window. Requires a new parameter slot and
  schema migration. Deferred.
- New feature: sidechain listen/solo. Deferred.

---

# 0.5.4

## New

- The sidechain is not steep enough: add steeper crossover
- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Did a round of test fixing
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details
