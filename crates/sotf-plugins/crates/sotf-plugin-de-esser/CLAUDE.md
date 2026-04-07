# sotf-plugin-de-esser

De-esser — sibilance reduction.

## Architecture

- `lib.rs` — Main `DeEsserPlugin`, implements `InPlacePlugin` trait
- `params.rs` — Parameter definitions (`DeEsserPluginParams`)

## Key Public API

- `DeEsserPlugin` implementing `InPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-de-esser
```

## Important Notes

- InPlacePlugin — same channel count in/out
- Targets sibilant frequencies (typically 4-10 kHz)
- Frequency-selective dynamics processing
