# plugins-denoiser

Shared denoiser DSP building blocks for SOTF plugins.

Modules:
- `transient` — `TransientSuppressor` for click and transient repair.
- `hiss` — `HissReducer` for stationary high-frequency noise reduction.
- `rnnoise` — `RnnoiseBackend` wrapping `nnnoiseless` for RNNoise voice denoising.

Used by `sotf-plugin-declick`, `sotf-plugin-hiss-reducer`, and `sotf-plugin-speech-denoiser` so each plugin stays a thin host adapter.
