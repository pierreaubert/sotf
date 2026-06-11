# sotf-plugin-band-split

Split signal into frequency bands.

## Architecture

- `lib.rs` — Main plugin struct, implements `Plugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::Plugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-band-split
```

## Important Notes

- Companion to sotf-plugin-band-merge — must be used in pairs
- Output channel count = input channels × number of bands
- Uses Linkwitz-Riley crossover filters for phase-coherent band splitting
