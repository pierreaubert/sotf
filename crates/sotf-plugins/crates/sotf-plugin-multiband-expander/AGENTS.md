# sotf-plugin-multiband-expander

Broadband (1 band) and multiband (2-5 bands) dynamic range expansion.

## Architecture

- `lib.rs` — Main plugin struct, implements `ParametricInPlacePlugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::ParametricInPlacePlugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-multiband-expander
```

## Important Notes

- Uses LR4 crossovers for phase-coherent band splitting
- Re-exports `CROSSOVER_PRESETS` from sotf-host for standard frequency splits
- Each band has independent threshold, ratio, attack, release parameters
- `expander` is a true one-band factory path; `multiband_expander` requires 2-5 bands.
- `num_bands` and `processing_mode` are structural and must never allocate in a live setter.
- Spectral mode is dual-Hann, 75% overlap, `N/4` hop, and `N` samples latency.
- Sidechain HPF affects detector samples only; lookahead applies uniformly to bypassed bands.
