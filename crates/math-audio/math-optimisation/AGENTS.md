# math-optimisation (lib: `math_audio_optimisation`, version: 0.3.2)

Differential Evolution optimizer with interfaces to NLOPT and MetaHeuristics.

## Purpose

Provides the primary global optimization algorithm for AutoEQ, forked from SciPy's DE implementation and ported to Rust.

## Key Features

- Custom DE implementation with multiple mutation strategies
- Constraint handling for bounded optimization
- Interface adapters for NLOPT and MetaHeuristics backends
- Convergence benchmarking tools

## Binaries

- `plot_de` - Visualization of DE convergence
- `benchmark_convergence` - Convergence testing and comparison
- `run-de` - CLI optimizer for testing

## Features

- `plotly_static` - Static plot generation for convergence visualization

## Testing

```bash
cargo test -p math-optimisation --lib
cargo check -p math-optimisation && cargo clippy -p math-optimisation
```

## Examples

```bash
cargo run --release --example <example_name> -p math-optimisation
```
