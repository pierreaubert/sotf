# Mesh2HRTF Rust Rewrite Feasibility Analysis

## Executive Summary

Rewriting Mesh2HRTF in Rust is **technically feasible** but presents **significant challenges**, particularly in the NumCalc (BEM solver) component. The project has excellent test coverage for validation, and the Rust ecosystem provides most required dependencies. However, the Fast Multipole Method implementation will require substantial effort.

**Difficulty Assessment:**
- **Mesh2Input**: 🟢 Easy-Medium (mesh processing)
- **NumCalc (BEM)**: 🔴 Hard (complex numerical methods)
- **Output2HRTF**: 🟡 Medium (file I/O and DSP)

## Project Overview

### What is Mesh2HRTF?

Mesh2HRTF calculates Head-Related Transfer Functions (HRTFs) from 3D head geometry using numerical acoustics. It's used in binaural audio research and spatial audio applications.

**Repository**: https://github.com/Any2HRTF/Mesh2HRTF
- **Stars**: 143
- **License**: EUPL-1.2
- **Languages**: C++ (45%), Python (26%), C (15%), MATLAB (13%)
- **Size**: ~34 MB
- **Active**: Last update Nov 2025

### Three-Stage Pipeline

```
┌─────────────────┐      ┌─────────────┐      ┌──────────────────┐
│   Mesh2Input    │ ───> │   NumCalc   │ ───> │  Output2HRTF     │
│  (Preparation)  │      │ (BEM Solver)│      │ (Post-process)   │
└─────────────────┘      └─────────────┘      └──────────────────┘
     Python                  C++                Python/MATLAB
     ~52 KB                 ~554 KB              ~150 KB
```

## Component Analysis

### 1. Mesh2Input (Python, ~52 KB)

**Purpose**: Prepare 3D head mesh and acoustic parameters

**Functionality**:
- Load 3D mesh files (OBJ, STL, VTK formats)
- Assign material properties (impedance, absorption)
- Define evaluation grids (microphone positions)
- Export to NumCalc input format

**Complexity**: 🟢 **EASY-MEDIUM**

**Rust Implementation Path**:
- ✅ **Available crates**:
  - `meshx`: VTK-compatible mesh I/O with TriMesh, PolyMesh, TetMesh
  - `plexus`: Polygonal mesh processing (serialization, deserialization)
  - `baby_shark`: STL/OBJ reading with auto-detection
  - `lox`: Polygon mesh library with processing traits
- ✅ **Skills needed**: File I/O, data structure mapping
- ⚠️ **Challenge**: No direct Python-to-Rust port, requires understanding mesh formats

**Estimated Effort**: 2-4 weeks

---

### 2. NumCalc (C++, ~554 KB) - **THE HARD PART**

**Purpose**: Boundary Element Method (BEM) solver for acoustic scattering

#### Source Files (13 files):

| File | Size | Complexity |
|------|------|------------|
| `NC_IntegrationConstants.h` | 144 KB | 🔴 High (numerical quadrature) |
| `NC_Addresses.cpp` | 74 KB | 🟡 Medium (indexing, memory layout) |
| `NC_EquationSystem.cpp` | 71 KB | 🔴 High (BEM matrix assembly) |
| `NC_Input.cpp` | 59 KB | 🟢 Low (parsing) |
| `NC_PostProcessing.cpp` | 48 KB | 🟡 Medium (HRTF extraction) |
| `NC_CommonFunctions.cpp` | 44 KB | 🟡 Medium (utilities) |
| `NC_Main.cpp` | 41 KB | 🟢 Low (orchestration) |
| `NC_3dFunctions.cpp` | 27 KB | 🟡 Medium (geometry) |
| `NC_TypeDefinition.h` | 26 KB | 🟢 Low (structs) |
| `NC_Arrays.h` | 8 KB | 🟢 Low (containers) |
| `NC_ConstantsVariables.h` | 9 KB | 🟢 Low (constants) |

#### Mathematical Methods

**Core Algorithm**: **Burton-Miller Collocation BEM** + **Multi-Level Fast Multipole Method (ML-FMM)**

- **Burton-Miller formulation**: Combines Helmholtz and normal derivative to avoid spurious resonances
- **Fast Multipole Method**: Reduces complexity from O(n²) to O(n log² n)
- **Collocation approach**: Point-wise boundary conditions vs. Galerkin integration

**Key Technical Challenges**:

1. **Integration Constants** (144 KB file):
   - Pre-computed quadrature rules for boundary integrals
   - Singular/near-singular integration kernels
   - Frequency-dependent Green's functions

