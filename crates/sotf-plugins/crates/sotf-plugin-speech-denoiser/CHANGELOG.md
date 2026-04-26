# 0.5.4

- Initial release. Split out of `sotf-plugin-denoiser` into a dedicated voice denoiser using the RNNoise (`nnnoiseless`) backend via the shared `plugins-denoiser` crate.
- Single parameter: `enabled`.
- Reports its actual `latency_samples()` so the host can compensate.
