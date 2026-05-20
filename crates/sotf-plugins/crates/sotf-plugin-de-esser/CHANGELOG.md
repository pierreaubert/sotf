# 0.5.6

## Fixes

- **Detection Q now reaches the sidechain filters** (`src/lib.rs`): The user Q
  parameter is passed to both highpass and lowpass `BiquadBank` construction
  and updates, so Q affects actual sidechain filter shape in addition to
  bandwidth edge calculation.

## Review Notes

- Clarified `review-plugin-de-esser.md` issue 3: `Lr4Crossover::new` is always
  LR4; its third argument is channel count, not filter order. The de-esser's
  `1` value is correct because it builds one single-channel crossover per
  plugin channel.

# 0.5.5

## Fixes

- **Mix smoother zipper noise (review issue 1)**: `next_n(num_frames)` advanced the
  one-pole smoother to its final value and applied that single scalar to the entire
  block — causing an instantaneous jump at the block boundary instead of a
  per-sample ramp. Replaced with `advance()` called once per frame inside the
  processing loop so the mix transitions smoothly sample-by-sample.
  `src/lib.rs` lines 538–606 (both wideband and split-band paths).

- **Filter precision + vectorization (review issues 4, 6)**: wideband sidechain filtering
  now uses f32 `BiquadBank::process_interleaved_frame` over reusable per-frame
  scratch buffers (`sidechain_frame`), and the final wideband wet/dry multiply now uses
  `apply_per_channel_gain_simd` on `frame_gains`. This removes the per-sample
  f64↔f32 conversion overhead and moves part of the hot loop into a
  channel-vectorized path.
  `src/lib.rs` lines 565–606.

- **Monitoring write cadence (review issue 5)**: moved `monitoring_gr` updates from per-sample
  writes to final-frame writes in both processing modes, reducing unnecessary stores in the
  inner loop while keeping the cache contract unchanged.
  `src/lib.rs` lines 590 and 629.

## Deferred / Reviewed Claims

- **Review issue 2 — Q parameter labeling**: The user-facing Q parameter controls detection
  bandwidth via `bandpass_edges()`, while the actual biquad poles are fixed at Butterworth
  Q (≈0.707). The behavior is documented in `bandpass_edges` comments. Renaming the
  parameter to "Bandwidth" is a UI/API-breaking change deferred to a follow-up that
  updates the corresponding GPUI/TUI parameter wiring.

---

# 0.5.4

## New

- Added missing qa_*.rs files for some plugins

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
# Unreleased

## Fixes
- Fixed block-constant mix smoother: replaced `next_n(num_frames)` with per-frame linear ramp to prevent zipper noise during mix automation.
- Fixed split-band crossover order: changed from 1st-order (6 dB/octave) to 4th-order (24 dB/octave) for proper band isolation.
