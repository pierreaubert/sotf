# 0.5.14

- Replace spline-based peak estimation with the ITU-R BS.1770 Table-2 4x
  polyphase FIR and cover its detector delay in predictive ISP mode.
- Replace full lookahead scans with preallocated monotonic maximum queues,
  preserving independent channel behavior with amortized O(1) updates.
- Require controllable ISP configurations (hard mode, 100% wet, adequate
  lookahead), sanitize non-finite input, and keep lookahead graph-structural.
- Move the soft behavior into a one-dB dB-domain gain-computer knee, avoiding
  the alias-prone post-gain cubic waveshaper.
- Publish meters on a sample cadence independent of callback partitioning,
  make reset/reinitialize deterministic, and stop rebuilding parameter schema
  storage for ordinary control changes.

# 0.5.13

## Fixes

- `link_amount=0` now uses independent per-channel gain-reduction envelopes and
  lookahead peak histories, so a loud channel no longer attenuates a quiet channel.

# 0.5.12

## Fixes

- Zero lookahead is now a true zero-latency path instead of an unreported
  one-sample ring delay.
- Nonzero lookahead always uses the upcoming peak window, preventing release
  from occurring before the delayed transient reaches the output.
- Serialized numeric parameters are made finite and clamped to their
  authoritative schema bounds before buffer sizing and DSP initialization.
- Processing rejects mismatched buffers before advancing state; reset now
  restores ring position, smoothers, meters, counters, and detector state.
- ISP correction now releases toward unity with the correct one-pole recurrence.

# 0.5.11

## Fixes

- **Make `link_amount` effective** — partial linking now blends the detector
  from average channel peak toward the strict linked maximum. Previously the
  code took `max()` after per-channel blending, which always returned the
  linked maximum and made every nonzero setting behave like full linking.
  Added `test_link_amount_interpolates_average_to_peak_detection`.

# 0.5.10

## Fixes

- Preallocate lookahead audio/peak storage to the configured maximum for the current sample rate.
  Runtime lookahead changes now update the active window length without resizing the backing buffers
  inside the audio path.

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

- **Mix and threshold smoothers are now advanced per frame** (`src/lib.rs:413-470`):
  `mix` and `threshold` previously advanced once per block, which made parameter automation
  within a block effectively stepwise. They are now advanced per-frame inside the processing
  loop so smoother ramps progress continuously at sample resolution.

## Deferred

- **set_parameter validation now uses cached definitions** (`src/lib.rs:314-326`):
  Parameter lookup is now done directly from `self.cached_parameters` so we no longer
  allocate a fresh parameter `Vec` inside `set_parameter` just for validation.

- **ISP meter now floors whenever true peak is inactive** (`src/lib.rs:626-646`):
  `monitoring_isp_linear` is reset each block, and cache updates now explicitly write
  `-120.0 dB` when `true_peak`/`isp_mode` are disabled. This avoids stale per-channel
  ISP values persisting after toggling modes.

- **Issue #7 (smoother advances once per block)**: resolved (`src/lib.rs:413-470`) by
  advancing smoothers per sample inside the processing loop.

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
# Unreleased

## Fixes
- Fixed catastrophic CPU waste in feed-forward lookahead scan: replaced O(lookahead_len × channels) per-sample scan with amortized O(1) running-max update.
- Fixed 32-channel hard cap: `ch_peaks` now dynamically sized to `channels`, so all channels are analyzed.
- Fixed ISP correction decay operating in wrong domain: decay now happens in linear gain space before converting back to dB, matching the release time constant.
# 0.5.15

- Mark latency-changing lookahead structural in host-visible metadata, matching its rebuild-only setter.
