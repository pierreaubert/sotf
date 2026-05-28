# sotf-plugin-fir-designer

FIR magnitude and phase designer with parametric target bands.

## What It Does

A FIR (Finite Impulse Response) equalizer that turns parametric target bands into a designed FIR response. It can run as a linear-phase EQ for phase-coherent work, or as a minimum-phase FIR when lower perceived latency is more important.

## Features

- **Phase mode**: Linear or minimum-phase FIR design
- **FIR convolution**: Uses frequency-domain convolution for efficient processing
- **Parametric bands**: Standard frequency, Q, and gain controls
- **High precision**: Ideal for mastering and critical listening
- **Auto Gain**: Normalizes the FIR's DC gain to unity. It is a predictable
  reference-point correction, not a perceptual loudness match.

## When to Use

- Crossover alignment where phase coherence matters
- Mastering chains where phase transparency is critical
- Situations where latency is acceptable (not live monitoring)

For low-latency use cases, prefer `sotf-plugin-eq` (minimum-phase IIR).

## Architecture

```
src/
├── lib.rs     # FirDesignerPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-fir-designer
```

## License

Part of the SOTF (Sound of the Future) project.
