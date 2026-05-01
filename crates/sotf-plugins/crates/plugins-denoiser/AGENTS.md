# plugins-denoiser

Shared denoiser DSP building blocks for SOTF plugins. Extracted from `sotf-plugin-denoiser` so the dedicated declick, hiss-reducer, and speech-denoiser plugins can reuse the same primitives without duplicating logic.

## Modules

- `transient` — `TransientSuppressor` for click and transient repair (used by `sotf-plugin-declick`).
- `hiss` — `HissReducer` for stationary high-frequency noise reduction (used by `sotf-plugin-hiss-reducer`).
- `rnnoise` — `RnnoiseBackend` wrapping `nnnoiseless` for RNNoise voice denoising (used by `sotf-plugin-speech-denoiser`).

## Testing

```bash
cargo check -p plugins-denoiser && cargo clippy -p plugins-denoiser
cargo test -p plugins-denoiser
```

## Important Notes

- Pure DSP: no `Plugin` trait impls live here — those are in the per-plugin crates that depend on this one.
- Pre-allocate all working buffers in builders; never allocate on the audio path.
- Each block is independently testable — keep public APIs narrow and free of plugin-host concerns.
