# autoeq-roomsim (lib: `autoeq-roomsim`, version: 0.3.1)

Room acoustic simulator with BEM solver, supporting WASM compilation.

## Purpose

Simulates room acoustics using Boundary Element Method for room EQ optimization and visualization.

## Crate Types

- `cdylib` - Dynamic library (for WASM)
- `rlib` - Rust library

## Module Layout

- `bem_solver.rs` - BEM solver implementation
- `scattering_objects.rs` - Room object modeling
- `plotting/` - Visualization output

## WASM Support

Compiles to WebAssembly with parallel processing via wasm-bindgen-rayon.

## Dependencies

- `wasm-bindgen`, `wasm-bindgen-rayon` - WASM interop
- `math-bem` (with wasm feature) - BEM solver backend

## Testing

```bash
cargo test -p autoeq-roomsim --lib
cargo check -p autoeq-roomsim && cargo clippy -p autoeq-roomsim
```

## Notes

- When building for WASM, disable `native` features and enable `wasm` feature
- Uses `math-bem` internally for the actual solver
