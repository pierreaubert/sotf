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

- **Fix (review issue 3, medium)**: `slope_envelope` is initialised to `1e-6` (non-zero floor) in
  both `new()` and `reset()`. The old `== 0.0` exact-float guard is removed. This eliminates the
  discontinuous jump on the first sample after `reset()` and removes the risk of FP denormal
  accumulation at exactly zero.

Deferred / out-of-scope (noted from review):

- **Review issue 1 (high, acoustics)**: Replacing slew-rate limiting with median-filter or AR
  interpolation is a fundamental algorithm replacement, not a bug fix. Deferred to a future
  feature PR.
- **Review issue 4 (medium)**: Making the decay coefficient sample-rate-dependent or two-stage
  (fast attack / slow release) requires a plugin API change (`initialize` must propagate
  sample rate to the suppressor). Deferred.
- **Review issue 5 (medium)**: Adding a high-pass pre-emphasis stage for frequency discrimination
  is an algorithm enhancement. Deferred.
- **Review issue 7 (low)**: SIMD parallelisation of the per-channel loop is a performance
  enhancement; the current scalar code is adequate for all supported channel counts. Deferred.

# 0.5.4

- Initial release. Split out of `sotf-plugin-denoiser` into a dedicated time-domain click and transient repair plugin.
- Uses the shared `TransientSuppressor` from `plugins-denoiser`.
- Parameters: `enabled`, `sensitivity` (1.0–100.0).
