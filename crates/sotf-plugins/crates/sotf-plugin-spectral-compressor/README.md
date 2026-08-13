# sotf-plugin-spectral-compressor

Spectral compressor — per-bin frequency domain dynamics processing.

## What It Does

Applies dynamic range compression independently to each frequency bin in the STFT domain. Unlike a multiband compressor (2-5 bands), a spectral compressor operates on hundreds of individual frequency bins, enabling extremely transparent dynamics control. Particularly effective for taming harsh resonances without affecting surrounding frequencies.

## Features

- **Per-bin compression**: Independent dynamics for each frequency bin
- **STFT-based**: Operates in the frequency domain via Short-Time Fourier Transform
- **Transparent dynamics**: Far more granular than multiband compression
- **Spectral taming**: Reduces resonances without coloring surrounding frequencies
- **Targeted processing**: All, tonal, or transient spectral components
- **Adaptive threshold**: 500 ms programme-relative estimator with deterministic priming
- **Channel linking**: Continuous independent-to-maximum detector linking for image stability

The processor uses periodic Hann analysis/synthesis at 75% overlap and reports
one FFT frame of latency. FFT size is structural and requires host graph rebuild.
Target mode is an integer choice (`All`, `Tonal`, `Transient`); classification
masks and envelopes are independent per channel. Construction rejects unknown,
non-finite, and out-of-range state. Initialized processing supports blocks up to
16384 frames without heap allocation.

Threshold is defined as local narrowband coherent amplitude. The detector sums
five calibrated Hann bins before compression, so equal tones remain stable across
FFT sizes and fractional-bin alignment. Broadband per-bin readings intentionally
follow FFT analysis bandwidth; this control is not an SPL, LUFS, or PSD threshold.

Spectral smoothing is a reversal-invariant, edge-normalized box kernel. Its
0–100% amount maps to a radius of 0–12 bins (bin width is `sample_rate / FFT size`).
Adaptive threshold uses `exp(-hop/(0.5*sample_rate))`, primes from the first valid
spectrum, and reprimes on reset or enable. `channel_link` blends each independent
gain-reduction value toward the maximum across all channels without mixing audio.

## Architecture

```
src/
├── lib.rs                              # crate surface
├── params.rs                           # canonical specs and layout
└── lib/
    ├── spectral_compressor_plugin.rs   # host contract and streaming DSP
    ├── spectral_compressor_plugin_params.rs # strict serialized state
    ├── stft_state.rs                   # prepared circular WOLA state
    ├── misc.rs                         # compression/calibration helpers
    └── tests.rs                        # DSP property regressions
```

## Testing

```bash
cargo test -p sotf-plugin-spectral-compressor
cargo run -p sotf-plugin-spectral-compressor --features qa --bin qa-spectral-compressor
```

## License

Part of the SOTF (Sound of the Future) project.
