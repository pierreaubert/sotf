# math-fem

Multigrid FEM solver for the Helmholtz equation.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
math-fem = { path = "../math-fem" }
```

## Features

- **2D and 3D meshes**: Triangles, quadrilaterals, tetrahedra, hexahedra
- **Lagrange elements**: P1, P2, P3 polynomial basis functions
- **Boundary conditions**: Dirichlet, Neumann, Robin, PML
- **Multigrid solver**: V-cycle, W-cycle with geometric coarsening
- **Adaptive refinement**: h-refinement with residual-based error estimation

## Usage

```rust
use fem::{mesh, boundary::BoundaryConditions};

// Create a 2D mesh
let mesh = mesh::unit_square_triangles(10);

// Create a 3D mesh
let mesh_3d = mesh::unit_cube_tetrahedra(5);
```

## Modules

### mesh

Mesh generators for common domains:

```rust
use fem::mesh::*;

// 2D meshes
let rect = rectangular_mesh_triangles(0.0, 1.0, 0.0, 1.0, 10, 10);
let quads = rectangular_mesh_quads(0.0, 1.0, 0.0, 1.0, 10, 10);
let circle = circular_mesh_triangles(0.0, 0.0, 1.0, 5, 16);
let annulus = annular_mesh_triangles(0.0, 0.0, 0.5, 2.0, 4, 16);

// 3D meshes
let box_tet = box_mesh_tetrahedra(0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 5, 5, 5);
let box_hex = box_mesh_hexahedra(0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 5, 5, 5);

// Convenience functions
let square = unit_square_triangles(10);
let cube = unit_cube_tetrahedra(5);
```

### basis

Lagrange polynomial basis functions:

- P1, P2, P3 for simplices (triangles, tetrahedra)
- Q1, Q2 for quads/hexes

### boundary

Boundary condition handling:

```rust
use fem::boundary::BoundaryConditions;
use num_complex::Complex64;

let mut bcs = BoundaryConditions::new();

// Dirichlet: u = g on boundary
bcs.add_dirichlet(1, |x, y, z| Complex64::new(1.0, 0.0));

// Neumann: du/dn = h on boundary
bcs.add_neumann(2, |x, y, z| Complex64::new(0.0, 0.0));

// Robin: du/dn + alpha*u = g on boundary
bcs.add_robin(3,
    |x, y, z| Complex64::new(1.0, 0.0),  // alpha
    |x, y, z| Complex64::new(0.0, 0.0),  // g
);
```

### assembly

Matrix assembly for FEM:

- Stiffness matrix assembly
- Mass matrix assembly
- Helmholtz matrix (K - k²M)

### quadrature

Gaussian quadrature rules for numerical integration.

### multigrid

Geometric multigrid solver:

- V-cycle, W-cycle, F-cycle methods
- Gauss-Seidel smoothing
- Linear interpolation transfer operators

## Element Types

| Element | Dimension | Nodes (P1) | Nodes (P2) |
|---------|-----------|------------|------------|
| Triangle | 2D | 3 | 6 |
| Quadrilateral | 2D | 4 | 9 |
| Tetrahedron | 3D | 4 | 10 |
| Hexahedron | 3D | 8 | 27 |

## Feature Flags

- `native` (default) - Enables rayon parallelism and native BLAS
- `parallel` - Enables rayon for parallel assembly

## Dependencies

- `math-solvers` - Sparse linear algebra solvers
- `ndarray` - N-dimensional arrays
- `num-complex` - Complex number support
- `rayon` (optional) - Parallel processing
