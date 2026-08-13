# 0.5.11

## Complete 2026-08-12 review remediation

- Split-Band Mix now controls gain-reduction depth against the phase-matched
  LR4 low+high reference. Inactive processing is identical at every Mix value,
  eliminating dry/wet comb filtering.
- Q defines detector bandwidth only through symmetric octave edges. Both edge
  filters now use fixed Butterworth pole Q, avoiding double-encoded resonance.
- Frequency, Q, and Mode are structural in static/runtime schemas and reject
  live mutation transactionally; graph reconstruction prepares clean filters,
  crossovers, and dynamics state off the audio thread.
- Meter publication is based on elapsed samples at 30 Hz and reset clears its
  cadence. Held UI snapshots no longer impede later gain-reduction publication.
- Realtime setters no longer rebuild parameter metadata or allocate. Non-finite
  audio is sanitized before detector/filter state can be poisoned.
- Runtime metadata now reports the crate version. Strict serialized state rejects
  unknown fields, preventing future preset/schema drift from being ignored.
- Added inactive null, structural-state, fixed-pole bandwidth, sample cadence,
  held-snapshot, non-finite recovery, and realtime setter allocation regressions.

# 0.5.10

## Fixes (2026-08-12 runtime sample-rate validation)

- Frequency and Q automation now validate the resulting detector band against
  the initialized sample rate before changing state or rebuilding filters.
  In-range preset values that would reach Nyquist are rejected atomically.
- Added a 32 kHz regression covering a valid-range 16 kHz frequency update.

# 0.5.9

## Fixes (2026-08-12 buffer-contract follow-up)

- Completed process preflight before enabling DSP processing: checked `frames * channels`
  arithmetic and exact active-buffer sizing now precede any processing-side effects.
- Added regressions proving undersized buffers remain unchanged and do not advance filter/smoother
  state, and that overflowing frame/channel counts return an error without touching samples.

# 0.5.8

## Fixes (2026-08-12 factory validation follow-up)

- The canonical facade and bridge factories now use the fallible constructor.
- Unknown modes, invalid ranges, zero channels, and detector bands that reach
  Nyquist at the requested host sample rate are rejected instead of clamped.

# 0.5.7

## Fixes (2026-08-12 review remediation)

- Split-Band detection now uses the same Q-defined bandpass sidechain as Wideband mode, so the
  exposed Q control changes detection in both modes.
- Added fallible preset construction and routed the plugin bridge factory through it; malformed modes,
  zero channels, non-finite/out-of-range parameters, zero sample rate, and detection bands that
  reach Nyquist are rejected.
- Reset now clears sidechain filter and smoother state rather than only updating coefficients.
- Processing checks sample-count overflow and buffer length, and denormal flushing is restricted
  to the active region.
- Monitoring cache values now deep-clone their meter vectors, allowing cache updates while a UI
  snapshot is held.

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
