# math-fem (lib: `math_audio_fem`, version: 0.3.5)

Finite Element Method solver for the Helmholtz equation in acoustic simulation.

## Key Features

- Multigrid FEM solver
- Helmholtz equation for room acoustics
- Acoustic field computation

## Features

- `native` (default) - Rayon parallelism
- `cli` - Command-line interface
- `parallel` - Parallel processing

## Binaries

- `roomsim-fem` - FEM-based room simulator
- `qa-suite` - Quality assurance tests

## Benchmarks

```bash
cargo bench -p math-fem -- helmholtz_3d_scaling
```

## Testing

```bash
cargo test -p math-fem --lib
cargo check -p math-fem && cargo clippy -p math-fem
```

## Dependencies

- `math-solvers` - Linear system solvers
- `math-wave` - Analytical solutions for validation
- `math-xem-common` - Shared types with BEM
