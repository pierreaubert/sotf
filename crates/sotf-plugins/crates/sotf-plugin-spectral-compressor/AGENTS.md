# sotf-plugin-spectral-compressor

Spectral compressor — per-bin frequency domain dynamics.

## Architecture

- `lib.rs` — Main `SpectralCompressorPlugin`, implements `ParametricInPlacePlugin` trait
- `params.rs` — Parameter definitions

## Key Public API

- `SpectralCompressorPlugin` implementing `ParametricInPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-spectral-compressor
```

## Important Notes

- Periodic dual-Hann WOLA uses 75% overlap, an `N/4` hop, `1/(1.5N)`
  normalization, and exactly one FFT frame of reported latency.
- Input history and output overlap-add are circular. Plans and all scratch are
  prepared outside `process_in_place`; initialized blocks are bounded at 16,384 frames.
- Threshold is calibrated as local narrowband coherent amplitude using five-bin
  Hann energy. This stabilizes tones across bin alignment; broadband per-bin level
  intentionally follows analysis bandwidth rather than pretending to be full-band LUFS.
- Spectral smoothing is an edge-normalized symmetric box with a 0–12-bin radius.
- Adaptive threshold uses a sample-rate/FFT-independent 500 ms time constant and
  primes from the first valid spectrum.
- `channel_link=0` is independent; `1` applies maximum per-bin gain reduction to
  every channel. FFT size is structural and requires host graph reconstruction.
