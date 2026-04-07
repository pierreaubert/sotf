# sotf-plugin-spectral-compressor

Spectral compressor — per-bin frequency domain dynamics processing.

## What It Does

Applies dynamic range compression independently to each frequency bin in the STFT domain. Unlike a multiband compressor (2-5 bands), a spectral compressor operates on hundreds of individual frequency bins, enabling extremely transparent dynamics control. Particularly effective for taming harsh resonances without affecting surrounding frequencies.

## Features

- **Per-bin compression**: Independent dynamics for each frequency bin
- **STFT-based**: Operates in the frequency domain via Short-Time Fourier Transform
- **Transparent dynamics**: Far more granular than multiband compression
- **Spectral taming**: Reduces resonances without coloring surrounding frequencies

## Architecture

```
src/
├── lib.rs     # SpectralCompressorPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-spectral-compressor
```

## License

Part of the SOTF (Sound of the Future) project.
