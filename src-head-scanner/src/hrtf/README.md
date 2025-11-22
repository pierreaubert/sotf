# HRTF Processing Module

Complete implementation of HRTF (Head-Related Transfer Function) post-processing for Mesh2HRTF output.

## Overview

This module processes NumCalc BEM (Boundary Element Method) simulation output to generate industry-standard SOFA (Spatially Oriented Format for Acoustics) files containing HRTFs and HRIRs.

**Complete Pipeline:**
```
3D Head Mesh → Evaluation Grids → NC.inp → NumCalc BEM →
be.out → HRTF Data → HRIR → SOFA File
```

## Module Structure

```
src/hrtf/
├── README.md            - This file
├── mod.rs               - Module exports and documentation
├── types.rs             - Data structures (PressureData, VelocityData, HrtfData, HrirData)
├── numcalc_parser.rs    - Parse NumCalc be.out files (Sprint 4)
├── hrir.rs              - HRIR computation via inverse FFT (Sprint 5)
└── sofa_writer.rs       - SOFA file export (Sprint 6)
```

## Features

### Sprint 4: NumCalc Output Parsing

**Parse BEM simulation results:**
- Read `be.out/be.{1..N}/` directory structure
- Parse complex pressure data (`pEvalGrid`, `pBoundary`)
- Parse velocity data (`vEvalGrid`, `vBoundary`)
- Compute velocity magnitudes from 3D complex vectors
- Support multi-frequency simulations

**File Format:**
```
NumCalc/source_N/
└── be.out/
    ├── be.1/               # Frequency 1
    │   ├── pEvalGrid       # Pressure on evaluation points: <node_id> <real> <imag>
    │   ├── pBoundary       # Pressure on mesh boundary: <node_id> <real> <imag>
    │   ├── vEvalGrid       # Velocity vector: <node_id> <rx> <ix> <ry> <iy> <rz> <iz>
    │   └── vBoundary       # Velocity magnitude: <node_id> <real> <imag>
    ├── be.2/               # Frequency 2
    ...
```

### Sprint 5: HRIR Computation

**Convert frequency-domain HRTFs to time-domain HRIRs:**
- Inverse real FFT with conjugate symmetry
- DC bin addition (0 Hz = 1.0, since HRTF is 0 dB at DC)
- Nyquist frequency real-valued enforcement
- Circular shift for causality
- Windowing functions (Hann, Hamming, Blackman)

**Algorithm:**
```rust
1. Add 0 Hz bin (DC = 1.0)
2. Make Nyquist frequency real-valued: Im(f_nyq) = 0
3. Create full spectrum via complex conjugate mirroring
4. Apply inverse FFT
5. Circular shift for causality (move peak away from t=0)
6. Optional windowing (Hann, Hamming, or Blackman)
```

### Sprint 6: SOFA File Export

**Write industry-standard SOFA files:**
- SimpleFreeFieldHRIR convention (AES69-2022, SOFA 2.1)
- netCDF-4 format (HDF5-based)
- Coordinate transformations (Cartesian ↔ Spherical)
- Complete metadata handling
- Multi-measurement support

**SOFA Structure:**
```
SOFA File (.sofa)
├── Global Attributes
│   ├── Conventions: "SOFA"
│   ├── Version: "2.1"
│   ├── SOFAConventions: "SimpleFreeFieldHRIR"
│   ├── DataType: "FIR"
│   └── ... (metadata)
│
├── Dimensions
│   ├── M (measurements)
│   ├── R (receivers, typically 2 ears)
│   ├── N (samples in HRIR)
│   └── C (coordinates, always 3)
│
├── Data Variables
│   ├── Data.IR [M, R, N]           - Impulse responses
│   ├── Data.SamplingRate           - Sample rate (Hz)
│   └── Data.Delay [M, R]           - Delays (samples)
│
└── Position Variables
    ├── SourcePosition [M, C]       - Source positions
    ├── ReceiverPosition [R, C]     - Ear positions (±9cm)
    ├── ListenerPosition [M, C]     - Listener at origin
    ├── ListenerView [M, C]         - Forward direction
    └── ListenerUp [M, C]           - Up direction
```

## API Usage

### Complete Pipeline

```rust
use head_scanner::hrtf::*;
use ndarray::Array2;

// Step 1: Parse NumCalc output
let mut parser = NumCalcParser::new("/path/to/project")?;
let hrtf_data = parser.parse_source(0)?;  // Parse source 1

// Step 2: Compute HRIRs
let sample_rate = 48000.0;
let n_shift = 128;  // Circular shift amount for causality
let hrir_data = compute_hrir(&hrtf_data.eval_pressure, sample_rate, n_shift)?;

// Step 3: Prepare source positions
// Assume we have M measurement positions in Cartesian coordinates
let source_positions = Array2::from_shape_vec((M, 3), vec![
    // x,    y,    z
    0.0,  1.5,  0.0,  // Front
    1.5,  0.0,  0.0,  // Left
    0.0, -1.5,  0.0,  // Back
   -1.5,  0.0,  0.0,  // Right
])?;

// Step 4: Export to SOFA
let metadata = SofaMetadata {
    title: "My HRTF Dataset".to_string(),
    database_name: Some("MyDatabase".to_string()),
    listener_short_name: Some("Subject01".to_string()),
    author_contact: "researcher@example.com".to_string(),
    organization: "Research Lab".to_string(),
    license: "CC-BY-4.0".to_string(),
    application_name: "head-scanner".to_string(),
    application_version: env!("CARGO_PKG_VERSION").to_string(),
    comment: Some("Generated from BEM simulation".to_string()),
};

let writer = SofaWriter::new()
    .with_metadata(metadata)
    .with_coordinate_system(CoordinateSystem::Spherical)
    .with_room_type("free field".to_string());

writer.write_hrir(&hrir_data, &source_positions, "output.sofa")?;
```

