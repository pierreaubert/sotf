# 0.5.7

- Replace the causal slew limiter with an eight-sample-lookahead robust
  pre/post median and MAD detector that reconstructs short clicks by
  channel-specific interpolation.
- Link detection in adjacent channel pairs by default, with a persisted
  `link_channels` control for independent repair when required.
- Keep the detector warm during latency-matched bypass and smooth both bypass
  and sensitivity changes over 5 ms without callback allocation.
- Process interleaved audio frame-major without deinterleave scratch or worker
  dispatch; reject invalid construction/rate/buffer contracts explicitly.
- Add clean/corrupt reference, onset, step, high-frequency, square-wave,
  repeated-click, stereo-coherence, automation, allocation, and callback-size
  regressions plus active 1–40-channel QA timing.

# 0.5.6

- Preserve the first legitimate sample after construction/reset by priming
  detector history instead of classifying an onset against zero history.
- Freeze clean-slope estimation while rejecting impulses so sustained crackle
  cannot rapidly teach the detector that corruption is normal.
- Canonicalize sensitivity to finite 1–100 values, reject zero sample rates,
  zero-channel processing, and process-context sample-rate mismatches.
- Sanitize isolated non-finite samples to the last finite output without
  poisoning subsequent detector state.
- Report the scalar nonlinear realtime path as `Dynamics`, not `Fft`.

# 0.5.5

Fixes applied to `plugins-denoiser::transient::TransientSuppressor` based on code review:

- **Fix (review issue 6, high impact)**: `last_samples` now tracks the *input* sample rather than the
  processed (clamped) output. Previously, after a click was suppressed the next frame's delta was
  computed relative to the clamped value, causing a cascade of over-suppression on legitimate
  post-click samples. Tracking the input breaks that self-referential loop.

- **Fix (review issue 2, high)**: The slope envelope is now updated during suppression using the
  *allowed* delta (`threshold`) instead of being frozen. Previously a burst of clicks left the
  envelope at its pre-burst value; now it adapts continuously so each click in a burst is evaluated
  against a current threshold.

- **Fix (review issue 5, medium)**: Added a high-curvature discriminator to prevent smooth ramps from being
  treated like clicks. The suppressor now compares second-order slope before clamping, preserving
  musical transients that are fast but continuous while still suppressing abrupt spikes.

- **Fix (review issue 3, medium)**: `slope_envelope` is initialised to `1e-6` (non-zero floor) in
  both `new()` and `reset()`. The old `== 0.0` exact-float guard is removed. This eliminates the
  discontinuous jump on the first sample after `reset()` and removes the risk of FP denormal
  accumulation at exactly zero.

- **Fix (review issue 4, medium)**: `TransientSuppressor` decay is now sample-rate-aware and slower for musical content.
  A 20 ms release target is used by default, and `set_sample_rate` computes
  `decay`/`one_minus_decay` from sample rate so release behavior is much less
  aggressive than the old fixed `decay=0.99`.
- **Fix (review issue 7, low)**: `plugins-denoiser::TransientSuppressor` now deinterleaves
  multi-channel audio into planar scratch buffers and processes each channel slice in parallel
  (`process` uses Rayon), replacing the pure single-threaded per-frame interleaved loop
  for channels > 1.

Deferred / out-of-scope (noted from review):

- **Review issue 1 (high, acoustics)**: Replacing slew-rate limiting with median-filter or AR
  interpolation is a fundamental algorithm replacement, not a bug fix. Deferred to a future
  feature PR.
# 0.5.4

- Initial release. Split out of `sotf-plugin-denoiser` into a dedicated time-domain click and transient repair plugin.
- Uses the shared `TransientSuppressor` from `plugins-denoiser`.
- Parameters: `enabled`, `sensitivity` (1.0–100.0).
