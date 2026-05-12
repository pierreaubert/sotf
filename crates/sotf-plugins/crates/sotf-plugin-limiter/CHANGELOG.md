# 0.5.9

## Fixes

- **Soft-knee curve passes exactly through threshold** (`src/lib.rs:535-560`):
  The old algebraic sqrt-based curve gave ~0.9707×threshold at the threshold boundary,
  making soft mode ~0.25 dB stricter than hard mode for the same setting.
  Replaced with a cubic Hermite polynomial over the knee region [threshold−10%, threshold].
  The new curve is C1-continuous, bounded strictly by threshold, and passes exactly through
  (threshold, threshold). Hard-clip above the knee is retained.

- **ISP correction decays in linear gain space, not dB** (`src/lib.rs:584-593`):
  `release_coeff` = exp(−1/(release_ms × sr)) is designed for linear-domain envelope
  interpolation. Applying it multiplicatively to `isp_correction_db` (a dB value) caused
  double-exponential decay in the linear domain — the correction vanished much faster than
  the configured release time. Fixed by converting to linear, decaying, then converting back.

- **Feed-forward scan is O(lookahead_len) per frame, not per sample** (`src/lib.rs:502-509`):
  The old code scanned the entire lookahead buffer (lookahead_len × channels elements) for
  every individual sample. Added a `lookahead_peaks` ring buffer (one f32 per lookahead slot)
  updated once per frame; the feed-forward scan reads only this compact array. At 48 kHz with
  5 ms lookahead and 2 channels the old code did ~23 M comparisons/s; the new code does
  ~240 K comparisons/s (100× reduction). Also removed the silent scaling problem where the
  full interleaved buffer was scanned with raw .abs() instead of using the already-computed
  per-channel peaks.

- **Channel cap removed: channels > 32 are now fully analyzed** (`src/lib.rs:455`):
  Per-channel peak detection used a fixed `[0.0f32; 32]` stack array capped at 32 channels.
  Channels 33+ were silently excluded from peak detection and would clip without gain reduction.
  Replaced with `vec![0.0f32; self.channels]` to support any channel count.

## Deferred

- **Issue #6 (set_parameter clones Vec<Parameter>)**: The `self.parameters()` call in
  `set_parameter` allocates a 10-element Vec for validation. Negligible cost (parameter
  changes are infrequent); fixing requires storing parameters in a `LazyLock`/static ref
  which is a cross-crate refactor of the parameter system. Deferred.

- **Issue #5 (stale monitoring_isp_linear)**: When `use_true_peak` is false, the
  `monitoring_isp_linear` array retains values from previous blocks but is not copied to
  the cache (guarded by `use_true_peak` check). Harmless; no observable incorrect data
  is exposed. Deferred as low-priority cleanup.

- **Issue #7 (smoother advances once per block)**: The threshold and mix smoothers advance
  once per block, not per sample. This is intentional — an explicit code comment explains
  that per-sample advance caused double-advancing. The correct fix is `next_n(num_frames)`
  returning a sample-accurate ramp, which requires a smoother API change. Deferred as a
  cross-crate refactor.

# 0.5.8

## New

- Added missing qa_*.rs files for some plugins
- Added missing parameters for new plugins

## Changes

- SOTA plugin improvements: shared DSP components + plugin upgrades
- Next iteration on UI and testing for plugins this time with native look&feel
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Assed ISP mode for the limiter
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 5)
- Massive update to plugins, see individual markdown plan for details (wave 3)