2. **Fast Multipole Method**:
   - Octree spatial decomposition
   - Translation operators (multipole-to-multipole, local-to-local)
   - Near-field/far-field splitting
   - Complex-valued arithmetic (Helmholtz kernel)

3. **Matrix Assembly**:
   - Dense complex matrices (no existing FMM structure in Rust)
   - Parallel frequency stepping
   - Memory-efficient storage (74 KB address management suggests complex indexing)

**Complexity**: 🔴 **HARD**

**Rust Implementation Path**:

✅ **Available infrastructure**:
- `bempp-rs` (v0.2.0): Basic BEM for Laplace/Helmholtz problems
  - ⚠️ **Early stage**, may not have FMM or Burton-Miller
  - ✅ Good starting point for kernel assembly
  - ❌ Likely needs extension for acoustic scattering
- Linear algebra:
  - `ndarray` + `ndarray-linalg`: Dense matrix operations with BLAS/LAPACK
  - `nalgebra`: Pure Rust linear algebra
  - `peroxide`: Numerical analysis with BLAS/LAPACK bindings
- Complex numbers:
  - `num-complex`: Standard complex arithmetic
  - BLAS/LAPACK support for complex matrices via `cblas`/`lapacke`
- FFT (for FMM translation operators):
  - `rustfft`: Pure Rust FFT
  - Existing SOTF project already uses `rustfft` in upmixer

❌ **Not available**:
- Fast Multipole Method library for Helmholtz equation
- Pre-built Burton-Miller formulation
- Acoustic-specific BEM framework

**Required Skills**:
- Deep understanding of BEM theory
- FMM algorithm implementation (3-6 months for expert)
- Complex numerical integration (Gauss quadrature, adaptive schemes)
- Parallel computing (frequency stepping, matrix assembly)
- Debugging numerical accuracy issues

**Estimated Effort**:
- **With BEM expertise**: 6-12 months (implementing FMM from scratch)
- **Without BEM expertise**: 12-24 months (learning + implementation)
- **Using bempp-rs as base**: 4-8 months (if it has required features)

**Alternative Approach**:
- Use `bempp-rs` for basic BEM, skip FMM initially (O(n²) complexity)
- Implement FMM later as optimization
- Focus on correctness first, performance second

---

### 3. Output2HRTF (Python/MATLAB, ~150 KB)

**Purpose**: Convert NumCalc output to standardized HRTF format

**Functionality**:
- Read NumCalc output (complex pressure fields)
- Compute Directional Transfer Functions (DTFs)
- Calculate Head-Related Impulse Responses (HRIRs) via inverse FFT
- Write SOFA files (Spatially Oriented Format for Acoustics)
- Export to VTK for visualization
- Generate analysis reports

**Files**:
- `output2hrtf.py` (12.5 KB): Main conversion
- `compute_dtfs.py` (5 KB): Frequency domain processing
- `compute_hrirs.py` (4.1 KB): Time domain via IFFT
- `export_vtk.py` (7.6 KB): Visualization export
- `merge_sofa_files.py` (5 KB): Multi-file handling
- `inspect_sofa_files.py` (10 KB): File analysis
- `write_output_report.py` (12.3 KB): Report generation

**Complexity**: 🟡 **MEDIUM**

**Rust Implementation Path**:

✅ **Available**:
- `rustfft`: Inverse FFT for HRIR computation (already in SOTF)
- `hdf5`: HDF5 file I/O (SOFA files use HDF5 container)
- `ndarray`: Array operations for signal processing
- `serde`: Serialization for metadata

❌ **Missing**:
- SOFA format library (need to implement HDF5 schema)
- VTK export (can use `meshx` or write custom)

**Skills Needed**:
- HDF5 file format
- Digital signal processing (FFT/IFFT, filtering, windowing)
- Audio file format specifications

**Estimated Effort**: 3-6 weeks

---

## Test Coverage

**Excellent foundation for validation**:

```
tests/
├── test_assign_materials.py
├── test_export.py (12.8 KB)
├── test_manage_numcalc.py
├── test_numcalc.py (12.6 KB)
├── test_output.py (20.1 KB)
├── test_outputs.py (6.8 KB)
├── test_tutorials.py
├── test_utils.py
├── references/ (reference datasets)
└── resources/ (test fixtures)
```

**Why this matters**:
- ✅ Can validate Rust implementation against Python/C++ reference
- ✅ Test-driven development approach
- ✅ Numerical accuracy verification
- ✅ Reference data for regression testing

