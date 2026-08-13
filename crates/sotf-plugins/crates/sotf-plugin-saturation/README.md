# sotf-plugin-saturation

Saturation / Harmonic Exciter — adds warmth and harmonic richness.

## What It Does

Adds harmonic overtones through deterministic nonlinear waveshaping. The Tube
and Tape labels describe static, tube- and tape-flavoured curves; they are not
physical circuit, bias, hysteresis, or magnetic-head models.

## Features

- **Multiple saturation curves**: Soft clip, odd-symmetric Tube-style curve,
  memoryless Tape-style sigmoid, split-band Exciter, and a bounded Asymmetric
  curve with controlled even harmonics
- **Anti-aliased processing**: ADAA (Anti-Derivative Anti-Aliasing) reduces digital artifacts
- **Drive control**: From subtle warmth to heavy distortion
- **Mix control**: Blend dry and saturated signal

## Asymmetric model contract

The Asymmetric mode is an explicit memoryless family, not analog circuit
emulation. It subtracts the zero-input value from a bias-shifted `tanh`, then
normalizes the positive and negative rails independently to +1 and -1. Tone
maps the dimensionless bias from 0.08 to 0.40. The result is zero at zero input,
bounded, deterministic, and intentionally produces even harmonics. Use 2x or
4x host oversampling for alias suppression and the DC Block option when
programme-dependent offset is undesirable.

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
