# sotf-plugin-dynamic-eq

Dynamic EQ — frequency-selective dynamics processing.

## What It Does

A parametric EQ where each filter band only activates when the signal in that frequency range crosses a threshold. Unlike a static EQ that applies constant gain, a dynamic EQ adapts in real-time — useful for taming resonances that only appear at certain levels or for frequency-dependent compression.

## Features

- **Per-band dynamics**: Each EQ band has its own threshold, ratio, attack, and release
- **Parametric EQ base**: Standard frequency, Q, and gain per band
- **Adaptive processing**: Gain changes dynamically based on signal level

## Architecture

```
src/
├── lib.rs     # DynamicEqPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-dynamic-eq
```

## License

Part of the SOTF (Sound of the Future) project.
