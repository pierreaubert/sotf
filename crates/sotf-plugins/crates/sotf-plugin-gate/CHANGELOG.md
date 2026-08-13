# 0.5.11

## Complete review remediation

- Make construction and preset loading fallible and reject zero channels,
  non-finite/out-of-range values, invalid choices, and unknown preset fields.
- Require initialization, a matching process sample rate, and an exact checked
  interleaved buffer length; sanitize non-finite programme and detector samples
  before they can enter filter, detector, envelope, or lookahead state.
- Treat channel linking, sidechain topology/detection, external sidechain mode,
  and lookahead as graph-rebuild parameters. Exact no-op structural writes are
  accepted, while actual live changes fail transactionally.
- Make realtime setters and reset allocation-free, clamp active hold counters
  when Hold decreases, and reset smoothers, diagnostics, filters, detectors,
  lookahead, and scratch state deterministically.
- Keep external sidechain samples read-only and flush only interleaved programme
  samples. Publish immutable, independently owned diagnostic snapshots at a
  sample-derived 30 Hz cadence.
- Derive plugin version/defaults/ranges from crate metadata and `ParamSpec`, and
  add lifecycle, buffer, factory, metadata, range, cache, reset, and allocation
  regressions.

# 0.5.10

## Fixes

- Mark the sidechain HPF order parameter as structural so its metadata matches
  the runtime setter, which rejects topology changes after initialization.

# 0.5.9

## Review follow-up

- Keep linked-channel diagnostic attenuation aligned with the envelope applied to every output channel.
- Add regression coverage for distinct per-channel input levels, attenuation snapshots, and the finite `range_db=0` safety ceiling.

# 0.5.8

## Review remediation

- Treat `range_db=0` as unlimited attenuation with a finite numerical ceiling.
- Validate factory parameters and reject invalid timing, modes, orders, NaN, and zero-channel instances.
- Reject live topology/latency changes that require graph recompilation.
- Publish independent input-level and attenuation diagnostics, at a sample-rate-derived cadence.
- Reset sidechain filter state in place to avoid lifecycle allocations; use checked buffer arithmetic.

# 0.5.7

## Fixes

- **Guard against buffer-length mismatch when external sidechain is enabled** —
  `process_in_place` now returns an error instead of indexing out of bounds when
  `sidechain_external` is toggled on but the input buffer does not contain the
  expected sidechain channels.
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
