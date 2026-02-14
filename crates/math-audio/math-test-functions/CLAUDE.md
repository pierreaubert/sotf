# math-test-functions (lib: `math_audio_test_functions`, version: 0.3.0)

Non-linear test functions for validating optimization algorithms.

## Purpose

Provides standard benchmark functions (Rosenbrock, Rastrigin, Ackley, etc.) used to test and validate the DE and other optimizers.

## Binaries

- `plot-functions` - Visualize test function landscapes

## Features

- `plotly_static` - Static plot generation

## Testing

```bash
cargo test -p math-test-functions --lib
cargo check -p math-test-functions && cargo clippy -p math-test-functions
```
