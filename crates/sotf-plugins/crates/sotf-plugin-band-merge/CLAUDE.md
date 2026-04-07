# sotf-plugin-band-merge

Merge frequency bands back together.

## Architecture

- `lib.rs` — Main plugin struct, implements `Plugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::Plugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-band-merge
```

## Important Notes

- Companion to sotf-plugin-band-split — must be used in pairs
- Input channel count = base channels × number of bands; output = base channels
