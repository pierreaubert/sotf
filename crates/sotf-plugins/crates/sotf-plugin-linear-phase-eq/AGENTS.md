# sotf-plugin-linear-phase-eq

Linear-phase EQ — parametric EQ with FIR convolution for zero phase distortion.

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

- Uses FIR filters instead of IIR biquads — zero phase distortion but introduces latency
- Higher latency than standard EQ due to FIR convolution
- Useful when phase coherence is critical (e.g., crossover alignment, mastering)
