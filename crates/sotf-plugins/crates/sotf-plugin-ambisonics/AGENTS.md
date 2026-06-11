# sotf-plugin-ambisonics

Ambisonics Decoder — AllRAD decoding from Higher-Order Ambisonics (HOA) to speaker layouts.

## Architecture

- `lib.rs` — Main `AmbisonicsDecoderPlugin`, implements `Plugin` trait
- `config.rs` — `AmbisonicsDecoderConfig`: decoder configuration
- `decode_matrix.rs` — Decoding matrix computation
- `spherical_harmonics.rs` — Spherical harmonics evaluation
- `params.rs` — Parameter definitions

## Key Public API

- `AmbisonicsDecoderPlugin` implementing `Plugin`
- `AmbisonicsDecoderConfig` — decoder configuration

## Testing

```bash
cargo test -p sotf-plugin-ambisonics
```

## Important Notes

- AllRAD (All-Round Ambisonic Decoding) algorithm
- Supports arbitrary speaker layouts via VBAP
- Changes channel count (HOA channels → speaker layout channels)
- Uses spherical harmonics for spatial encoding/decoding
- Behind `iamf` feature flag in sotf-engine
