# math-wave

Analytical solutions for wave and Helmholtz equations.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
math-wave = { path = "../math-wave" }
```

## Features

- **1D solutions**: Plane waves, standing waves, damped waves
- **2D solutions**: Cylinder scattering (Bessel/Hankel series)
- **3D solutions**: Sphere scattering (Mie theory)
- **Special functions**: Spherical Bessel/Hankel, Legendre polynomials
- **Green's functions**: Helmholtz kernel and derivatives

## Usage

### 1D Plane Wave

```rust
use math_wave::analytical::plane_wave_1d;
use std::f64::consts::PI;

let wave = plane_wave_1d(1.0, 0.0, 2.0 * PI, 100);
assert_eq!(wave.pressure.len(), 100);
```

### 3D Sphere Scattering

```rust
use math_wave::analytical::sphere_scattering_3d;
use std::f64::consts::PI;

let scatter = sphere_scattering_3d(1.0, 1.0, 20, vec![2.0], vec![0.0, PI / 2.0]);
assert!(scatter.pressure[0].norm() > 0.0);
```

### Green's Functions

The Helmholtz Green's function for 3D:

```
G(x, y) = exp(ik|x-y|) / (4π|x-y|)
```

For 2D (cylindrical):

```
G(x, y) = (i/4) H₀⁽¹⁾(k|x-y|)
```

## Modules

- `analytical` - Exact solutions for validation (1D, 2D, 3D)
- `greens` - Green's function implementations
- `special` - Spherical Bessel, Hankel, and Legendre functions

## Key Types

### Point

Represents a point in 1D, 2D, or 3D space with coordinate conversions:

```rust
use math_wave::Point;

let p = Point::new_3d(1.0, 2.0, 3.0);
let r = p.radius();
let theta = p.theta_3d();
```

### AnalyticalSolution

Contains solution data with error computation:

```rust
use math_wave::AnalyticalSolution;

let l2_err = solution.l2_error(&reference);
let linf_err = solution.linf_error(&reference);
let rel_err = solution.relative_l2_error(&reference);
```

## Use Cases

- Validating BEM (Boundary Element Method) solvers
- Validating FEM (Finite Element Method) solvers
- Reference solutions for convergence studies
- Test data generation for numerical methods
