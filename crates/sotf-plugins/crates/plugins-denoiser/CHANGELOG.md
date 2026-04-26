# 0.5.4

- Initial release. Shared DSP building blocks extracted from `sotf-plugin-denoiser` so that the new dedicated declick, hiss reducer, and speech denoiser plugins can reuse the same primitives.
- Modules: `transient` (TransientSuppressor for click repair), `hiss` (HissReducer for stationary high-frequency noise), `rnnoise` (RnnoiseBackend wrapping `nnnoiseless`).
