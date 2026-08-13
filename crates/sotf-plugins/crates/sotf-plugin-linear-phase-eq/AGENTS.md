# sotf-plugin-linear-phase-eq

FIR EQ — parametric EQ with selectable linear or minimum-phase FIR convolution.

## Architecture

- `lib.rs` — Main `LinearPhaseEqPlugin`, implements `ParametricParametricInPlacePlugin` trait
- `params.rs` — Parameter definitions

## Key Public API

- `LinearPhaseEqPlugin` implementing `ParametricParametricInPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-linear-phase-eq
```

## Important Notes

- Linear phase preserves phase coherence and reports `fir_length / 2 + 32` samples, including partition latency.
- Minimum phase reports the 32-sample partition latency and avoids pre-ringing.
- FIR convolution remains more CPU-intensive than the standard IIR EQ.
