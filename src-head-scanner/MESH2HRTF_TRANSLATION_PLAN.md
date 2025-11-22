# Mesh2Input and Output2HRTF Translation to Rust

**Status**: Analysis and Planning Phase
**Target Crate**: `src-head-scanner`
**Source**: Mesh2HRTF Python codebase

## Overview

The goal is to translate the Mesh2Input and Output2HRTF components from Python to Rust, integrating them into the existing `src-head-scanner` crate to create a complete HRTF calculation pipeline.

## Current Mesh2HRTF Pipeline

### Complete Workflow

```
1. Mesh2Input (Python/Blender)
   ↓
   3D Head Mesh + NC.inp + Evaluation Grids
   ↓
2. NumCalc (C++ BEM Solver)  [✓ Now wrapped via FFI in src-bem]
   ↓
   Pressure Data (be.out, fe.out, NC.out)
   ↓
3. Output2HRTF (Python)
   ↓
   SOFA Files (HRTF/HRIR Data)
```

## Component Analysis

### 1. Mesh2Input - Input Preparation

**Purpose**: Convert 3D head meshes into NumCalc-compatible format

**Current Implementation**: Blender addon (`mesh2input.py`, 2500+ lines)

**Key Responsibilities:**

1. **Mesh Processing**
   - Read head mesh from Blender scene
   - Identify ear regions (left ear, right ear materials)
   - Extract vertices and triangular elements
   - Validate mesh quality (manifold, watertight, normals)

2. **Evaluation Grid Generation**
   - Create spherical grids around head
   - Create horizontal/vertical plane grids
   - Custom grid definitions
   - Grid positioning and orientation

3. **Source Configuration**
   - Source types:
     - Both ears (velocity boundary condition on ear elements)
     - Left ear only
     - Right ear only
     - Point source (analytical)
     - Plane wave (analytical)
   - Source parameters (position, area, velocity)

4. **NC.inp File Generation**
   - Control parameters (method, frequencies, convergence)
   - Mesh references (Nodes.txt, Elements.txt)
   - Boundary conditions
   - Material properties (speed of sound, air density)

5. **Project Structure Creation**
   ```
   project/
   ├── ObjectMeshes/
   │   └── Reference/
   │       ├── Nodes.txt       (vertices: x, y, z)
   │       └── Elements.txt    (triangles: v1, v2, v3)
   ├── EvaluationGrids/
   │   ├── HorPlane/
   │   │   ├── Nodes.txt
   │   │   └── Elements.txt
   │   └── ...
   ├── NumCalc/
   │   ├── source_1/
   │   │   └── NC.inp
   │   └── source_2/
   │       └── NC.inp
   ├── parameters.json         (metadata)
   └── 3d Model.blend          (optional, original)
   ```

6. **Mesh File Formats**
   - **Nodes.txt**: `<node_id> <x> <y> <z>`
   - **Elements.txt**: `<element_id> <material_id> <v1> <v2> <v3>`

### 2. Output2HRTF - Post-Processing

**Purpose**: Convert BEM output to SOFA HRTF files

**Current Implementation**: Python library (`output2hrtf.py`, ~1000 lines)

**Key Responsibilities:**

1. **NumCalc Output Parsing**
   - Read `NC*.out` files (simulation logs)
   - Read `be.out/be.*/pEvalGrid` (pressure on evaluation grids)
   - Read `be.out/be.*/pBoundary` (pressure on object mesh)
   - Parse convergence information
   - Detect and report issues

2. **Data Processing**
   - Complex pressure data for each frequency
   - Organize by source and evaluation grid
   - Handle multiple sources (left/right ear)

3. **HRTF Referencing**
   - Reference to center of head (classic HRTF definition)
   - Modes: "min" (minimum phase), "linear" (linear phase)
   - Account for source type (point source, piston, etc.)

4. **HRIR Computation**
   - Inverse Fourier Transform (HRTF → HRIR)
   - Windowing and truncation
   - Time-domain shifting (typically 30cm equivalent)
   - Requires referenced HRTFs

