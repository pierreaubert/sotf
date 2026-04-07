# sotf-plugin-mono-to-stereo

Mono to stereo conversion with optional widening.

## Architecture

- `lib.rs` — Main plugin struct, implements `Plugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::Plugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-mono-to-stereo
```

## Important Notes

- Plugin trait (not InPlacePlugin) — changes channel count (1 → 2)
- Simple duplication or optional decorrelation-based widening
