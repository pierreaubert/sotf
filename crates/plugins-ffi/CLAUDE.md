# plugins-ffi

C FFI bindings for Audio Unit plugin integration.

## Purpose

Exposes the Rust plugin system as a C-compatible static/dynamic library for use by the Swift Audio Unit wrapper (`plugins-au`).

## Crate Types

- `staticlib` - Static library for linking
- `cdylib` - Dynamic library

## Build

Uses `cbindgen` to auto-generate C header files from the Rust API.

## Platform

macOS only.

## Testing

```bash
cargo check -p plugins-ffi && cargo clippy -p plugins-ffi
```

## Notes

- This is the bridge between Rust plugins and the Swift Audio Unit in `plugins-au`
- The generated C headers must stay in sync with the Rust API