5. **SOFA File Generation**
   - SOFA (Spatially Oriented Format for Acoustics)
   - HDF5-based format
   - Metadata (conventions, coordinates, units)
   - Data organization (M sources, R receivers, N samples)
   - Coordinate transformations (cartesian ↔ spherical)

6. **Report Generation**
   - CSV reports with convergence info
   - Issue detection (non-convergence, missing data)
   - Warnings and errors

## Rust Translation Strategy

### Phase 1: Core Data Structures

Create Rust equivalents for mesh and HRTF data:

```rust
// Mesh representation
pub struct Mesh {
    pub nodes: Vec<Node>,
    pub elements: Vec<Element>,
    pub metadata: MeshMetadata,
}

pub struct Node {
    pub id: usize,
    pub position: Point3<f64>,
}

pub struct Element {
    pub id: usize,
    pub material_id: usize,
    pub vertices: [usize; 3],  // Triangle
}

// Evaluation grid
pub struct EvaluationGrid {
    pub name: String,
    pub nodes: Vec<Node>,
    pub elements: Vec<Element>,
    pub grid_type: GridType,
}

pub enum GridType {
    HorizontalPlane,
    VerticalPlane,
    Sphere { radius: f64 },
    Custom,
}

// NumCalc configuration
pub struct NumCalcProject {
    pub title: String,
    pub method: BemMethod,
    pub source_type: SourceType,
    pub object_mesh: Mesh,
    pub evaluation_grids: Vec<EvaluationGrid>,
    pub frequencies: Vec<f64>,
    pub parameters: NumCalcParameters,
}

pub enum BemMethod {
    Bem,
    SlFmmBem,  // Single-level FMM
    MlFmmBem,  // Multi-level FMM
}

pub enum SourceType {
    BothEars { left_material: usize, right_material: usize },
    LeftEar { material: usize },
    RightEar { material: usize },
    PointSource { position: Point3<f64> },
    PlaneWave { direction: Vector3<f64> },
}

// HRTF data structures
pub struct HrtfData {
    pub frequencies: Vec<f64>,
    pub sources: Vec<SourcePosition>,
    pub receivers: Vec<ReceiverPosition>,
    pub pressure: ndarray::Array3<Complex64>,  // [M, R, N]
    pub metadata: HrtfMetadata,
}

pub struct HrirData {
    pub sample_rate: f64,
    pub sources: Vec<SourcePosition>,
    pub receivers: Vec<ReceiverPosition>,
    pub impulse_response: ndarray::Array3<f64>,  // [M, R, N]
    pub metadata: HrtfMetadata,
}
```

### Phase 2: Mesh Processing (mesh2input functionality)

**Module**: `src-head-scanner/src/mesh2hrtf/` (new)

**Files to create:**

1. **`mesh_io.rs`** - Mesh file I/O
   - Read/write Nodes.txt and Elements.txt
   - Read common 3D formats (OBJ, STL, PLY)
   - Mesh validation (manifold, watertight, orientation)

2. **`evaluation_grid.rs`** - Grid generation
   - Spherical grids (Lebedev, uniform angular)
   - Planar grids (horizontal, vertical)
   - Custom grid positioning

3. **`source_config.rs`** - Source definition
   - Ear region detection
   - Source type configuration
   - Boundary condition setup

4. **`nc_inp_writer.rs`** - NC.inp generation
   - Format NC.inp file
   - Write control parameters
   - Write boundary conditions

5. **`project_builder.rs`** - Project creation
   - Directory structure setup
   - File copying and organization
   - parameters.json generation

**Dependencies:**
- `nalgebra` - Linear algebra (already in workspace)
- `ndarray` - N-dimensional arrays (already in workspace)
- `serde` + `serde_json` - Serialization (already in workspace)
- `std::fs` - File operations (stdlib)

**Integration with src-head-scanner:**

