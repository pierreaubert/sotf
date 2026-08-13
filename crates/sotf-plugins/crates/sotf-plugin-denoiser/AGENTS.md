# sotf-plugin-denoiser

Audio denoising using MCRA noise estimation and Wiener filtering.

## Architecture

- `lib/denoiser_plugin.rs` — plugin state, streaming STFT, parameter and host contracts
- `fft.rs`, `wiener/`, `mcra.rs` — transforms, gain calculation, and noise estimation
- `multi_resolution.rs` — optional aligned 512-point analysis arm
- `noise_profile.rs` — one-second captured-profile state machine
- `masking.rs`, `polyphonic.rs`, `spectral_sub.rs` — optional analysis/reduction modes
- `params/` and `config.rs` — schema, UI layout, serialization, and validation


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::ParametricInPlacePlugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-denoiser
```

## Important Notes

- MCRA (Minima Controlled Recursive Averaging) for noise floor estimation
- Wiener filter for noise reduction
- Operates in STFT domain
- FFT topology controls are setup-only; rebuild the plugin to change them.
- The process path must remain allocation-free. Run the QA matrix when changing optional modes.