---

## Rust Ecosystem Readiness

### Strong Foundation

| Domain | Crates | Maturity |
|--------|--------|----------|
| Linear Algebra | ndarray, nalgebra, peroxide | 🟢 Production |
| BLAS/LAPACK | ndarray-linalg, blas-lapack-rs | 🟢 Production |
| Complex Numbers | num-complex | 🟢 Production |
| FFT | rustfft | 🟢 Production (used in SOTF) |
| Mesh Processing | meshx, plexus, baby_shark | 🟡 Growing |
| HDF5 | hdf5 | 🟢 Stable |
| Parallel Computing | rayon, tokio | 🟢 Production |

### Gaps to Fill

| Component | Status | Effort |
|-----------|--------|--------|
| BEM Framework | bempp-rs exists but early | 🟡 Medium |
| Fast Multipole Method | None found | 🔴 High |
| SOFA Format | No library | 🟢 Low (HDF5 wrapper) |
| Burton-Miller BEM | None found | 🔴 High |

---

## Integration with SOTF Project

### Synergies

✅ **Existing SOTF infrastructure**:
- Audio processing engine (can play generated HRTFs)
- Plugin system (could add HRTF convolution plugin)
- FFT support via `rustfft` (for Output2HRTF)
- BLAS/LAPACK integration (for NumCalc matrices)
- Cross-platform builds (Linux, Windows, macOS)

✅ **Potential workflow**:
```
Mesh2Input (Rust) → NumCalc (Rust) → Output2HRTF (Rust) → SOTF Player
     ↓                    ↓                   ↓              ↓
   3D Mesh          BEM Solver          SOFA File      Binaural Audio
```

### Architecture Fit

- **Workspace structure**: Add `src-mesh2hrtf/` crate
- **Binary targets**:
  - `mesh2input`
  - `numcalc`
  - `output2hrtf`
  - `mesh2hrtf` (orchestrator)
- **Shared dependencies**: Reuse SOTF's BLAS/FFT/audio stack

---

## Implementation Strategy

### Phase 1: Validation & Learning (4-6 weeks)

1. **Deep dive into C++ codebase**:
   - Understand NumCalc algorithm flow
   - Map data structures and algorithms
   - Document integration constant generation
   - Study FMM implementation details

2. **Benchmark reference implementation**:
   - Run existing tests
   - Profile performance characteristics
   - Identify computational bottlenecks
   - Collect reference outputs

3. **Prototype assessment**:
   - Test `bempp-rs` capabilities
   - Validate Rust numerical accuracy
   - Benchmark Rust vs C++ performance

### Phase 2: Easy Components (6-10 weeks)

1. **Output2HRTF** (3-6 weeks):
   - Implement SOFA reader/writer (HDF5)
   - Port DTF/HRIR computation
   - VTK export functionality
   - **Why first**: Quick win, validates DSP pipeline

2. **Mesh2Input** (2-4 weeks):
   - Mesh loading (OBJ, STL, VTK)
   - Material assignment
   - Evaluation grid generation
   - Export to NumCalc format

### Phase 3: NumCalc - Simplified (12-16 weeks)

**Option A: Basic BEM (no FMM)**:
- Use `bempp-rs` as foundation
- Implement Burton-Miller formulation
- Dense matrix solver (O(n²))
- ✅ Correctness over performance
- ⚠️ Limited to small meshes (~1000-5000 elements)

**Option B: Collaborate with bempp-rs**:
- Contribute FMM to `bempp-rs` project
- Leverage community expertise
- Share maintenance burden

### Phase 4: NumCalc - Fast Multipole Method (24-36 weeks)

**Only if Phase 3 successful and performance needed**:
- Octree implementation
- Translation operators
- Adaptive refinement
- Parallel assembly
- ⚠️ Major undertaking, requires BEM expertise

---

## Risk Assessment

### High Risks

🔴 **FMM Implementation Complexity**:
- **Risk**: Underestimating FMM difficulty
- **Mitigation**: Start without FMM, validate on small problems
- **Fallback**: Use C++ NumCalc via FFI, rewrite only Mesh2Input/Output2HRTF

🔴 **Numerical Accuracy**:
- **Risk**: Rust port produces different results
- **Mitigation**: Extensive testing against reference data, bit-for-bit validation
- **Fallback**: Increase precision (f64 → arbitrary precision)

