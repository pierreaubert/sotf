# math-bem (lib: `math_audio_bem`, version: 0.3.2)

Boundary Element Method solver for acoustic scattering and room simulation.

## Key Features

- High-performance BEM solver for acoustic problems
- GMRES iterative solver with AMG preconditioning
- Memory optimization with out-of-core mode (disk streaming for large problems)
- HRTF generation capability
- WASM support with rayon-based parallel processing

## Features

- `native` (default) - BLAS/LAPACK support for performance
- `cli` - Command-line interface
- `parallel` - Parallel matrix assembly
- `memory-optimized` - Memory management for large problems
- `out-of-core` - Disk streaming for problems exceeding RAM
- `wasm` - WebAssembly compilation

## Binaries

- `roomsim` - Room simulator (requires `cli`, `native`, `parallel`, `memory-optimized`, `out-of-core`)
- `qa-suite` - Quality assurance tests

## Dependencies

- `math-solvers` - GMRES solver
- `math-wave` - Analytical solutions for validation
- `math-xem-common` - Shared types with FEM

## Testing

```bash
cargo test -p math-bem --lib
cargo check -p math-bem && cargo clippy -p math-bem
```

## Examples

Examples require the `native` feature:
```bash
cargo run --release --example <example_name> -p math-bem --features native
```

## Notes

- WASM builds must disable `native` features and enable `wasm`
- The out-of-core mode allows solving problems larger than available RAM
- Uses `math-solvers` for the underlying GMRES linear system solve
