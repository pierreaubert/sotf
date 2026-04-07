# math-test-functions (lib: `math_audio_test_functions`, version: 0.4.0)

56+ non-linear test functions for validating optimization algorithms.

## Purpose

Provides standard benchmark functions (Rosenbrock, Rastrigin, Ackley, etc.) used to test and validate the DE and other optimizers in `math-optimisation`.

## Key Types

- `FunctionMetadata` -- bounds, global minima, constraints, description, multimodal flag, supported dimensions
- `get_function_metadata()` -- returns all function metadata as `HashMap<String, FunctionMetadata>`
- `get_function_bounds(name)` / `get_function_bounds_2d(name, default)` -- bounds lookup

## Function Categories

- **Unimodal**: sphere, rosenbrock, powell, dixons_price, zakharov, booth, matyas, sum_squares, elliptic, cigar, tablet, discus, ridge, sharp_ridge, brown, exponential
- **Multimodal**: ackley, rastrigin, griewank, schwefel, hartman_3d/4d/6d, michalewicz, alpine_n1/n2, branin, goldstein_price, six_hump_camel, eggholder, holder_table, cross_in_tray
- **Constrained**: keanes_bump, rosenbrock_disk, binh_korn, mishras_bird
- **Composite/Modern**: expanded_griewank_rosenbrock, xin_she_yang_n1-n4, happycat, katsuura, vincent, gramacy_lee_2012, forrester_2008

## Architecture

```
src/
  lib.rs                -- Main library with metadata registry and utility functions
  functions/
    mod.rs              -- Module exports (1 file per function)
    *.rs                -- 82+ individual function implementations
```

## Binaries

- `plot-functions` -- Visualize test function landscapes as contour plots

## Features

- `plotly_static` -- Static plot generation

## Testing

```bash
cargo test -p math-test-functions --lib
cargo check -p math-test-functions && cargo clippy -p math-test-functions
```

## Examples

```bash
cargo run --release --example test_hartman_4d -p math-test-functions
cargo run --release --example test_new_sfu_functions -p math-test-functions
cargo run --release --example test_additional_functions -p math-test-functions
```