The existing src-head-scanner creates 3D head meshes via scanning. We extend it to:
1. Export scanned mesh to Mesh2HRTF format
2. Generate evaluation grids
3. Create complete NumCalc project

```rust
// In src-head-scanner/src/lib.rs

pub mod mesh2hrtf {
    pub mod mesh_io;
    pub mod evaluation_grid;
    pub mod source_config;
    pub mod nc_inp_writer;
    pub mod project_builder;
}

// Example usage
use head_scanner::mesh2hrtf::*;

// After scanning, export to Mesh2HRTF format
let mesh = scanner.generate_mesh()?;

// Convert to Mesh2HRTF project
let project = ProjectBuilder::new()
    .with_mesh(mesh)
    .with_evaluation_grid(GridType::Sphere { radius: 1.5 })
    .with_source_type(SourceType::BothEars { ... })
    .with_frequencies(vec![200.0, 400.0, ..., 20000.0])
    .build()?;

// Export project
project.export("/path/to/project")?;
```

### Phase 3: HRTF Post-Processing (output2hrtf functionality)

**Module**: `src-head-scanner/src/hrtf/` (new)

**Files to create:**

1. **`numcalc_parser.rs`** - Parse NumCalc output
   - Read NC*.out files
   - Parse be.out/pEvalGrid, pBoundary, vBoundary
   - Detect convergence issues
   - Build HrtfData from raw output

2. **`hrtf_processing.rs`** - HRTF operations
   - Reference to head center
   - Phase unwrapping
   - Interpolation and resampling
   - Diffuse field equalization

3. **`hrir_computation.rs`** - HRIR calculation
   - Inverse FFT (HRTF → HRIR)
   - Windowing (Hann, Hamming, Blackman)
   - Time-domain shifting
   - Minimum phase computation

4. **`sofa_writer.rs`** - SOFA file export
   - HDF5 file writing
   - SOFA conventions (SimpleFreeFieldHRIR, etc.)
   - Metadata management
   - Coordinate transformations

5. **`report_generator.rs`** - Reporting
   - CSV report generation
   - Issue detection and warnings
   - Visualization data export (for plotly.js)

**Dependencies:**
- `hdf5` - HDF5 file format (for SOFA)
- `rustfft` - FFT operations (already in workspace via src-audio)
- `ndarray` - Array operations (already in workspace)
- `num-complex` - Complex numbers (already in workspace)
- `csv` - CSV writing

**Integration:**

```rust
use head_scanner::hrtf::*;

// After NumCalc simulation (via src-bem FFI)
let project_dir = "/path/to/project";

// Parse NumCalc output
let parser = NumCalcParser::new(project_dir)?;
let raw_data = parser.parse_all_sources()?;

// Process HRTFs
let mut hrtf = HrtfData::from_numcalc(raw_data)?;
hrtf.reference_to_head_center(ReferenceMode::MinimumPhase)?;

// Export to SOFA
let sofa_file = "/path/to/output.sofa";
SofaWriter::new()
    .with_convention("SimpleFreeFieldHRIR")
    .write(&hrtf, sofa_file)?;

// Compute HRIRs
let hrir = compute_hrir(&hrtf, 48000.0, 512)?;
SofaWriter::new()
    .with_convention("SimpleFreeFieldHRIR")
    .write(&hrir, "/path/to/hrir.sofa")?;

// Generate report
let report = ReportGenerator::new()
    .analyze_convergence(&raw_data)
    .export_csv("/path/to/report.csv")?;
```

## Implementation Roadmap

### Sprint 1: Foundation (1-2 weeks)

**Goal**: Basic data structures and mesh I/O

- [ ] Create `src-head-scanner/src/mesh2hrtf/` module
- [ ] Implement `Mesh`, `Node`, `Element` types
- [ ] Implement `mesh_io.rs` (read/write Nodes.txt, Elements.txt)
- [ ] Unit tests for mesh I/O
- [ ] Integration test with real Mesh2HRTF project

**Deliverables**:
- Read/write mesh files in Mesh2HRTF format
- Validate mesh quality
- 100% test coverage for I/O

