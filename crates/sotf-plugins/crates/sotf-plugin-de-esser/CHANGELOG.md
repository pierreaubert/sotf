# 0.5.5

## Fixes

- **Mix smoother zipper noise (review issue 1)**: `next_n(num_frames)` advanced the
  one-pole smoother to its final value and applied that single scalar to the entire
  block — causing an instantaneous jump at the block boundary instead of a
  per-sample ramp. Replaced with `advance()` called once per frame inside the
  processing loop so the mix transitions smoothly sample-by-sample.
  `src/lib.rs` lines 538–606 (both wideband and split-band paths).

## Deferred / Reviewed Claims

- **Review issue 3 — Crossover "order=1"**: The review claims `Lr4Crossover::new(freq, sr, 1)`
  uses `order=1` (6 dB/octave). This is incorrect. The third argument is `channels`, not
  `order`. `Lr4Crossover` always implements a true LR4 filter by cascading two second-order
  Butterworth biquads per band (24 dB/octave). The value `1` is the per-crossover channel
  count, which is correct because each channel has its own `Lr4Crossover` instance. No fix
  needed.

- **Review issue 2 — Q parameter labeling**: The user-facing Q parameter controls detection
  bandwidth via `bandpass_edges()`, while the actual biquad poles are fixed at Butterworth
  Q (≈0.707). The behavior is documented in `bandpass_edges` comments. Renaming the
  parameter to "Bandwidth" is a UI/API-breaking change deferred to a follow-up that
  updates the corresponding GPUI/TUI parameter wiring.

- **Review issues 4, 5 — f64↔f32 conversion overhead, monitoring_gr write frequency**:
  Low-severity performance items. Benchmarks do not yet show these as bottlenecks.
  Deferred.

- **Review issue 6 — SIMD vectorization**: Cross-crate refactor; deferred.

---

# 0.5.4

## New

- Added missing qa_*.rs files for some plugins

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
