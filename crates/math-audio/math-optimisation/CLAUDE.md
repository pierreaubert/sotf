# math-optimisation (lib: `math_audio_optimisation`, version: 0.4.1)

Differential Evolution optimizer with Levenberg-Marquardt local refinement.

## Purpose

Provides the primary global optimization algorithm for AutoEQ, forked from SciPy's DE implementation and ported to Rust.

## Key Features

- Custom DE implementation with multiple mutation strategies (rand/1, best/1, current-to-best/1, rand/2, current-to-pbest/1)
- L-SHADE: linear population reduction with adaptive F/CR and external archive
- Levenberg-Marquardt local optimizer for nonlinear least-squares refinement
- Constraint handling (linear penalty stacking, nonlinear constraints)
- Parallel population evaluation (rayon)
- Binomial and exponential crossover
- Latin hypercube and random initialization
- Evaluation recording and convergence visualization
- Function registry for test function metadata

## Module Layout

| Module | Description |
|---|---|
| `mod.rs` | Main DE implementation (`DEConfigBuilder`, `DEConfig`, `DEResult`) |
| `levenberg_marquardt.rs` | LM local optimizer (`LMConfig`, `levenberg_marquardt()`) |
| `lshade.rs` | L-SHADE configuration |
| `parallel_eval.rs` | Parallel population evaluation |
| `function_registry.rs` | Test function registry |
| `recorder.rs` | Optimization progress recording |
| `external_archive.rs` | External archive for diversity |
| `mutant_*.rs` | Mutation strategies (rand1, best1, current_to_best1, rand2, current_to_pbest1, adaptive) |
| `crossover_*.rs` | Crossover strategies (binomial, exponential) |
| `init_*.rs` | Initialization (latin_hypercube, random) |
| `stack_linear_penalty.rs` | Linear constraint penalty stacking |
| `metadata.rs` | Metadata structures |
| `error.rs` | Error types (`DEError`) |

## Binaries

- `plot-de` -- Visualization of DE convergence traces
- `benchmark-convergence` -- Convergence benchmarking and comparison
- `run-de` -- CLI optimizer for testing

## Features

- `plotly_static` -- Static plot generation for convergence visualization

## Testing

```bash
cargo test -p math-optimisation --lib
cargo check -p math-optimisation && cargo clippy -p math-optimisation
```

## Examples

```bash
cargo run --release --example optde_basic -p math-optimisation
cargo run --release --example optde_adaptive_demo -p math-optimisation
cargo run --release --example optde_linear_constraints -p math-optimisation
cargo run --release --example optde_nonlinear_constraints -p math-optimisation
cargo run --release --example optde_parallel -p math-optimisation
```