### Sprint 2: Evaluation Grids (1 week)

**Goal**: Generate evaluation grids

- [ ] Implement `evaluation_grid.rs`
- [ ] Spherical grid generation (Lebedev quadrature)
- [ ] Planar grid generation (horizontal, vertical)
- [ ] Grid positioning and orientation
- [ ] Tests with analytical validation

**Deliverables**:
- Generate standard evaluation grids
- Match Mesh2HRTF grid conventions
- Visualization output (JSON for plotly.js)

### Sprint 3: Project Creation (1 week)

**Goal**: Generate complete NumCalc projects

- [ ] Implement `source_config.rs`
- [ ] Implement `nc_inp_writer.rs`
- [ ] Implement `project_builder.rs`
- [ ] Integration with src-bem FFI wrapper
- [ ] End-to-end test (mesh → project → NumCalc ready)

**Deliverables**:
- Complete project generation
- Compatible with NumCalc FFI wrapper
- Integration tests with src-bem

### Sprint 4: HRTF Post-Processing (2 weeks)

**Goal**: Parse NumCalc output and generate HRTFs

- [ ] Create `src-head-scanner/src/hrtf/` module
- [ ] Implement `numcalc_parser.rs`
- [ ] Implement `hrtf_processing.rs`
- [ ] Reference to head center (minimum phase)
- [ ] Integration tests with real NumCalc output

**Deliverables**:
- Parse be.out files
- HRTF referencing working
- Validated against Python output2hrtf

### Sprint 5: HRIR Computation (1 week)

**Goal**: Time-domain impulse responses

- [ ] Implement `hrir_computation.rs`
- [ ] Inverse FFT (frequency → time domain)
- [ ] Windowing and truncation
- [ ] Time shifting
- [ ] Validate against analytical solutions

**Deliverables**:
- HRIR computation working
- Match Python output (within numerical tolerance)

### Sprint 6: SOFA Export (1-2 weeks)

**Goal**: Write industry-standard SOFA files

- [ ] Implement `sofa_writer.rs`
- [ ] HDF5 file creation
- [ ] SOFA conventions implementation
- [ ] Metadata management
- [ ] Coordinate transformations
- [ ] Validate with SOFA validators

**Deliverables**:
- Write SOFA files
- Pass SOFA validation
- Compatible with spatial audio software

### Sprint 7: Reporting and Documentation (1 week)

**Goal**: Production-ready system

- [ ] Implement `report_generator.rs`
- [ ] CSV reports
- [ ] Issue detection and warnings
- [ ] Complete documentation
- [ ] Examples and tutorials
- [ ] CLI tool for complete pipeline

**Deliverables**:
- Comprehensive reporting
- User documentation
- CLI tool: `head-scanner-cli hrtf`

## Technical Challenges and Solutions

### Challenge 1: SOFA Format Complexity

**Problem**: SOFA is HDF5-based with complex conventions

**Solution**:
- Use `hdf5` crate for file I/O
- Study existing SOFA implementations (pysofar, libmysofa)
- Focus on SimpleFreeFieldHRIR convention first
- Validate with SOFA Matlab/Python tools

### Challenge 2: Numerical Accuracy

**Problem**: Must match Python output within numerical tolerance

**Solution**:
- Use analytical test cases (spheres with Mie theory)
- Validate each step (parsing, referencing, IFFT)
- Use `approx` crate for floating-point comparisons
- Property-based testing with `proptest`

### Challenge 3: Blender Integration

**Problem**: Mesh2Input is currently a Blender addon

**Solution**:
- **Not translating Blender UI** - src-head-scanner creates meshes directly
- Read common 3D formats (OBJ, STL, PLY) instead
- Provide CLI for mesh processing outside Blender
- Integration: Scanner → Mesh → Mesh2HRTF format

### Challenge 4: Large Data Handling

**Problem**: HRTF data can be large (many sources × receivers × frequencies)