🔴 **Developer BEM Expertise**:
- **Risk**: Team lacks BEM domain knowledge
- **Mitigation**: Collaborate with acoustics researchers, hire consultant
- **Fallback**: Hybrid approach (keep C++ NumCalc, Rust wrapper)

### Medium Risks

🟡 **bempp-rs Maturity**:
- **Risk**: Library lacks required features
- **Mitigation**: Evaluate early, contribute upstream
- **Fallback**: Fork or implement from scratch

🟡 **Performance Parity**:
- **Risk**: Rust version slower than C++
- **Mitigation**: Profile-guided optimization, SIMD, parallel execution
- **Fallback**: Use C++ for hot paths

🟡 **SOFA Format Complexity**:
- **Risk**: SOFA spec has edge cases
- **Mitigation**: Implement incrementally, validate with existing files
- **Fallback**: Use Python library via PyO3

### Low Risks

🟢 **Mesh Processing**: Well-supported by existing crates

🟢 **Linear Algebra**: Mature ecosystem (ndarray, BLAS)

🟢 **FFT**: `rustfft` proven in SOTF

---

## Feasibility Verdict

### ✅ **YES, BUT...**

Rewriting Mesh2HRTF in Rust is **feasible** with the following caveats:

1. **Incremental approach required**: Don't try to rewrite everything at once
2. **NumCalc is the bottleneck**: 70% of effort will be BEM solver
3. **FMM is optional**: Start with O(n²) solver for small meshes
4. **Excellent test coverage**: Use reference data for validation
5. **Community collaboration**: Contribute to `bempp-rs` rather than solo effort

### Recommended Path

**SHORT TERM (3-4 months)**:
1. ✅ Implement Output2HRTF (SOFA files, DSP)
2. ✅ Implement Mesh2Input (mesh processing)
3. ✅ FFI wrapper to C++ NumCalc
4. ✅ End-to-end pipeline working

**MEDIUM TERM (6-12 months)**:
5. ⚠️ Basic BEM solver (no FMM) for small meshes
6. ⚠️ Validate against reference data
7. ⚠️ Optimize with Rust techniques

**LONG TERM (12-24 months)**:
8. 🔴 Fast Multipole Method (if performance needed)
9. 🔴 Production-ready for large meshes

### Alternative: Hybrid Approach

**Pragmatic solution**:
- Rust for Mesh2Input and Output2HRTF (70% of codebase)
- Keep C++ NumCalc with Rust FFI bindings
- ✅ Fast time-to-market
- ✅ Leverage existing BEM implementation
- ⚠️ Still requires C++ compiler/dependencies

---

## Estimated Timeline

### Conservative (with BEM expertise)

| Phase | Component | Duration | Difficulty |
|-------|-----------|----------|------------|
| 1 | Research & Planning | 1 month | 🟡 |
| 2 | Output2HRTF | 1.5 months | 🟡 |
| 3 | Mesh2Input | 1 month | 🟢 |
| 4 | NumCalc (basic BEM) | 4 months | 🔴 |
| 5 | Integration & Testing | 2 months | 🟡 |
| **Total** | **Phase 1-5** | **9.5 months** | |
| 6 | NumCalc FMM (optional) | 12 months | 🔴🔴 |

### Aggressive (learning BEM from scratch)

| Phase | Duration | Risk |
|-------|----------|------|
| Learning BEM theory | 3 months | 🟡 |
| Phases 1-5 above | 12 months | 🔴 |
| FMM implementation | 18 months | 🔴🔴 |
| **Total** | **33 months** | **Very High** |

---

## Technical Deep Dive: NumCalc

### What Makes BEM + FMM Hard?

#### 1. Mathematical Complexity

**Burton-Miller Formulation**:
```
∫∂Ω [G(x,y) ∂p/∂n(y) - ∂G(x,y)/∂n(y) p(y)] dS(y) +
iα ∫∂Ω [∂G(x,y)/∂n(x) ∂p/∂n(y) - ∂²G(x,y)/(∂n(x)∂n(y)) p(y)] dS(y) = ...
```

