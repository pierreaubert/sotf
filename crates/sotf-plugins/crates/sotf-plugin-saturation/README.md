# sotf-plugin-saturation

Saturation / Harmonic Exciter — adds warmth and harmonic richness.

## What It Does

Adds harmonic overtones to audio through nonlinear waveshaping, simulating the pleasing distortion characteristics of analog equipment (tape machines, tube amplifiers). At low drive levels, adds subtle warmth; at higher levels, produces audible distortion.

## Features

- **Multiple saturation curves**: Soft clip, hard clip, tape emulation, tube emulation
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