**Solution**:
- Memory-mapped I/O for large files
- Streaming processing where possible
- Rayon parallelism for processing (already in workspace)
- Progress reporting for long operations

## Dependencies

**New dependencies needed:**

```toml
# In src-head-scanner/Cargo.toml
[dependencies]
# ... existing dependencies ...

# SOFA/HDF5 support
hdf5 = "0.8"

# CSV reporting
csv = "1.2"

# FFT (may already be available via src-audio)
rustfft = "6.0"

# Mesh processing (if not already available)
obj = "0.10"  # OBJ file support
stl_io = "0.7"  # STL file support
ply-rs = "0.1"  # PLY file support
```

## Validation Strategy

### Level 1: Unit Tests

- Mesh I/O round-trip tests
- Grid generation validation
- NC.inp parsing tests
- NumCalc output parsing tests
- HRTF processing tests

### Level 2: Integration Tests

- Complete project creation
- NumCalc execution via src-bem FFI
- Output parsing → SOFA export
- End-to-end pipeline tests

### Level 3: Analytical Validation

- Rigid sphere scattering (Mie theory)
- Compare BEM HRTF vs analytical
- Validate HRIR via inverse FFT

### Level 4: Reference Validation

- Compare with Python Mesh2HRTF output
- Bit-for-bit comparison where possible
- Numerical tolerance testing (<1e-6 relative error)

### Level 5: Production Validation

- Real head mesh simulations
- SOFA file validation (SOFA validators)
- Compatibility with spatial audio tools (Blender, Unity, Unreal)

## Documentation Requirements

1. **API Documentation** (rustdoc)
   - All public types and functions
   - Usage examples
   - Mathematical background

2. **User Guide**
   - Complete pipeline tutorial
   - CLI usage examples
   - Troubleshooting guide

3. **Developer Guide**
   - Architecture overview
   - Adding new grid types
   - Extending SOFA conventions

4. **Integration Guide**
   - src-head-scanner → src-bem → SOFA pipeline
   - Using with spatial audio engines

## Success Criteria

- ✅ Read/write Mesh2HRTF mesh format
- ✅ Generate standard evaluation grids
- ✅ Create complete NumCalc projects
- ✅ Parse NumCalc output
- ✅ Reference HRTFs to head center
- ✅ Compute HRIRs via IFFT
- ✅ Export valid SOFA files
- ✅ Match Python output within tolerance (<1e-6)
- ✅ Pass SOFA validation
- ✅ Complete end-to-end pipeline working
- ✅ Comprehensive test coverage (>90%)
- ✅ Production documentation

## Timeline Estimate

**Total**: 8-10 weeks (full-time equivalent)

- Sprint 1 (Foundation): 1-2 weeks
- Sprint 2 (Grids): 1 week
- Sprint 3 (Projects): 1 week
- Sprint 4 (HRTF Processing): 2 weeks
- Sprint 5 (HRIR Computation): 1 week
- Sprint 6 (SOFA Export): 1-2 weeks
- Sprint 7 (Polish): 1 week

**Total Lines of Code Estimate**: ~8,000-10,000 lines

- Mesh I/O: ~800 lines
- Evaluation grids: ~600 lines
- Project creation: ~1,000 lines
- NumCalc parsing: ~1,200 lines
- HRTF processing: ~1,500 lines
- HRIR computation: ~800 lines
- SOFA export: ~1,500 lines
- Tests: ~2,500 lines
- Documentation: inline + guides

## Next Immediate Steps

1. **Create module structure** in src-head-scanner
2. **Implement basic mesh I/O** (Nodes.txt, Elements.txt)
3. **Add integration test** with real Mesh2HRTF data
4. **Validate round-trip** (read → write → read)

## References

- **Mesh2HRTF**: https://github.com/Any2HRTF/Mesh2HRTF
- **SOFA Format**: https://www.sofaconventions.org/
- **libmysofa**: https://github.com/hoene/libmysofa
- **pysofar**: https://pysofar.readthedocs.io/
- **BEM Theory**: See src-bem/README.md
