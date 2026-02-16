# math-fem

Multigrid Finite Element Method (FEM) solver for the Helmholtz equation, optimized for acoustics.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
math-fem = { version = "0.3" }
```

## Features

- **2D and 3D meshes**: Triangles, quadrilaterals, tetrahedra, hexahedra
- **Lagrange elements**: P1, P2, P3 polynomial basis functions
- **Boundary conditions**: Dirichlet, Neumann, Robin (impedance/absorption), PML
- **Optimized Assembly**: `HelmholtzAssembler` for efficient frequency sweeps without CSR topology reconstruction
- **WaveHoltz solver**: O(N) Helmholtz solver via time-filtering of the wave equation, with GMRES acceleration and multigrid inner solves
- **Schwarz-PML solver**: Domain decomposition with PML transmission conditions for k-independent convergence
- **Multigrid solver**: V-cycle, W-cycle with geometric coarsening
- **Parallel processing**: Rayon-based parallel matrix and RHS assembly
- **Room Acoustics Simulator**: CLI tool for frequency-domain room simulation

## Usage

```rust
use math_audio_fem::{mesh, basis::PolynomialDegree};
use math_audio_fem::assembly::HelmholtzAssembler;
use num_complex::Complex64;

// Create a 3D mesh for a room
let mesh = mesh::box_mesh_tetrahedra(0.0, 5.0, 0.0, 4.0, 0.0, 2.5, 10, 8, 5);

// Create an efficient assembler
let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);

// Assemble system for a specific frequency (wavenumber k)
let k = Complex64::new(1.5, 0.01); // k = omega/c + i*damping
let system_matrix = assembler.assemble(k, &std::collections::HashMap::new());
```

## Binaries

### roomsim-fem

A high-performance room acoustics simulator. It supports:
- Rectangular and L-shaped geometries
- Configurable wall absorption and impedance
- Directional sound sources with crossovers
- Hierarchical warm-starting for fast frequency sweeps

**Documentation:** [Room Simulator Guide](docs/room_simulator.md) | [JSON Schema](docs/room_config_schema.json)

```bash
cargo run --release --bin roomsim-fem --features "cli native" -- --config room.json
```

## Modules

### mesh

Mesh generators for common domains:

```rust
use math_audio_fem::mesh::*;

// 3D box mesh with tetrahedra
let box_tet = box_mesh_tetrahedra(0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 5, 5, 5);
```

### basis

Lagrange polynomial basis functions (P1, P2, P3).

### boundary

Boundary condition handling including Robin (Impedance) conditions for acoustics.

### assembly

High-performance matrix assembly:
- **Stiffness & Mass**: Stored as `f64` for memory efficiency.
- **HelmholtzAssembler**: Pre-assembles sparsity patterns for lightning-fast frequency updates.
- **Boundary Mass**: Efficiently integrates terms over boundary surfaces.

### solver

Multiple solver strategies for the Helmholtz system:

| Solver | Best For | Complexity |
|--------|----------|------------|
| Direct (LU) | N < 1000 | O(N³) |
| GMRES+ILU | N < 10000 | O(N) per iter |
| GMRES+AMG | Large parallel | O(N) per iter |
| Shifted-Laplacian | High frequency | O(N) per iter |
| **WaveHoltz** | **High frequency, large N** | **O(N) total** |
| **Schwarz-PML** | **High frequency, k-robust** | **O(N) per iter** |

### waveholtz

WaveHoltz solver that converts the indefinite Helmholtz problem into positive-definite
wave equation time-steps. Features:
- Implicit Newmark time-stepping with CG+AMG inner solves
- GMRES-accelerated outer iteration
- Multi-frequency solving
- Dispersion correction for high-accuracy

### schwarz_pml

Optimized Schwarz domain decomposition with PML transmission conditions. Unlike
classical Schwarz methods, using PML absorption at artificial subdomain boundaries
achieves bounded iteration counts independent of wavenumber k. Features:
- 1D strip decomposition with automatic PML sizing
- Additive (parallel) and multiplicative (sequential) variants
- Partition-of-unity blending in overlap regions
- Auto-tuning via `SchwarzPmlConfig::for_wavenumber(k)`

Called directly via `schwarz_pml::solve_schwarz_pml()` (requires mesh geometry).

### multigrid

Geometric multigrid solver for large systems.

## Element Types

| Element | Dimension | Nodes (P1) | Nodes (P2) |
|---------|-----------|------------|------------|
| Triangle | 2D | 3 | 6 |
| Quadrilateral | 2D | 4 | 9 |
| Tetrahedron | 3D | 4 | 10 |
| Hexahedron | 3D | 8 | 27 |

## Feature Flags

- `native` (default) - Enables rayon parallelism and hardware-specific BLAS
- `parallel` - Enables rayon for parallel assembly
- `cli` - Enables CLI dependencies for `roomsim-fem`

## Dependencies

- `math-solvers` - Iterative solvers (GMRES, AMG, Schwarz)
- `math-xem-common` - Shared room acoustics types and configurations
- `ndarray` - N-dimensional arrays for numerical computing
- `num-complex` - Complex number support for Helmholtz systems

## References

- D. Appelo, D. Garcia, O. Runborg. "WaveHoltz: Iterative Solution of the Helmholtz
  Equation via the Wave Equation." SIAM J. Sci. Comput., 2020.
  https://doi.org/10.1137/19M1299062 | https://arxiv.org/abs/1910.10148

- D. Appelo, J.W. Banks, W.D. Henshaw, D.W. Schwendeman. "An Optimal O(N) Helmholtz
  Solver for Complex Geometry using WaveHoltz and Overset Grids." 2025.
  https://arxiv.org/abs/2504.03074

- D. Appelo, J.W. Banks, W.D. Henshaw, F. Schillinger. "A Multi-Frequency Helmholtz
  Solver Based on the WaveHoltz Algorithm." 2025.
  https://arxiv.org/abs/2507.23613

- T. Hagstrom, D. Appelo. "Convergence of the Semi-Discrete WaveHoltz Iteration." 2024.
  https://arxiv.org/abs/2407.06929

- J. Galkowski, S. Sherwood, D. Lafontaine, E.A. Spence. "Optimised Schwarz methods
  with PML transmission conditions for the Helmholtz equation." 2024.
  https://arxiv.org/abs/2408.16580