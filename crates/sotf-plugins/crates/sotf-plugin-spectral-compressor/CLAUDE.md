# sotf-plugin-spectral-compressor

Spectral compressor — per-bin frequency domain dynamics.

## Architecture

- `lib.rs` — Main `SpectralCompressorPlugin`, implements `InPlacePlugin` trait
- `params.rs` — Parameter definitions

## Key Public API

- `SpectralCompressorPlugin` implementing `InPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-spectral-compressor
```

## Important Notes

- Operates in STFT domain — processes each frequency bin independently
- Much more granular than multiband compression (hundreds of bands vs 2-5)
- Higher latency due to FFT windowing
- Useful for transparent dynamics control and spectral taming