### Parse NumCalc Output Only

```rust
use head_scanner::hrtf::NumCalcParser;

// Parse single source
let mut parser = NumCalcParser::new("/path/to/project")?;
let data = parser.parse_source(0)?;

println!("Frequencies: {:?}", data.eval_pressure.frequencies);
println!("Eval points: {}", data.eval_pressure.node_ids.len());
println!("Pressure shape: {:?}", data.eval_pressure.pressure.shape());
```

### Compute HRIR with Custom Parameters

```rust
use head_scanner::hrtf::{compute_hrir, apply_hann_window};

// Compute HRIR at different sample rates
let hrir_44100 = compute_hrir(&pressure_data, 44100.0, 128)?;
let hrir_48000 = compute_hrir(&pressure_data, 48000.0, 128)?;

// Apply windowing to reduce time-domain artifacts
let mut hrir = compute_hrir(&pressure_data, 48000.0, 128)?;
for i in 0..hrir.num_points() {
    let mut ir_slice = hrir.impulse_response.row_mut(i);
    let mut ir_vec: Vec<f64> = ir_slice.to_vec();
    apply_hann_window(&mut ir_vec);
    ir_slice.assign(&ndarray::Array1::from(ir_vec));
}
```

### Export with Different Coordinate Systems

```rust
use head_scanner::hrtf::{SofaWriter, CoordinateSystem};

// Cartesian coordinates
let writer_cart = SofaWriter::new()
    .with_coordinate_system(CoordinateSystem::Cartesian);
writer_cart.write_hrir(&hrir, &cartesian_positions, "cartesian.sofa")?;

// Spherical coordinates
let writer_sph = SofaWriter::new()
    .with_coordinate_system(CoordinateSystem::Spherical);
writer_sph.write_hrir(&hrir, &cartesian_positions, "spherical.sofa")?;
```

## Data Types

### PressureData

Stores complex pressure values from BEM simulation.

```rust
pub struct PressureData {
    pub pressure: Array2<Complex64>,  // [points × frequencies]
    pub node_ids: Vec<usize>,
    pub frequencies: Vec<f64>,
}
```

### VelocityData

Stores velocity magnitude values.

```rust
pub struct VelocityData {
    pub velocity: Array2<f64>,        // [points × frequencies]
    pub node_ids: Vec<usize>,
    pub frequencies: Vec<f64>,
}
```

### HrtfData

Complete HRTF dataset from NumCalc output.

```rust
pub struct HrtfData {
    pub eval_pressure: PressureData,
    pub eval_velocity: Option<VelocityData>,
    pub boundary_pressure: Option<PressureData>,
    pub boundary_velocity: Option<VelocityData>,
    pub source_index: usize,
    pub speed_of_sound: f64,
    pub density: f64,
}
```

### HrirData

Time-domain impulse responses.

```rust
pub struct HrirData {
    pub impulse_response: Array2<f64>,  // [points × samples]
    pub sample_rate: f64,
    pub node_ids: Vec<usize>,
}

impl HrirData {
    pub fn get_ir(&self, point_index: usize) -> Vec<f64>;
    pub fn num_samples(&self) -> usize;
    pub fn num_points(&self) -> usize;
    pub fn duration(&self) -> f64;  // Duration in seconds
}
```

## Coordinate Systems

### Cartesian

Standard 3D Cartesian coordinates in meters:
- **x**: Left (-) to Right (+)
- **y**: Back (-) to Front (+)
- **z**: Down (-) to Up (+)

### Spherical

Spherical coordinates commonly used in acoustics:
- **Azimuth** (degrees): Horizontal angle
  - 0° = Front (+y axis)
  - 90° = Left (+x axis)
  - ±180° = Back (-y axis)
  - -90° = Right (-x axis)
- **Elevation** (degrees): Vertical angle
  - 0° = Horizontal plane
  - 90° = Up (+z axis)
  - -90° = Down (-z axis)
- **Radius** (meters): Distance from origin

### Conversion Functions

```rust
use head_scanner::hrtf::{cartesian_to_spherical, spherical_to_cartesian};

// Convert Cartesian to Spherical
let (azimuth, elevation, radius) = cartesian_to_spherical(x, y, z);

// Convert Spherical to Cartesian
let (x, y, z) = spherical_to_cartesian(azimuth, elevation, radius);

// Round-trip accuracy: < 1e-10
```

