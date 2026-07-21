# sotf-plugin-linear-phase-eq

Parametric FIR EQ with selectable linear or minimum phase.

## What It Does

A parametric equalizer that uses FIR (Finite Impulse Response) filters instead of traditional IIR biquads. This eliminates phase distortion entirely — all frequencies are delayed by the same amount. The tradeoff is higher latency compared to a minimum-phase EQ.

## Features

- **Zero phase distortion**: All frequencies delayed equally
- **Minimum-phase mode**: Low-latency, causal response without linear-phase pre-ringing
- **FIR convolution**: Uses frequency-domain convolution for efficient processing
- **Parametric bands**: Standard frequency, Q, and gain controls
- **High precision**: Ideal for mastering and critical listening
- **Auto Gain**: Normalizes the FIR's DC gain to unity. It is a predictable
  reference-point correction, not a perceptual loudness match.

## When to Use

- Crossover alignment where phase coherence matters
- Mastering chains where phase transparency is critical
- Situations where latency is acceptable (not live monitoring)

For the lowest CPU cost, prefer `sotf-plugin-eq` (minimum-phase IIR).

## Architecture

```
src/
├── lib.rs     # LinearPhaseEqPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-linear-phase-eq
```

## License

Part of the SOTF (Sound of the Future) project.
