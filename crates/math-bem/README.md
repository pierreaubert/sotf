# math-bem: Boundary Element Method Library

A high-performance, memory-efficient Boundary Element Method (BEM) library in Rust for solving acoustic scattering and room acoustics problems.

## Overview

This crate provides a pure Rust BEM implementation with Fast Multipole Method (FMM) acceleration and multiple solver options. The library is designed to be:

- **Reusable**: General-purpose BEM solver for acoustic problems
- **Memory-efficient**: Careful memory management for large-scale problems
- **Parallel**: Uses Rayon for data parallelism
- **Well-tested**: Comprehensive validation against analytical solutions
- **Scientifically rigorous**: All algorithms validated against published research

## Features

### Current (v0.1.2)

#### Core BEM
- Pure Rust BEM implementation with Burton-Miller formulation
- Collocation method with adaptive Gaussian quadrature
- Support for triangular and quadrilateral elements
- Near-singular and singular integration handling

#### Solvers
- **Direct solver**: LU factorization for small problems (N < 1000)
- **GMRES + ILU**: Iterative solver with ILU preconditioning
- **FMM + GMRES**: Fast Multipole Method with O(N log N) complexity
- **FMM + GMRES + ILU**: FMM with ILU preconditioner (recommended for medium problems)
- **FMM + Batched BLAS**: Optimized FMM using batched matrix operations (recommended for large problems)
- **Hierarchical FMM Preconditioner**: Near-field ILU for FMM systems

#### Room Acoustics Application
- Room geometry parsing (rectangular and L-shaped rooms)
- Multi-source acoustic simulations
- Frequency sweep with parallel processing
- SPL computation at listening positions
- Spatial field visualization (pressure slices)

#### Optimizations
- Batched BLAS operations for efficient matvec
- Pre-allocated workspaces to minimize allocations
- Rayon-based parallel assembly and solving
- Frequency-adaptive mesh refinement

### Planned
- GPU acceleration via wgpu
- Multi-level FMM (ML-FMM)
- Coupled BEM-FEM

## Mathematical Background

### Boundary Element Method

The Boundary Element Method solves boundary value problems by reformulating them as boundary integral equations. For acoustic scattering problems, we solve the Helmholtz equation:

```
nabla^2 p + k^2 p = 0
```

where:
- `p` is the acoustic pressure
- `k = 2*pi*f/c` is the wave number
- `f` is frequency, `c` is speed of sound

### Burton-Miller Formulation

We use the **Burton-Miller formulation** to avoid spurious resonances at interior eigenfrequencies:

```
integral_dOmega [G(x,y) dp/dn(y) - dG(x,y)/dn(y) p(y)] dS(y) +
i*alpha integral_dOmega [dG(x,y)/dn(x) dp/dn(y) - d^2G(x,y)/(dn(x)dn(y)) p(y)] dS(y) = p_inc(x)
```

where:
- `G(x,y) = exp(ik|x-y|) / (4*pi*|x-y|)` is the Helmholtz Green's function
- `alpha` is a coupling parameter (typically `1/k`)
- `dOmega` is the boundary surface
- `p_inc` is the incident field

### Fast Multipole Method

For large problems (N > 1000 elements), the **Single-Level Fast Multipole Method (SLFMM)** reduces complexity from O(N^2) to approximately O(N log N) by:

1. **Hierarchical decomposition**: Octree spatial subdivision
2. **Far-field approximation**: Multipole expansions for distant interactions
3. **Translation operators**: Efficient propagation of expansions

The FMM implementation includes:
- Adaptive octree construction
- T-matrices (element DOFs to multipole expansion)
- D-matrices (translation between clusters)
- S-matrices (local expansion to field DOFs)
- Batched BLAS operations for efficient matvec

## Solver Selection Guide

| System Size | Solver Method | Configuration |
|-------------|---------------|---------------|
| N < 1000 | Direct LU | `"direct"` |
| N < 5000 | GMRES + ILU | `"gmres+ilu"` |
| N < 20000 | FMM + GMRES + ILU | `"fmm+gmres+ilu"` |
| N > 20000 | FMM + Batched BLAS | `"fmm+batched"` or `"fmm+gmres+batched"` |

**Recommended for room acoustics**: `"fmm+gmres+ilu"` with hierarchical preconditioner for medium rooms, `"fmm+batched"` for large simulations.

## Usage

### Room Simulator Binary

```bash
# Build
cargo build --release --bin room-simulator-bem

# Run simulation
./target/release/room-simulator-bem --config configs/test_room.json --output output.json

# Verbose output
./target/release/room-simulator-bem --config configs/test_room.json --output output.json --verbose
```

### Configuration File

```json
{
  "room": {
    "type": "rectangular",
    "width": 5.0,
    "depth": 4.0,
    "height": 3.0
  },
  "sources": [
    {
      "name": "Speaker",
      "position": { "x": 1.0, "y": 0.5, "z": 1.2 },
      "amplitude": 1.0,
      "directivity": { "type": "omnidirectional" }
    }
  ],
  "listening_positions": [
    { "x": 2.5, "y": 3.0, "z": 1.2 }
  ],
  "frequencies": {
    "min_freq": 100.0,
    "max_freq": 500.0,
    "num_points": 20,
    "spacing": "logarithmic"
  },
  "solver": {
    "method": "fmm+gmres+ilu",
    "mesh_resolution": 8,
    "gmres": {
      "max_iter": 1000,
      "restart": 50,
      "tolerance": 1e-6
    },
    "ilu": {
      "method": "tbem",
      "scanning_degree": "fine",
      "use_hierarchical": true
    },
    "adaptive_meshing": false
  }
}
```

### Solver Methods

