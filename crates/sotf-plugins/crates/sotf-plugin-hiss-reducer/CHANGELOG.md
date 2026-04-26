# 0.5.4

- Initial release. Split out of `sotf-plugin-denoiser` into a dedicated stationary high-frequency hiss reducer.
- Uses the shared `HissReducer` core from `plugins-denoiser`.
- Parameters: `enabled`, `threshold_db` (SNR threshold), `frequency_hz` (cutoff above which hiss removal applies), `strength` (0.0–1.0), `low_latency` (smaller FFT path).
