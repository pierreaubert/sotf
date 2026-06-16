# sotf-plugin-de-esser

De-esser — sibilance reduction.

## Architecture

- `lib.rs` — Main `DeEsserPlugin`, implements `ParametricInPlacePlugin` trait
- `params.rs` — Parameter definitions (`DeEsserPluginParams`)

## Key Public API

- `DeEsserPlugin` implementing `ParametricInPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-de-esser
```

## Important Notes

- ParametricInPlacePlugin — same channel count in/out
- Targets sibilant frequencies (typically 4-10 kHz)
- Frequency-selective dynamics processing