- `"direct"`: Direct LU solver
- `"gmres+ilu"`: GMRES with ILU preconditioner (no FMM)
- `"fmm+gmres"`: FMM with GMRES (no preconditioning)
- `"fmm+gmres+ilu"`: FMM with GMRES and ILU preconditioner
- `"fmm+batched"` or `"fmm+gmres+batched"`: FMM with batched BLAS operations

### Library API

```rust
use bem::room_acoustics::{RoomSimulation, RoomConfig, solve_bem_fmm_gmres_ilu};
use bem::core::solver::{GmresConfig, IluMethod, IluScanningDegree};

// Load configuration
let config = RoomConfig::load("config.json")?;
let simulation = RoomSimulation::from_config(&config)?;

// Solve at a specific frequency
let frequency = 200.0;
let k = simulation.wavenumber(frequency);
let mesh = simulation.room.generate_mesh(config.solver.mesh_resolution);

let solution = solve_bem_fmm_gmres_ilu(
    &mesh,
    &simulation.sources,
    k,
    frequency,
    &FmmSolverConfig::default(),
    1000,  // max iterations
    50,    // restart
    1e-6,  // tolerance
    IluMethod::Tbem,
    IluScanningDegree::Fine,
)?;
```

### Batched BLAS Operations

For large-scale problems, use the batched BLAS solver:

```rust
use bem::core::solver::{
    gmres_solve_fmm_batched_with_ilu, GmresConfig, IluMethod, IluScanningDegree
};
use bem::room_acoustics::build_fmm_system;

// Build FMM system
let (fmm_system, elements, nodes) = build_fmm_system(
    &mesh, &sources, k, frequency, &fmm_config
)?;

// Solve with batched operations
let gmres_config = GmresConfig {
    max_iterations: 1000,
    restart: 100,
    tolerance: 1e-6,
    print_interval: 1,
};

let result = gmres_solve_fmm_batched_with_ilu(
    &fmm_system,
    &fmm_system.rhs,
    IluMethod::Tbem,
    IluScanningDegree::Fine,
    &gmres_config,
);

println!("Converged: {}, Iterations: {}", result.converged, result.iterations);
```

## Architecture

```
math-bem/
├── src/
│   ├── lib.rs                  # Public API
│   ├── room_acoustics/         # Room acoustics application
│   │   ├── mod.rs              # Room geometry, mesh generation
│   │   ├── config.rs           # JSON configuration parsing
│   │   └── solver.rs           # High-level solve functions
│   ├── core/
│   │   ├── assembly/
│   │   │   ├── tbem.rs         # Traditional BEM assembly
│   │   │   └── slfmm.rs        # Single-Level FMM assembly
│   │   ├── solver/
│   │   │   ├── gmres.rs        # GMRES iterative solver
│   │   │   ├── ilu_preconditioner.rs  # ILU preconditioner
│   │   │   ├── preconditioner.rs      # Preconditioner traits
│   │   │   ├── fmm_interface.rs       # FMM solver interfaces
│   │   │   └── batched_blas.rs        # Batched BLAS operations
│   │   ├── quadrature/         # Gaussian quadrature
│   │   └── types.rs            # Core data structures
│   └── analytical/             # Analytical solutions for validation
├── bin/
│   └── room_simulator_bem.rs   # Room simulator binary
├── configs/                    # Example configurations
├── tests/                      # Integration tests
└── plotting/                   # Visualization tools
```

## References

### Primary Research

1. **Burton, A.J., & Miller, G.F. (1971)**
   "The application of integral equation methods to the numerical solution of some exterior boundary-value problems"
   *Proceedings of the Royal Society of London A*, vol. 323, pp. 201-210
   [DOI: 10.1098/rspa.1971.0097](https://doi.org/10.1098/rspa.1971.0097)

2. **Gumerov, N.A., & Duraiswami, R. (2009)**
   "Fast multipole methods for the Helmholtz equation in three dimensions"
   *Elsevier Series in Electromagnetism*
   ISBN: 978-0080531595

3. **Sauter, S.A., & Schwab, C. (2011)**
   "Boundary Element Methods"
   *Springer Series in Computational Mathematics*, vol. 39
   ISBN: 978-3-540-68092-5

4. **Marburg, S., & Nolte, B. (2008)**
   "Computational Acoustics of Noise Propagation in Fluids - Finite and Boundary Element Methods"
   *Springer*
   ISBN: 978-3-540-77447-1

## Build Instructions

### Prerequisites

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# BLAS library (platform-specific)
# macOS: Accelerate framework (included)
# Linux: OpenBLAS
sudo apt install libopenblas-dev
# Windows: Intel MKL or OpenBLAS
```

### Build

```bash
# Build library and binary
cargo build --release

# Run tests
cargo test --release

# Build documentation
cargo doc --open
```

## Performance Notes

### Memory Efficiency

- **Batched workspaces**: Pre-allocated buffers for FMM matvec
- **Sparse near-field**: Only store near-field blocks
- **Parallel assembly**: Rayon-based parallel matrix construction

### Solver Selection

- For N < 1000: Direct solver is fastest
- For 1000 < N < 20000: FMM+GMRES+ILU with hierarchical preconditioner
- For N > 20000: FMM+Batched BLAS for reduced memory pressure

### Parallel Execution

```bash
# Control thread count with Rayon
RAYON_NUM_THREADS=8 ./target/release/room-simulator-bem --config config.json
```

## License

Same license as parent project (SOTF): check root directory.

## Citation

```bibtex
@software{math_bem,
  title = {math-bem: Rust Boundary Element Method Library},
  author = {SOTF Contributors},
  year = {2025},
  url = {https://github.com/pierreaubert/sotf/tree/master/math-bem}
}
```
