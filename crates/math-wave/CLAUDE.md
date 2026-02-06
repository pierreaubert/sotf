# math-wave (lib: `math_audio_wave`, version: 0.3.0)

Analytical solutions for wave and Helmholtz equations.

## Purpose

Provides reference analytical solutions used to validate BEM and FEM numerical solvers.

## Key Components

- Bessel function evaluation
- Helmholtz equation analytical solutions
- Wave propagation formulas

## Dependencies

- `num-complex` - Complex number support
- `spec_math` - Special mathematical functions

## Testing

```bash
cargo test -p math-wave --lib
cargo check -p math-wave && cargo clippy -p math-wave
```
