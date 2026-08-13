# 0.5.13

- Expose RNNoise's bounded, release-smoothed 22-band suppression decisions and
  VAD probability instead of reconstructing a broadband gain from output RMS.
- Apply one polarity-aware, energy-normalized stereo detector's band gains to
  both original channels with fixed latency and the existing warm bypass path.

# 0.5.12

- Keep RNNoise state advancing through a latency-aligned, 480-sample bypass
  crossfade and reject layouts wider than stereo.
- Sanitize and clamp model input, remove unused reduction metering, and move
  large reusable model workspaces out of the callback stack while preserving
  reference-vector output.

# 0.5.11

- Rework `HissReducer` as an honestly documented zero-latency high-band
  downward expander with fast/slow power tracking, persistence, hysteresis,
  continuous reduction depth, and sample-rate-derived timing.
- Add warm click-free bypass, smoothed cutoff automation, non-finite recovery,
  denormal guards, and exact settled-dry/zero-strength behavior.

# 0.5.10

- Replace `TransientSuppressor`'s derivative clamp with a fixed eight-sample
  lookahead robust-context interpolator for Declick.
- Add linked stereo-pair decisions, warm smoothed bypass, smoothed sensitivity,
  exact delayed dry output, and frame-major allocation-free processing.
- Remove the unused Rayon dependency and the inaccurate parallel-processing
  claim from the shared transient path.

# 0.5.9

Linked-stereo RNNoise gain now transitions between detector decisions over
model frames. This bounds cancellation-prone stereo amplitude modulation while
retaining one common gain and preserving the stereo image.

# 0.5.8

Stereo RNNoise now detects cancellation-prone layouts before linking. Coherent
stereo retains one shared frame gain; anti-phase, hard-panned, and unequal-level
stereo use independent model detectors but apply one bounded gain to the
original channels, preserving the stereo image without collapse or sample-wise
gain modulation.

# 0.5.7

Bug fixes from the Speech Denoiser review (2026-08-12):

- Stream arbitrary host callback sizes through the fixed RNNoise quantum with
  a pre-seeded, constant-latency output queue.
- Sanitize non-finite samples, prepare shared FFT resources during initialize,
  and use frame-level linked stereo gain for cancellation-safe behavior.

# 0.5.6

Bug fixes from code review (2026-05-16):

- RNNoise reduction metering now averages input/output power across all processed channels instead
  of sampling channel 0 only. This keeps stereo and multichannel monitoring representative.

# 0.5.5

Bug fixes from code review (2026-05-11):

- **Fixed (1.4 major):** First processed 480-sample frame is now discarded to remove the fade-in artifact documented by `nnnoiseless`. This adds a fixed one-time 480-sample startup delay; subsequent frames are unaffected.
- **Fixed (3.1 medium):** Per-channel `input_buf` / `output_buf` scratch arrays are now pre-allocated in `initialize()` and stored in `scratch_input` / `scratch_output`. The audio callback no longer pushes 3.8 KB onto the stack on every 480-sample block.
- `initialize()` now returns `Result<(), String>` and rejects sample rates other than 48000 Hz with a descriptive error message.
- **Fixed (review issue 4):** `TransientSuppressor` now uses a sample-rate-aware release constant derived from a 20 ms target.
  `set_sample_rate` computes `decay` and `one_minus_decay` from sample rate, replacing the fixed `decay=0.99` fallback and making suppression release substantially less aggressive for music.
- **Fixed (review issue 5):** `TransientSuppressor` now applies a high-curvature discriminator before clamping.
  This rejects smoother onsets where `|\(x[n]-x[n-1])-(x[n-1]-x[n-2])| / |x[n]-x[n-1]|` is low, while still suppressing impulsive spikes.
- **Fixed (review issue 7):** `TransientSuppressor` now deinterleaves multi-channel input into
  planar scratch buffers and processes each channel in parallel, avoiding the single-threaded
  per-frame interleaved loop when `channels > 1`.

Deferred / noted:
- **2.1 (medium — ring buffer pointer overflow):** Absolute write/read positions remain monotonically increasing `usize`; practical overflow would require ~years of continuous audio on a 64-bit system. Full modulo wrap-on-advance deferred.
- **3.2 (major — alloc in reset):** `nnnoiseless::DenoiseState` has no `clear()` method, so `reset()` must allocate. Documented in code; callers must not invoke `reset()` per-callback.
- **1.5 (major — stereo image):** Independent per-channel RNNoise processing is a known limitation. Mid/side or gain-application-only approach deferred.

# 0.5.4

- Initial release. Shared DSP building blocks extracted from `sotf-plugin-denoiser` so that the new dedicated declick, hiss reducer, and speech denoiser plugins can reuse the same primitives.
- Modules: `transient` (TransientSuppressor for click repair), `hiss` (HissReducer for stationary high-frequency noise), `rnnoise` (RnnoiseBackend wrapping `nnnoiseless`).
# 0.5.6

- Improve the shared transient suppressor's startup priming, rejected-sample
  slope learning, sensitivity bounds, and non-finite recovery for Declick.
