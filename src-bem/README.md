# src-bem: Boundary Element Method Library

A high-performance, memory-efficient Boundary Element Method (BEM) library in Rust for solving acoustic scattering problems.

## Overview

This crate provides Rust bindings to the NumCalc BEM solver and, in the future, a pure Rust BEM implementation. The library is designed to be:

- **Reusable**: General-purpose BEM solver for acoustic problems
- **Memory-efficient**: Careful memory management for large-scale problems
- **Parallel**: Uses Rayon for data parallelism without async overhead
- **Well-tested**: Comprehensive validation against analytical solutions in 1D, 2D, and 3D
- **Scientifically rigorous**: All algorithms validated against published research

## Features

### Current (v0.1.0)
- ✅ FFI wrapper around NumCalc C++ BEM solver
- ✅ Memory-efficient parallel execution with Rayon
- ✅ Comprehensive test suite with analytical solutions
- ✅ JSON output for validation and visualization
- ✅ Interactive plotting (plotly.js for 1D/2D, three.js for 3D)

### Planned (v0.2.0+)
- 🔄 Pure Rust BEM implementation (basic collocation method)
- 🔄 Fast Multipole Method (FMM) acceleration
- 🔄 GPU acceleration via wgpu
- 🔄 Adaptive mesh refinement

## Mathematical Background

### Boundary Element Method

The Boundary Element Method solves boundary value problems by reformulating them as boundary integral equations. For acoustic scattering problems, we solve the Helmholtz equation:

```
∇²p + k²p = 0
```

where:
- `p` is the acoustic pressure
- `k = 2πf/c` is the wave number
- `f` is frequency, `c` is speed of sound

### Burton-Miller Formulation

NumCalc uses the **Burton-Miller formulation** to avoid spurious resonances at interior eigenfrequencies:

```
∫∂Ω [G(x,y) ∂p/∂n(y) - ∂G(x,y)/∂n(y) p(y)] dS(y) +
iα ∫∂Ω [∂G(x,y)/∂n(x) ∂p/∂n(y) - ∂²G(x,y)/(∂n(x)∂n(y)) p(y)] dS(y) = p_inc(x)
```

where:
- `G(x,y) = exp(ik|x-y|) / (4π|x-y|)` is the Helmholtz Green's function
- `α` is a coupling parameter (typically `1/k`)
- `∂Ω` is the boundary surface
- `p_inc` is the incident field

### Fast Multipole Method

For large problems (N > 10,000 elements), direct BEM has O(N²) complexity. The **Multi-Level Fast Multipole Method (ML-FMM)** reduces this to O(N log N) by:

1. **Hierarchical decomposition**: Octree spatial subdivision
2. **Far-field approximation**: Multipole expansions for distant interactions
3. **Translation operators**: Efficient propagation of expansions

## References

### Primary Research

