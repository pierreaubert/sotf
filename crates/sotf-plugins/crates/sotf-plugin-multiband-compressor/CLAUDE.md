# sotf-plugin-multiband-compressor

Multiband dynamic range compression (2-5 bands).

## Architecture

- `lib.rs` — Main plugin struct, implements `InPlacePlugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::InPlacePlugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-multiband-compressor
```

## Important Notes

- Uses LR4 crossovers for phase-coherent band splitting
- Re-exports `CROSSOVER_PRESETS` from sotf-host for standard frequency splits
- Each band has independent threshold, ratio, attack, release, knee, makeup gain
