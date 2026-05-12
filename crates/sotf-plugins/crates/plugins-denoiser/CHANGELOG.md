# 0.5.5

- `rnnoise`: `initialize()` now returns `Result<(), String>` and validates 48 kHz sample rate.
- `rnnoise`: `process()` now takes a `bypass` flag and returns the actual number of frames written.
- `rnnoise`: first processed frame is discarded to avoid RNNoise fade-in artifacts.
- `rnnoise`: stereo processing now uses mid-channel downmix with a single `DenoiseState` to preserve the stereo image.
- `rnnoise`: `reset()` no longer allocates by swapping with a pre-allocated denoiser pool.
- `rnnoise`: pointer positions are wrapped to prevent overflow on long-running sessions.

# 0.5.4

- Initial release. Shared DSP building blocks extracted from `sotf-plugin-denoiser` so that the new dedicated declick, hiss reducer, and speech denoiser plugins can reuse the same primitives.
- Modules: `transient` (TransientSuppressor for click repair), `hiss` (HissReducer for stationary high-frequency noise), `rnnoise` (RnnoiseBackend wrapping `nnnoiseless`).
