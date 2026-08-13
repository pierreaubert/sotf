# sotf-plugin-saturation

Saturation / Harmonic Exciter — adds warmth and harmonic richness.

## What It Does

Adds harmonic overtones through deterministic nonlinear waveshaping. The Tube
and Tape labels describe static, tube- and tape-flavoured curves; they are not
physical circuit, bias, hysteresis, or magnetic-head models.

## Features

- **Multiple saturation curves**: Soft clip, odd-symmetric Tube-style curve,
  memoryless Tape-style sigmoid, and split-band Exciter
- **Anti-aliased processing**: ADAA (Anti-Derivative Anti-Aliasing) reduces digital artifacts
- **Drive control**: From subtle warmth to heavy distortion
- **Mix control**: Blend dry and saturated signal

## Architecture

```
src/
├── lib.rs     # SaturationPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-saturation
```

## License

Part of the SOTF (Sound of the Future) project.
