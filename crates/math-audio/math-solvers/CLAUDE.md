# math-solvers (lib: `math_audio_solvers`, version: 0.3.4)

High-performance iterative linear solvers for BEM and FEM.

## Key Components

- GMRES iterative solver
- Algebraic Multigrid (AMG) preconditioner
- Sparse matrix support

## Features

- `native` (default) - BLAS/LAPACK + rayon for performance
- `parallel` - Parallel processing
- `wasm` - WebAssembly support

## Testing

```bash
cargo test -p math-solvers --lib
cargo check -p math-solvers && cargo clippy -p math-solvers
```

## Examples

```bash
cargo run --release --example <example_name> -p math-solvers
```

## Notes

- Used by both `math-bem` and `math-fem` for solving large linear systems
- WASM builds must disable `native` and enable `wasm`
