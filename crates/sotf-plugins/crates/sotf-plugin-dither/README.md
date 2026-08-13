# sotf-plugin-dither

TPDF dither with F-weighted noise shaping for bit-depth reduction.

## What It Does

When reducing audio bit depth (e.g., 24-bit to 16-bit for CD), quantization distortion is introduced. Dithering adds a small amount of shaped noise to eliminate this distortion, replacing it with a low-level, perceptually benign noise floor. F-weighted noise shaping pushes the dither noise into frequency ranges where human hearing is least sensitive.

## Features

- **TPDF dither**: Triangular Probability Density Function for optimal quantization noise decorrelation
- **F-weighted noise shaping**: Pushes noise to less audible frequencies
- **Configurable target bit depth**: 16, 20, and 24 bit targets

## Architecture

```
src/
├── lib.rs     # DitherPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-dither
```

## License

Part of the SOTF (Sound of the Future) project.