## Windowing Functions

Apply windowing to HRIRs to reduce time-domain artifacts:

### Hann Window
```rust
use head_scanner::hrtf::apply_hann_window;

let mut hrir = vec![/* samples */];
apply_hann_window(&mut hrir);

// w(n) = 0.5 * (1 - cos(2π*n/N))
// Good general-purpose window, near-zero at endpoints
```

### Hamming Window
```rust
use head_scanner::hrtf::apply_hamming_window;

let mut hrir = vec![/* samples */];
apply_hamming_window(&mut hrir);

// w(n) = 0.54 - 0.46 * cos(2π*n/N)
// Non-zero at endpoints, better frequency selectivity than Hann
```

### Blackman Window
```rust
use head_scanner::hrtf::apply_blackman_window;

let mut hrir = vec![/* samples */];
apply_blackman_window(&mut hrir);

// w(n) = 0.42 - 0.5*cos(2π*n/N) + 0.08*cos(4π*n/N)
// Best frequency selectivity, lowest energy retention
```

## Validation

All sprints have been validated with comprehensive test scripts:

```bash
# Individual sprint validation
python3 scripts/test_sprint4.py  # NumCalc parsing
python3 scripts/test_sprint5.py  # HRIR computation
python3 scripts/test_sprint6.py  # SOFA export

# End-to-end integration
python3 scripts/test_sprint7_integration.py
```

## Dependencies

```toml
[dependencies]
ndarray = { version = "0.16.1", features = ["rayon", "serde"], default-features = false }
num-complex = { workspace = true }
rustfft = { workspace = true }
netcdf = "0.11"
chrono = { workspace = true }
anyhow = "1.0"

[dev-dependencies]
approx = "0.5"
```

## File Format Specifications

### NumCalc be.out Files

**Pressure Files** (`pEvalGrid`, `pBoundary`):
```
<node_id> <real> <imag>
1 0.123456 -0.234567
2 0.345678 0.456789
...
```

**Velocity Files**:
- `vBoundary`: `<node_id> <real> <imag>` (magnitude only)
- `vEvalGrid`: `<node_id> <real_x> <imag_x> <real_y> <imag_y> <real_z> <imag_z>`

Velocity magnitude computed as:
```
|v| = sqrt(|v_x|^2 + |v_y|^2 + |v_z|^2)
where |v_x| = sqrt(real_x^2 + imag_x^2)
```

### SOFA Files

SOFA files are netCDF-4 format (HDF5-based) following the SimpleFreeFieldHRIR convention.

**Validate SOFA files:**
```bash
# Using netCDF tools
ncdump -h output.sofa

# Using HDF5 tools
h5dump output.sofa

# Using Python
python3 -c "
import netCDF4
ds = netCDF4.Dataset('output.sofa', 'r')
print(ds.variables.keys())
print('IR shape:', ds.variables['Data.IR'].shape)
"
```

## Numerical Concepts

### HRTF (Head-Related Transfer Function)

Frequency-domain representation of how sound is filtered by head and ear geometry:
- **Complex-valued**: Magnitude and phase at each frequency
- **Frequency range**: Typically 20 Hz - 20 kHz
- **Resolution**: Determined by BEM simulation (e.g., 60 frequencies)
- **Referencing**: Can be referenced to head center (minimum/linear phase)

### HRIR (Head-Related Impulse Response)

Time-domain representation obtained via inverse FFT:
- **Real-valued**: Time-domain samples
- **Sample rate**: User-defined (e.g., 44.1 kHz, 48 kHz)
- **Length**: Determined by frequency resolution (N = sample_rate / freq_step)
- **Causality**: Enforced via circular shift

### Inverse Real FFT

Efficient FFT for real-valued output:
1. Input: Complex spectrum with Hermitian symmetry
2. Output: Real-valued time-domain signal
3. Conjugate symmetry: X[k] = conj(X[N-k]) for k=1..N/2-1
4. DC (k=0) and Nyquist (k=N/2) must be real-valued

## Examples

See `scripts/test_sprint7_integration.py` for comprehensive usage examples.

## Future Extensions

1. **HRTF Referencing**: Reference to head center (minimum/linear phase)
2. **Diffuse Field Equalization**: Normalize for diffuse field response
3. **Phase Unwrapping**: Continuous phase for better interpolation
4. **Interpolation**: Spatial interpolation between measurement points
5. **Additional SOFA Conventions**: SimpleFreeFieldHRTF, MultiSpeakerBRIR
6. **Validation**: Analytical validation with rigid sphere (Mie theory)

## References

- **Mesh2HRTF**: https://github.com/Any2HRTF/Mesh2HRTF
- **SOFA Format**: https://www.sofaconventions.org/
- **AES69-2022**: SOFA Conventions 2.1
- **NumCalc**: Boundary Element Method solver
- **libmysofa**: C library for reading SOFA files
- **pysofar**: Python SOFA library

## License

This code is part of the head-scanner crate. See repository root for license information.
