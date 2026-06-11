# sotf-plugin-denoiser

Audio denoising using MCRA noise estimation and Wiener filtering.

## Architecture

- `lib.rs` — Main plugin struct, implements `InPlacePlugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::InPlacePlugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-denoiser
```

## Important Notes

- MCRA (Minima Controlled Recursive Averaging) for noise floor estimation
- Wiener filter for noise reduction
- Operates in STFT domain
