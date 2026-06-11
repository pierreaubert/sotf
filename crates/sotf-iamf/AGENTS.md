# sotf-iamf (lib: `sotf_iamf`)

Pure Rust IAMF v1.1.0 decoder -- bitstream parsing and rendering to target speaker layouts.

## Module Layout

- `codec/` -- Audio codec implementations
- `obu/` -- OBU (Open Bitstream Unit) parsing
- `renderer/` -- Element rendering to speaker layouts
- `mixer.rs` -- Mix state for combining audio elements
- `types.rs` -- IAMF bitstream types
- `error.rs` -- Error types

## Key Types

- `IamfDecoder` -- Main decoder. Pre-allocates buffers in `open()` for allocation-free decoding.

## Dependencies

- `sotf-host` -- Speaker configurations
- `sotf-plugin-ambisonics` -- Ambisonics rendering

## Testing

```bash
cargo test -p sotf-iamf --lib
cargo check -p sotf-iamf && cargo clippy -p sotf-iamf
```

## Important Notes

- Enabled in `sotf-engine` via the `iamf` feature flag
- No C/C++ dependencies