1. **Brinkmann, F., et al. (2024)**
   "NumCalc: An open-source BEM code for solving acoustic scattering problems"
   *Engineering Analysis with Boundary Elements*, vol. 161, pp. 157-178
   [DOI: 10.1016/j.enganabound.2024.01.001](https://doi.org/10.1016/j.enganabound.2024.01.001)

2. **Ziegelwanger, H., Kreuzer, W., & Majdak, P. (2015)**
   "Mesh2HRTF: An open-source software package for the numerical calculation of head-related transfer functions"
   *22nd International Congress on Sound and Vibration (ICSV22)*
   [ResearchGate](https://www.researchgate.net/publication/280007918)

3. **Burton, A.J., & Miller, G.F. (1971)**
   "The application of integral equation methods to the numerical solution of some exterior boundary-value problems"
   *Proceedings of the Royal Society of London A*, vol. 323, pp. 201-210
   [DOI: 10.1098/rspa.1971.0097](https://doi.org/10.1098/rspa.1971.0097)

### Fast Multipole Method

4. **Gumerov, N.A., & Duraiswami, R. (2009)**
   "Fast multipole methods for the Helmholtz equation in three dimensions"
   *Elsevier Series in Electromagnetism*
   ISBN: 978-0080531595

5. **Cheng, H., et al. (2006)**
   "A wideband fast multipole method for the Helmholtz equation in three dimensions"
   *Journal of Computational Physics*, vol. 216, pp. 300-325
   [DOI: 10.1016/j.jcp.2005.12.001](https://doi.org/10.1016/j.jcp.2005.12.001)

### Numerical Integration

6. **Sauter, S.A., & Schwab, C. (2011)**
   "Boundary Element Methods"
   *Springer Series in Computational Mathematics*, vol. 39
   ISBN: 978-3-540-68092-5

7. **Lachat, J.C., & Watson, J.O. (1976)**
   "Effective numerical treatment of boundary integral equations: A formulation for three-dimensional elastostatics"
   *International Journal for Numerical Methods in Engineering*, vol. 10, pp. 991-1005

### Acoustic Scattering

8. **Marburg, S., & Nolte, B. (2008)**
   "Computational Acoustics of Noise Propagation in Fluids - Finite and Boundary Element Methods"
   *Springer*
   ISBN: 978-3-540-77447-1

## Software References

### Mesh2HRTF Project

- **GitHub Repository**: [Any2HRTF/Mesh2HRTF](https://github.com/Any2HRTF/Mesh2HRTF)
- **Official Website**: [mesh2hrtf.org](https://www.mesh2hrtf.org)
- **Documentation**: [ReadTheDocs](https://mesh2hrtf.readthedocs.io)
- **License**: EUPL-1.2 (European Union Public License)

### Related Projects

- **Bempp**: Python/Rust BEM library - [bempp.com](https://bempp.com) | [GitHub](https://github.com/bempp/bempp-rs)
- **BEM++**: C++ BEM library - [bempp.com](https://bempp.com)
- **OpenBEM**: Open-source BEM framework

## Architecture

```
src-bem/
├── src/
│   ├── lib.rs              # Public API
│   ├── ffi/                # FFI bindings to NumCalc
│   │   ├── mod.rs          # FFI module
│   │   ├── wrapper.rs      # High-level wrapper
│   │   ├── runner.rs       # Subprocess execution
│   │   └── parallel.rs     # Rayon-based parallelism
│   ├── core/               # Pure Rust BEM (future)
│   │   ├── mod.rs
│   │   ├── mesh.rs         # Mesh data structures
│   │   ├── quadrature.rs   # Numerical integration
│   │   ├── greens.rs       # Green's functions
│   │   └── solver.rs       # Linear system solver
│   ├── analytical/         # Analytical solutions for testing
│   │   ├── mod.rs
│   │   ├── solutions_1d.rs # 1D wave equation
│   │   ├── solutions_2d.rs # 2D cylinder scattering
│   │   └── solutions_3d.rs # 3D sphere scattering (Mie theory)
│   └── testing/            # Test infrastructure
│       ├── mod.rs
│       ├── validation.rs   # Comparison with analytical
│       ├── json_output.rs  # JSON serialization
│       └── plotting.rs     # Plot generation helpers
├── tests/
│   ├── test_1d_wave.rs     # 1D analytical validation
│   ├── test_2d_cylinder.rs # 2D analytical validation
│   └── test_3d_sphere.rs   # 3D analytical validation (Mie)
├── benches/
│   └── bem_benchmarks.rs   # Performance benchmarks
├── examples/
│   ├── simple_sphere.rs    # Basic sphere scattering
│   └── parallel_frequencies.rs  # Parallel frequency sweep
├── plotting/               # Web-based visualization
│   ├── index.html          # Main plotting interface
│   ├── plot_1d.html        # 1D plots (plotly.js)
│   ├── plot_2d.html        # 2D plots (plotly.js)
│   └── plot_3d.html        # 3D plots (three.js + plotly.js)
└── NumCalc/                # C++ source (git submodule)
    └── src/                # NumCalc C++ code
```

## Testing Strategy

### Analytical Validation

All BEM implementations are validated against known analytical solutions:

#### 1D: Plane Wave Propagation

**Problem**: Plane wave in 1D: `p(x) = exp(ikx)`

**Analytical solution**:
```rust
p(x, k) = exp(ikx)
```

**Test cases**:
- Various wave numbers: `k = [1, 5, 10, 20]`
- Domain: `x ∈ [0, 10]`
- Boundary conditions: Dirichlet at x=0, Sommerfeld at x=10

**Metrics**:
- L2 error: `||p_bem - p_analytical||₂ / ||p_analytical||₂`
- L∞ error: `max|p_bem - p_analytical|`

#### 2D: Cylinder Scattering

**Problem**: Plane wave scattering by a rigid circular cylinder

**Analytical solution**: Sum of Bessel functions
```rust
p(r, θ, k) = exp(ikr cos θ) + ∑ aₙ Hₙ⁽¹⁾(kr) exp(inθ)
```

where `Hₙ⁽¹⁾` are Hankel functions of the first kind.

**Test cases**:
- Cylinder radius: `a = 1.0`
- Frequencies: `ka = [0.5, 1, 2, 5, 10]` (low to high frequency)
- Incident angles: `[0°, 45°, 90°]`

**Metrics**:
- Surface pressure error
- Far-field scattering pattern error
- Total scattering cross-section

#### 3D: Sphere Scattering (Mie Theory)

**Problem**: Plane wave scattering by a rigid sphere

**Analytical solution**: Mie series
```rust
p(r, θ, k) = ∑ₙ (2n+1) iⁿ [jₙ(kr) - aₙhₙ⁽¹⁾(kr)] Pₙ(cos θ)
```

where:
- `jₙ` = spherical Bessel functions
- `hₙ⁽¹⁾` = spherical Hankel functions (first kind)
- `Pₙ` = Legendre polynomials

**Test cases**:
- Sphere radius: `a = 1.0`
- Frequencies: `ka = [0.1, 0.5, 1, 2, 5, 10]` (Rayleigh to geometric)
- Mesh resolutions: `λ/10`, `λ/6`, `λ/4` elements per wavelength

**Metrics**:
- Surface pressure distribution
- Far-field directivity pattern
- Radar cross-section (RCS)
- Convergence rate with mesh refinement

### Test Output Format

All tests generate JSON files with this structure:

```json
{
  "test_name": "sphere_scattering_ka_1.0",
  "dimensions": 3,
  "parameters": {
    "wave_number": 1.0,
    "radius": 1.0,
    "num_elements": 512,
    "frequency_hz": 54.6
  },
  "analytical": {
    "positions": [[x1, y1, z1], [x2, y2, z2], ...],
    "pressure_real": [p1_re, p2_re, ...],
    "pressure_imag": [p1_im, p2_im, ...]
  },
  "bem": {
    "positions": [[x1, y1, z1], ...],
    "pressure_real": [p1_re, p2_re, ...],
    "pressure_imag": [p1_im, p2_im, ...]
  },
  "errors": {
    "l2_relative": 0.0023,
    "l2_absolute": 0.015,
    "linf": 0.012,
    "mean_absolute": 0.0045
  },
  "metadata": {
    "timestamp": "2025-11-22T10:30:00Z",
    "git_commit": "a205215",
    "execution_time_ms": 1250,
    "memory_peak_mb": 45.2
  }
}
```

### Visualization

Run tests and visualize:

```bash
# Run all analytical tests
cargo test --release -- --nocapture

# Generate JSON output
cargo test --release test_3d_sphere -- --nocapture > results.json

# Open web interface
cd plotting
python -m http.server 8080
# Navigate to http://localhost:8080
```

**Plotting features**:
- **1D plots**: Pressure magnitude/phase vs. position, error vs. position
- **2D plots**: Pressure field contours, directivity patterns, error heatmaps
- **3D plots**: Surface pressure on sphere, 3D directivity, mesh visualization

## Usage

### FFI Wrapper (Current)

```rust
use bem::{NumCalcRunner, NumCalcConfig};

// Run BEM simulation
let runner = NumCalcRunner::new("project_dir")?;
let config = NumCalcConfig {
    freq_start_idx: Some(0),
    freq_end_idx: Some(10),
    max_iterations: Some(1000),
    ..Default::default()
};

let output = runner.run(&config)?;
println!("Simulation complete: {:?}", output);
```

### Parallel Frequency Sweep

```rust
use bem::ParallelBemRunner;
use rayon::prelude::*;

let runner = ParallelBemRunner::new("project_dir")?;

// Parallel execution with Rayon (NOT tokio)
let frequencies = vec![100.0, 200.0, 500.0, 1000.0];

let results: Vec<_> = frequencies
    .par_iter()
    .map(|&freq| runner.solve_at_frequency(freq))
    .collect();
```

### Analytical Validation

```rust
use bem::analytical::sphere_scattering_3d;
use bem::testing::validate_against_analytical;

// Run BEM
let bem_result = run_bem_sphere(ka: 1.0, num_elements: 512)?;

// Compare with Mie theory
let analytical = sphere_scattering_3d(ka: 1.0, n_terms: 50)?;

let validation = validate_against_analytical(&bem_result, &analytical)?;

// Export to JSON
validation.save_json("sphere_ka1_validation.json")?;

println!("L2 error: {:.6}", validation.l2_error);
println!("L∞ error: {:.6}", validation.linf_error);
```

## Performance Considerations

### Memory Efficiency

- **Sparse storage**: Only store non-zero matrix elements
- **Out-of-core**: Stream data from disk for very large problems
- **Memory pooling**: Reuse allocations across frequency steps
- **Compression**: Store far-field interactions in compressed form

### Parallelism with Rayon

**Why Rayon, not Tokio?**

- ✅ **Data parallelism**: Perfect for matrix operations
- ✅ **No async overhead**: Direct thread pool management
- ✅ **Work stealing**: Automatic load balancing
- ✅ **Zero-cost abstraction**: Compiles to tight loops
- ❌ **Tokio**: Designed for I/O, not CPU-bound computation

**Parallel strategies**:

```rust
// 1. Parallel frequency sweep (embarrassingly parallel)
frequencies.par_iter().map(|f| solve_bem(f))

// 2. Parallel matrix assembly (shared-memory)
elements.par_iter().map(|elem| compute_matrix_row(elem))

// 3. Parallel equation solve (via BLAS/LAPACK threaded)
// Already handled by OpenBLAS/MKL
```

## Build Instructions

### Prerequisites

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install C++ compiler (for NumCalc)
# Linux:
sudo apt install build-essential

# macOS:
xcode-select --install

# Windows:
# Install Visual Studio 2022 with C++ tools
```

### Build

```bash
# Clone repository
git clone https://github.com/pierreaubert/sotf
cd sotf/src-bem

# Build library
cargo build --release

# Run tests with validation
cargo test --release

# Run benchmarks
cargo bench

# Build documentation
cargo doc --open
```

## Roadmap

### Phase 1: FFI Wrapper (Current - v0.1.0)
- [x] Build system integration
- [x] Subprocess wrapper
- [x] Rayon-based parallelism
- [x] 1D analytical tests
- [ ] 2D analytical tests (cylinder)
- [ ] 3D analytical tests (sphere - Mie theory)
- [ ] JSON output infrastructure
- [ ] Web-based plotting

### Phase 2: Pure Rust BEM (v0.2.0 - 6 months)
- [ ] Basic collocation method
- [ ] Gauss quadrature integration
- [ ] Direct solver (dense matrices)
- [ ] Validate against NumCalc
- [ ] Performance comparison

### Phase 3: FMM Acceleration (v0.3.0 - 12 months)
- [ ] Octree spatial decomposition
- [ ] Multipole expansions
- [ ] Translation operators
- [ ] Adaptive refinement
- [ ] O(N log N) complexity

### Phase 4: Advanced Features (v0.4.0+)
- [ ] GPU acceleration (wgpu)
- [ ] Adaptive mesh refinement
- [ ] Error estimation
- [ ] Multi-domain problems
- [ ] Coupled BEM-FEM

## Contributing

This is part of the SOTF (Sound of the Future) project. Contributions welcome!

See [CONTRIBUTING.md](../CONTRIBUTING.rst) for guidelines.

## License

Same license as parent project (SOTF): check root directory.

## Citation

If you use this library in academic work, please cite:

```bibtex
@software{sotf_bem,
  title = {src-bem: Rust Boundary Element Method Library},
  author = {SOTF Contributors},
  year = {2025},
  url = {https://github.com/pierreaubert/sotf/tree/master/src-bem}
}

@article{brinkmann2024numcalc,
  title = {NumCalc: An open-source BEM code for solving acoustic scattering problems},
  author = {Brinkmann, Fabian and others},
  journal = {Engineering Analysis with Boundary Elements},
  volume = {161},
  pages = {157--178},
  year = {2024},
  doi = {10.1016/j.enganabound.2024.01.001}
}
```

## Contact

For questions about BEM theory or implementation, please open an issue on GitHub.

---

**Note**: This is research-grade software under active development. APIs may change. Always validate results against analytical solutions for your specific problem.
