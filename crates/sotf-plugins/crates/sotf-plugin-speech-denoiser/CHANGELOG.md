# 0.5.5

- Fixed CRITICAL: bypass now routes audio through the same 480-sample delay line as active processing, so the actual latency stays constant when toggling enabled/disabled during playback. Previously bypass was zero-latency, causing phase cancellation against latency-compensated tracks.
- Fixed CRITICAL: `latency_samples()` now returns a constant 480 samples regardless of bypass state.
- Fixed CRITICAL: `process_in_place()` now rejects block sizes that are not a multiple of 480, preventing periodic dropouts/glitches in hosts with arbitrary buffer sizes.
- Fixed CRITICAL: `initialize()` now validates that sample rate is exactly 48 kHz; RNNoise is hard-coded for 48 kHz and silently corrupts frequency response at other rates.
- Fixed MAJOR: first processed frame is now discarded in both enabled and bypass modes to avoid RNNoise fade-in artifacts on transport start/reset.
- Fixed MAJOR: stereo content is now processed via mid-channel downmix with a single `DenoiseState`, preserving the stereo image instead of applying independent per-channel noise suppression.
- Fixed MAJOR: `reset()` no longer allocates on the audio thread by swapping with a pre-allocated `DenoiseState` pool.

# 0.5.4

- Initial release. Split out of `sotf-plugin-denoiser` into a dedicated voice denoiser using the RNNoise (`nnnoiseless`) backend via the shared `plugins-denoiser` crate.
- Single parameter: `enabled`.
- Reports its actual `latency_samples()` so the host can compensate.
# Unreleased

## Fixes
- Fixed dynamic latency on bypass: `latency_samples()` now always returns 480 regardless of `enabled` state.
- Fixed periodic dropouts for non-480-frame buffers: `process_in_place` now rejects block sizes that are not a multiple of 480 frames.
- Fixed missing sample-rate validation: `initialize` now returns an error if sample rate is not 48 kHz.