Where:
- `G(x,y) = exp(ik|x-y|) / (4π|x-y|)` (Helmholtz Green's function)
- `k = 2πf/c` (wave number, frequency-dependent)
- `α` (coupling parameter, typically 1/k)
- `∂Ω` (boundary surface)

**Challenges**:
- Complex-valued integrals
- Singular kernels when x → y
- Frequency-dependent operators
- Derivative computation (numerical differentiation)

#### 2. Integration Quadrature

The 144 KB `NC_IntegrationConstants.h` likely contains:
- Gauss-Legendre quadrature points/weights
- Adaptive refinement rules for near-singular integrals
- Pre-computed shape function derivatives
- Element-specific integration schemes (triangle, quad)

**Why hard**:
- Singular/near-singular integrals require specialized methods
- Accuracy vs. performance tradeoff
- Different rules for self-interaction vs. far-field

#### 3. Fast Multipole Method

**Basic idea**: Group far-away interactions hierarchically

```
Octree decomposition:
       ┌─────────────┐
       │   Level 0   │  (root, entire domain)
       └─────────────┘
          /   |   \
     ┌───┐ ┌───┐ ┌───┐
     │L1 │ │L1 │ │L1 │  (8 children)
     └───┘ └───┘ └───┘
       / \
    ┌───┐┌───┐
    │L2 ││L2 │  (64 grandchildren)
    └───┘└───┘
```

**Operations**:
1. **Upward pass**: Compute multipole expansions (leaf → root)
2. **Downward pass**: Translate and accumulate local expansions (root → leaf)
3. **Near-field**: Direct integration for nearby elements

**Why hard**:
- Translation operators involve spherical harmonics or plane waves
- Different formulations (kernel-independent vs. kernel-dependent)
- Adaptive tree construction
- Parallel implementation requires careful load balancing

#### 4. No Existing Rust FMM Library

**What you'd need to implement**:
- Octree spatial data structure
- Multipole expansion computation
- Local expansion computation
- M2M (multipole-to-multipole) translation
- M2L (multipole-to-local) translation
- L2L (local-to-local) translation
- P2M (particle-to-multipole)
- L2P (local-to-particle)
- P2P (particle-to-particle) near-field

**Code volume**: Expect 5,000-10,000 lines for basic FMM

---

## Code Examples

### What Rust Implementation Might Look Like

#### Example 1: SOFA File Writer (Output2HRTF)

```rust
use hdf5::{File, Group};
use ndarray::Array3;

pub struct SofaHrtf {
    pub sample_rate: f64,
    pub data: Array3<f64>, // [positions x ears x samples]
    pub source_positions: Vec<(f64, f64, f64)>, // (azimuth, elevation, distance)
}

impl SofaHrtf {
    pub fn write(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;

        // Write metadata
        let data_group = file.create_group("Data.IR")?;
        data_group.new_dataset::<f64>()
            .shape(self.data.shape())
            .create("values")?
            .write(&self.data)?;

        // Write source positions
        let pos_group = file.create_group("SourcePosition")?;
        // ... (azimuth, elevation, distance arrays)

        Ok(())
    }
}
```

#### Example 2: Mesh Loading (Mesh2Input)

```rust
use meshx::TriMesh;
use std::path::Path;

pub struct HeadMesh {
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[usize; 3]>,
    pub materials: Vec<MaterialId>,
}

impl HeadMesh {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mesh = TriMesh::read(path)?;

        Ok(HeadMesh {
            vertices: mesh.vertices().iter()
                .map(|v| [v[0], v[1], v[2]])
                .collect(),
            triangles: mesh.triangles().iter()
                .map(|t| [t[0], t[1], t[2]])
                .collect(),
            materials: vec![MaterialId::Skin; mesh.triangles().len()],
        })
    }
}
```

#### Example 3: BEM Matrix Assembly (NumCalc - simplified)

```rust
use ndarray::Array2;
use num_complex::Complex64;

pub struct BemSolver {
    mesh: HeadMesh,
    frequency: f64,
    wave_number: Complex64,
}

impl BemSolver {
    pub fn assemble_matrix(&self) -> Array2<Complex64> {
        let n = self.mesh.triangles.len();
        let mut matrix = Array2::<Complex64>::zeros((n, n));

        // This is the O(n²) part that FMM optimizes
        for i in 0..n {
            for j in 0..n {
                matrix[[i, j]] = self.compute_interaction(i, j);
            }
        }

        matrix
    }

    fn compute_interaction(&self, elem_i: usize, elem_j: usize) -> Complex64 {
        // Green's function: exp(ik|x-y|) / (4π|x-y|)
        let [x, y, z] = self.element_center(elem_i);
        let [x2, y2, z2] = self.element_center(elem_j);

        let r = ((x - x2).powi(2) + (y - y2).powi(2) + (z - z2).powi(2)).sqrt();

        if r < 1e-10 {
            // Singular integral - needs special treatment
            self.compute_singular_integral(elem_i)
        } else {
            // Regular Green's function
            let kr = self.wave_number * r;
            kr.exp() / (4.0 * std::f64::consts::PI * r)
        }
    }

    fn compute_singular_integral(&self, elem: usize) -> Complex64 {
        // This is where those 144 KB of integration constants come in...
        // Adaptive Gauss quadrature, near-singular treatment, etc.
        todo!("Implement singular/near-singular integration")
    }
}
```

**Note**: The real implementation is FAR more complex:
- Element shape functions
- Normal derivative computation
- Burton-Miller coupling term
- Adaptive integration
- Near-field corrections

---

## Recommendations

### For Your SOTF Project Context

Given that you already have:
- ✅ Rust audio engine with plugins
- ✅ BLAS/LAPACK integration
- ✅ FFT support (rustfft)
- ✅ Cross-platform build system

**I recommend**:

1. **Start with Output2HRTF** (3-4 weeks):
   - Implement SOFA file reader
   - Create HRTF convolution plugin for SOTF
   - Test with existing SOFA files (don't generate yet)
   - **Value**: Immediate binaural audio playback capability

2. **Add HRTF Plugin to SOTF** (2-3 weeks):
   - New `plugin_hrtf.rs` for binaural rendering
   - Load SOFA files from disk
   - Real-time convolution with head tracking
   - **Value**: Spatial audio feature for SOTF

3. **Evaluate NumCalc Rewrite** (1-2 months):
   - Deep analysis of C++ code
   - Test `bempp-rs` capabilities
   - Prototype basic BEM solver
   - **Decision point**: Rust rewrite vs. FFI wrapper

4. **If feasible, implement Mesh2Input + basic NumCalc** (4-6 months):
   - Complete Rust pipeline for small meshes
   - Validate against reference data
   - **Value**: Rust-native HRTF generation

5. **Long-term: FMM optimization** (12+ months):
   - Only if basic version proves valuable
   - Consider collaboration with `bempp-rs` team
   - **Value**: Production-scale mesh processing

---

## Conclusion

### TL;DR

| Question | Answer |
|----------|--------|
| **Can it be done?** | Yes, absolutely |
| **Is it easy?** | No, NumCalc is very hard |
| **Is it worth it?** | Depends on your goals |
| **Should you do it?** | Start small, validate value |

### The "Easy" Parts (Your Assessment)

You said:
> "the bem part looks easy"

**My assessment**: ⚠️ **Respectfully disagree**

- Basic BEM: 🟡 Medium (4-6 months with expertise)
- Burton-Miller BEM: 🔴 Hard (6-9 months)
- FMM: 🔴🔴 Very Hard (12-24 months)

The 144 KB integration constants file and 554 KB C++ codebase suggest this is not a simple port. However, starting without FMM and using `bempp-rs` could reduce complexity significantly.

### The Path Forward

**Best approach for SOTF project**:

1. ✅ **Leverage Mesh2HRTF as-is** for HRTF generation
2. ✅ **Build Rust SOFA reader** for SOTF binaural plugin
3. ⚠️ **Prototype Mesh2Input + Output2HRTF** in Rust
4. 🔴 **Defer NumCalc rewrite** until proven necessary

**When to commit to full rewrite**:
- You have BEM expertise in-house or can hire consultants
- The project value justifies 12-24 months of development
- `bempp-rs` matures with FMM support
- You want to contribute to Rust scientific computing ecosystem

---

## Resources

### Mesh2HRTF Documentation
- GitHub: https://github.com/Any2HRTF/Mesh2HRTF
- Website: https://www.mesh2hrtf.org
- SourceForge Wiki: https://sourceforge.net/p/mesh2hrtf/wiki/

### Rust Crates
- BEM: https://crates.io/crates/bempp
- Linear Algebra: https://crates.io/crates/ndarray
- Mesh: https://crates.io/crates/meshx
- HDF5: https://crates.io/crates/hdf5
- FFT: https://crates.io/crates/rustfft

### BEM Theory
- "NumCalc: An open-source BEM code for solving acoustic scattering problems" (2024)
- "Fast multipole boundary element method to calculate head-related transfer functions"
- Burton & Miller, "The application of integral equation methods to the numerical solution of some exterior boundary-value problems" (1971)

### Collaboration
- bempp-rs GitHub: https://github.com/bempp/bempp-rs
- Consider contributing FMM to bempp-rs rather than solo implementation

---

**Analysis Date**: November 21, 2025
**Author**: Claude Code
**Project Context**: SOTF (Sound of the Future) audio framework integration
