<!-- markdownlint-disable-file MD013 -->

# AutoEQ Differential Evolution

This crate provides a pure Rust implementation of Differential Evolution (DE) global optimization algorithm with advanced features.

## Features

- **Pure Rust Implementation**: No external dependencies for core optimization
- **Multiple DE Strategies**: Various mutation and crossover strategies
- **Constraint Handling**: Linear and nonlinear constraint support
- **Adaptive Parameters**: Self-adjusting F and CR parameters
- **Evaluation Recording**: Track optimization progress and convergence
- **Visualization Tools**: Plot test functions and optimization traces

## Optimization Strategies

### Mutation Strategies

- `DE/rand/1`: `x_trial = x_r1 + F * (x_r2 - x_r3)`
- `DE/best/1`: `x_trial = x_best + F * (x_r1 - x_r2)`
- `DE/current-to-best/1`: Combines current and best vectors
- `DE/rand/2`: Uses five random vectors for mutation

### Crossover Strategies

- **Binomial**: Random parameter-wise crossover
- **Exponential**: Sequential parameter crossover

## Usage

```rust
use autoeq_de::{differential_evolution, DEConfig, Strategy, Mutation};
use ndarray::Array1;

// Example objective function (Rosenbrock)
let objective = |x: &Array1<f64>| {
    let a = 1.0;
    let b = 100.0;
    (a - x[0]).powi(2) + b * (x[1] - x[0].powi(2)).powi(2)
};

// Define bounds for 2D problem
let bounds = vec![(-5.0, 5.0), (-5.0, 5.0)];

let config = DEConfig {
    strategy: Strategy::Rand1Bin,
    maxiter: 1000,
    popsize: 50,
    mutation: Mutation::Factor(0.8),
    recombination: 0.9,
    seed: Some(42),
    ..Default::default()
};

let result = differential_evolution(&objective, &bounds, config);
println!("Best solution: {:?}", result.x);
println!("Best fitness: {}", result.fun);
```

## Constraint Support

### Linear Constraints

```rust
use autoeq_de::{LinearConstraintHelper, DEConfig};
use ndarray::{Array1, Array2};

// Linear constraint: x1 + x2 <= 1.0
let constraint = LinearConstraintHelper {
    a: Array2::from_shape_vec((1, 2), vec![1.0, 1.0]).unwrap(),
    lb: Array1::from_vec(vec![f64::NEG_INFINITY]),
    ub: Array1::from_vec(vec![1.0]),
};

// Apply to configuration with penalty weight
let mut config = DEConfig::default();
constraint.apply_to(&mut config, 1000.0); // penalty weight
```

### Nonlinear Constraints

```rust
let nonlinear_constraint = |x: &[f64]| -> f64 {
    x[0].powi(2) + x[1].powi(2) - 1.0 // circle constraint
};
```

## Visualization

The crate includes a `plot_functions` binary for visualizing test functions and optimization traces:

```bash
# Plot test functions as contour plots
cargo run --bin plot_functions -- --functions rosenbrock,sphere

# Show optimization traces from CSV files
cargo run --bin plot_functions -- --csv-dir traces/ --show-traces
```

## Integration

This crate is part of the AutoEQ ecosystem:

- Used by `autoeq` for filter parameter optimization
- Integrates with `autoeq-testfunctions` for validation
- Works with `autoeq-iir` for audio filter optimization

## Examples

The crate includes several example programs demonstrating different DE capabilities:

- `basic_de`: Simple unconstrained optimization
- `linear_constraints`: Linear constraint handling
- `nonlinear_constraints`: Complex constraint optimization

## [References](./REFERENCES.md)

## License

GPL-3.0-or-later
