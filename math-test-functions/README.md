<!-- markdownlint-disable-file MD013 -->

# Optimization Test Functions Library

A comprehensive collection of **56+ test functions** for optimization algorithm benchmarking and validation, organized into logical modules for easy navigation and maintenance.

## Function Categories

[Plots per function](./docs/interactive_plots.html)

### **Unimodal Functions** (Single Global Optimum)

Perfect for testing **convergence speed** and **precision**:

- `sphere`, `rosenbrock`, `powell`, `dixons_price`
- `zakharov`, `booth`, `matyas`, `sum_squares`
- `elliptic`, `cigar`, `tablet`, `discus` (ill-conditioned)
- `ridge`, `sharp_ridge`, `brown`, `exponential`

### **Multimodal Functions** (Multiple Local Minima)

Test **global search** and **exploration** capabilities:

- `ackley`, `rastrigin`, `griewank`, `schwefel`
- `hartman_3d`, `hartman_4d`, `hartman_6d`
- `michalewicz`, `alpine_n1`, `alpine_n2`
- `branin`, `goldstein_price`, `six_hump_camel`
- `eggholder`, `holder_table`, `cross_in_tray`

### **Constrained Functions**

For **constrained optimization** algorithms:

- `keanes_bump_objective` with constraints
- `rosenbrock_disk_constraint`
- `binh_korn_*` with multiple constraints
- `mishras_bird_*` functions

### **Composite & Modern Functions**

**Hybrid** and **recent competition** functions:

- `expanded_griewank_rosenbrock`
- `xin_she_yang_n1`, `xin_she_yang_n2`, etc.
- `happycat`, `katsuura`, `vincent`
- `gramacy_lee_2012`, `forrester_2008`

## Usage Examples

### Basic Function Evaluation

```rust
use ndarray::Array1;
use autoeq_testfunctions::*;

// Evaluate sphere function
let x = Array1::from_vec(vec![1.0, 2.0]);
let result = sphere(&x); // Returns 5.0 (1² + 2²)

// Evaluate Rosenbrock function
let x = Array1::from_vec(vec![1.0, 1.0]);
let result = rosenbrock(&x); // Returns 0.0 (global minimum)
```

### Function Metadata and Bounds

```rust
use autoeq_testfunctions::{get_function_metadata, get_function_bounds, get_function_bounds_2d};
// Get all function metadata
let metadata = get_function_metadata();
println!("Available functions: {}", metadata.len());

// Get bounds for a specific function
let bounds = get_function_bounds("ackley").unwrap();
println!("Ackley bounds: {:?}", bounds);

// Get 2D bounds array (for compatibility)
let bounds_2d = get_function_bounds_2d("sphere", (-5.0, 5.0));
```

### Working with Different Categories

```rust
use ndarray::Array1;
use autoeq_testfunctions::*;

let x = Array1::from_vec(vec![1.0, 1.0]);
// Unimodal functions (good for convergence testing)
let result1 = elliptic(&x);    // Ill-conditioned
let result2 = rosenbrock(&x);  // Valley-shaped

// Multimodal functions (good for global search testing)
let result3 = ackley(&x);      // Highly multimodal
let result4 = rastrigin(&x);   // Many local minima

// Modern benchmarks
let result5 = happycat(&x);    // Recent CEC function
let result6 = katsuura(&x);    // Fractal-like landscape
```

## Function Properties

### **Difficulty Levels**

- **Easy**: `sphere`, `booth`, `matyas` - Good for initial testing
- **Medium**: `rosenbrock`, `ackley`, `griewank` - Standard benchmarks
- **Hard**: `schwefel`, `eggholder`, `katsuura` - Challenging landscapes
- **Extreme**: `holder_table`, `cross_in_tray` - Very difficult optimization

### **Dimensionality Support**

- **1D**: `gramacy_lee_2012`, `forrester_2008`
- **2D Fixed**: `branin`, `goldstein_price`, `six_hump_camel`
- **N-Dimensional**: `sphere`, `rosenbrock`, `ackley`, `rastrigin`
- **High-D Specific**: `elliptic`, `bent_cigar`, `katsuura`

### **Special Properties**

- **Ill-conditioned**: `elliptic`, `cigar`, `tablet`, `discus`
- **Separable**: `sphere`, `sum_squares`, `alpine_n1`
- **Non-separable**: `rosenbrock`, `ackley`, `griewank`
- **Discontinuous**: `step`, `de_jong_step2`
- **Constrained**: `keanes_bump_*`, `binh_korn_*`

## Benchmarking Guide

### **Algorithm Testing Workflow**

1. **Start Simple**: Test on `sphere` and `rosenbrock`
2. **Add Multimodality**: Try `ackley` and `rastrigin`
3. **Test Scalability**: Use N-dimensional functions
4. **Challenge Mode**: `schwefel`, `eggholder`, `katsuura`
5. **Constraints**: `keanes_bump_*` for constrained algorithms

### **Performance Metrics**

- **Convergence Speed**: Unimodal functions
- **Global Search**: Multimodal functions
- **Precision**: Functions with known exact minima
- **Robustness**: Ill-conditioned and noisy functions
- **Scalability**: High-dimensional variants

## Build & Test

```bash
# Build the library
cargo build --release

# Run all tests
cargo test --release

# Test specific examples
cargo run --example test_new_sfu_functions
cargo run --example test_additional_functions

# Check code quality
cargo check
cargo clippy
cargo fmt
```

## 🏗️ Architecture

The library is organized into modular components:

```text
src/
├── lib.rs                    # Main library with utilities and metadata
└── functions/
    ├── mod.rs               # Module exports and organization
    └── *.rs                 # 1 file per function
```
