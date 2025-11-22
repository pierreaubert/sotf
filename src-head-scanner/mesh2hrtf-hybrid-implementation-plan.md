# Mesh2HRTF Hybrid Implementation Plan
## Rust Wrapper with C++ NumCalc via FFI

**Date**: November 22, 2025
**Approach**: Pragmatic hybrid - Rust for I/O, C++ NumCalc via subprocess/FFI
**Timeline**: 8-12 weeks for complete pipeline

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Rust Workspace: src-mesh2hrtf/               │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────┐      ┌──────────────────┐      ┌────────────────────┐
│  │  mesh2input      │ ───> │  numcalc-wrapper │ ───> │  output2hrtf       │
│  │  (Rust)          │      │  (Rust FFI)      │      │  (Rust)            │
│  └──────────────────┘      └──────────────────┘      └────────────────────┘
│       Rust                   C++ via FFI                   Rust            │
│     ~2 weeks                  ~2 weeks                    ~3 weeks         │
│                                                                             │
│                              ┌────────────────┐                            │
│                              │  NumCalc C++   │                            │
│                              │  (unchanged)   │                            │
│                              └────────────────┘                            │
│                                Compiled lib                                │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: NumCalc FFI Wrapper (2-3 weeks)

### Strategy: Subprocess Approach (Simplest)

**Why subprocess instead of direct FFI?**
- ✅ No C++ ABI compatibility issues
- ✅ NumCalc already designed as standalone executable
- ✅ manage_numcalc.py uses this approach successfully
- ✅ Easier cross-platform builds
- ⚠️ Slightly higher overhead (acceptable for batch processing)

### Implementation

#### 1.1 Build System Integration

**Goal**: Compile NumCalc C++ code as part of Rust build

```toml
# src-mesh2hrtf/Cargo.toml
[package]
name = "mesh2hrtf"
version = "0.1.0"

[dependencies]
# ... other deps

[build-dependencies]
cc = "1.0"  # For compiling C++ code
cmake = "0.1"  # Alternative: use CMake if available
```

**build.rs**:
```rust
// src-mesh2hrtf/build.rs
fn main() {
    // Option 1: Use existing Makefile
    println!("cargo:rerun-if-changed=NumCalc/src/");

    let status = std::process::Command::new("make")
        .current_dir("NumCalc/src")
        .status()
        .expect("Failed to compile NumCalc");

    if !status.success() {
        panic!("NumCalc compilation failed");
    }

    // Option 2: Use cc crate to compile C++ files
    // cc::Build::new()
    //     .cpp(true)
    //     .files(&["NumCalc/src/NC_Main.cpp", ...])
    //     .compile("numcalc");
}
```

#### 1.2 Rust Wrapper API

```rust
// src-mesh2hrtf/src/numcalc/mod.rs

use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Context, Result};

pub struct NumCalcRunner {
    executable_path: PathBuf,
    project_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct NumCalcConfig {
    pub freq_start_idx: Option<usize>,
    pub freq_end_idx: Option<usize>,
    pub max_iterations: Option<usize>,
    pub estimate_ram: bool,
    pub check_normals: bool,
}

#[derive(Debug)]
pub struct NumCalcOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub output_files: Vec<PathBuf>,
}

impl NumCalcRunner {
    pub fn new(project_dir: impl AsRef<Path>) -> Result<Self> {
        let executable = Self::find_executable()?;
        Ok(Self {
            executable_path: executable,
            project_dir: project_dir.as_ref().to_path_buf(),
        })
    }

    fn find_executable() -> Result<PathBuf> {
        // Look for NumCalc executable in:
        // 1. $CARGO_MANIFEST_DIR/target/numcalc
        // 2. System PATH
        // 3. Bundled binary

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let build_path = PathBuf::from(manifest_dir)
            .join("target")
            .join("numcalc");

        if build_path.exists() {
            return Ok(build_path);
        }

        // Try system PATH
        which::which("NumCalc")
            .context("NumCalc executable not found")
    }

    pub fn run(&self, config: &NumCalcConfig) -> Result<NumCalcOutput> {
        let mut cmd = Command::new(&self.executable_path);

        // Set working directory to project folder (where NC.inp is)
        cmd.current_dir(&self.project_dir);

        // Add command-line arguments
        if let Some(start) = config.freq_start_idx {
            cmd.arg("-istart").arg(start.to_string());
        }
        if let Some(end) = config.freq_end_idx {
            cmd.arg("-iend").arg(end.to_string());
        }
        if let Some(max_iter) = config.max_iterations {
            cmd.arg("-nitermax").arg(max_iter.to_string());
        }
        if config.estimate_ram {
            cmd.arg("-estimate_ram");
        }
        if config.check_normals {
            cmd.arg("-check_normals");
        }

        // Execute
        let output = cmd.output()
            .context("Failed to execute NumCalc")?;

        // Collect output files
        let output_files = self.collect_output_files()?;

        Ok(NumCalcOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            output_files,
        })
    }

    fn collect_output_files(&self) -> Result<Vec<PathBuf>> {
        let be_out_dir = self.project_dir.join("be.out");
        let fe_out_dir = self.project_dir.join("fe.out");

        let mut files = Vec::new();

        // Recursively find all output files
        for dir in [be_out_dir, fe_out_dir] {
            if dir.exists() {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    files.push(entry.path());
                }
            }
        }

        Ok(files)
    }

    pub fn estimate_memory(&self) -> Result<MemoryEstimate> {
        let config = NumCalcConfig {
            estimate_ram: true,
            ..Default::default()
        };

        self.run(&config)?;

        // Read Memory.txt
        let mem_file = self.project_dir.join("Memory.txt");
        let content = std::fs::read_to_string(mem_file)?;

        MemoryEstimate::parse(&content)
    }
}

#[derive(Debug)]
pub struct MemoryEstimate {
    pub total_mb: f64,
    pub per_frequency_mb: Vec<f64>,
}

impl MemoryEstimate {
    fn parse(content: &str) -> Result<Self> {
        // Parse Memory.txt format
        // TODO: Implement based on actual format
        todo!()
    }
}

// Parallel frequency execution (like manage_numcalc.py)
pub struct ParallelNumCalc {
    runner: NumCalcRunner,
    max_concurrent: usize,
    max_ram_gb: f64,
}

impl ParallelNumCalc {
    pub fn new(project_dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            runner: NumCalcRunner::new(project_dir)?,
            max_concurrent: num_cpus::get(),
            max_ram_gb: Self::get_available_ram(),
        })
    }

    fn get_available_ram() -> f64 {
        // Use sysinfo crate
        use sysinfo::{System, SystemExt};
        let sys = System::new_all();
        sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0  // GB
    }

    pub async fn run_all_frequencies(
        &self,
        num_frequencies: usize,
    ) -> Result<Vec<NumCalcOutput>> {
        // Estimate memory per frequency
        let estimates = self.runner.estimate_memory()?;

        // Schedule frequency steps based on available resources
        // Similar to manage_numcalc.py logic

        use tokio::task::JoinSet;
        let mut tasks = JoinSet::new();

        for freq_idx in 0..num_frequencies {
            // Check if we have enough RAM
            let required_ram = estimates.per_frequency_mb[freq_idx] / 1024.0;
            if required_ram > self.max_ram_gb {
                log::warn!(
                    "Frequency {} requires {:.1} GB but only {:.1} GB available",
                    freq_idx, required_ram, self.max_ram_gb
                );
            }

            // Launch task
            let runner = self.runner.clone();
            tasks.spawn(async move {
                let config = NumCalcConfig {
                    freq_start_idx: Some(freq_idx),
                    freq_end_idx: Some(freq_idx),
                    ..Default::default()
                };
                runner.run(&config)
            });

            // Limit concurrent tasks
            if tasks.len() >= self.max_concurrent {
                tasks.join_next().await;
            }
        }

        // Wait for all
        let mut results = Vec::new();
        while let Some(result) = tasks.join_next().await {
            results.push(result??);
        }

        Ok(results)
    }
}
```

---

## Phase 2: Mesh2Input in Rust (2-3 weeks)

### Overview

**Current**: Blender add-on (mesh2input.py)
**Target**: Standalone Rust CLI + library

**Key difference**: We bypass Blender and work directly with mesh files

### Implementation

#### 2.1 Mesh Loading

```rust
// src-mesh2hrtf/src/mesh2input/mesh.rs

use meshx::TriMesh;
use std::path::Path;

#[derive(Debug)]
pub struct HeadMesh {
    pub nodes: Vec<Node>,
    pub elements: Vec<Element>,
}

#[derive(Debug)]
pub struct Node {
    pub id: usize,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug)]
pub struct Element {
    pub id: usize,
    pub node_ids: [usize; 3],  // Triangle
    pub material_id: usize,
}

impl HeadMesh {
    pub fn from_obj(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        // Use baby_shark or meshx
        let mesh = TriMesh::read(path)?;

        let nodes = mesh.vertices()
            .iter()
            .enumerate()
            .map(|(i, v)| Node {
                id: i + 1,  // 1-indexed for compatibility
                x: v[0],
                y: v[1],
                z: v[2],
            })
            .collect();

        let elements = mesh.triangles()
            .iter()
            .enumerate()
            .map(|(i, tri)| Element {
                id: i + 1,
                node_ids: [tri[0] + 1, tri[1] + 1, tri[2] + 1],
                material_id: 1,  // Default, can assign later
            })
            .collect();

        Ok(HeadMesh { nodes, elements })
    }

    pub fn export_nodes(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;

        for node in &self.nodes {
            writeln!(file, "{} {} {} {}",
                node.id, node.x, node.y, node.z)?;
        }

        Ok(())
    }

    pub fn export_elements(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;

        for elem in &self.elements {
            writeln!(file, "{} {} {} {}",
                elem.id, elem.node_ids[0], elem.node_ids[1], elem.node_ids[2])?;
        }

        Ok(())
    }
}
```

#### 2.2 Material Assignment

```rust
// src-mesh2hrtf/src/mesh2input/materials.rs

#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub admittance_curve: Vec<(f64, f64)>,  // (frequency, admittance)
}

#[derive(Debug)]
pub struct MaterialDatabase {
    materials: Vec<Material>,
}

impl MaterialDatabase {
    pub fn load_from_csv(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        // Parse CSV format used by Mesh2HRTF
        // Format: frequency, admittance_real, admittance_imag

        use csv::ReaderBuilder;
        let mut rdr = ReaderBuilder::new()
            .has_headers(false)
            .from_path(path)?;

        let mut curve = Vec::new();
        for result in rdr.records() {
            let record = result?;
            let freq: f64 = record[0].parse()?;
            let admittance: f64 = record[1].parse()?;
            curve.push((freq, admittance));
        }

        Ok(MaterialDatabase {
            materials: vec![Material {
                name: "default".to_string(),
                admittance_curve: curve,
            }],
        })
    }
}

impl HeadMesh {
    pub fn assign_material_by_region(
        &mut self,
        region_selector: impl Fn(&Element) -> usize,
    ) {
        for elem in &mut self.elements {
            elem.material_id = region_selector(elem);
        }
    }
}
```

#### 2.3 Evaluation Grids

```rust
// src-mesh2hrtf/src/mesh2input/grids.rs

#[derive(Debug)]
pub struct EvaluationGrid {
    pub nodes: Vec<Node>,
    pub name: String,
}

impl EvaluationGrid {
    pub fn spherical_grid(
        center: [f64; 3],
        radius: f64,
        num_azimuth: usize,
        num_elevation: usize,
    ) -> Self {
        let mut nodes = Vec::new();
        let mut id = 1;

        for elev_idx in 0..num_elevation {
            let elevation = -90.0 + (180.0 * elev_idx as f64 / (num_elevation - 1) as f64);
            let elev_rad = elevation.to_radians();

            for az_idx in 0..num_azimuth {
                let azimuth = 360.0 * az_idx as f64 / num_azimuth as f64;
                let az_rad = azimuth.to_radians();

                let x = center[0] + radius * elev_rad.cos() * az_rad.cos();
                let y = center[1] + radius * elev_rad.cos() * az_rad.sin();
                let z = center[2] + radius * elev_rad.sin();

                nodes.push(Node { id, x, y, z });
                id += 1;
            }
        }

        EvaluationGrid {
            nodes,
            name: "SphericalGrid".to_string(),
        }
    }

    pub fn export(&self, dir: impl AsRef<Path>) -> anyhow::Result<()> {
        let nodes_path = dir.as_ref().join("Nodes.txt");

        use std::io::Write;
        let mut file = std::fs::File::create(nodes_path)?;

        for node in &self.nodes {
            writeln!(file, "{} {} {} {}", node.id, node.x, node.y, node.z)?;
        }

        Ok(())
    }
}
```

#### 2.4 NC.inp Generator

```rust
// src-mesh2hrtf/src/mesh2input/nc_input.rs

#[derive(Debug)]
pub struct NumCalcInput {
    pub title: String,
    pub bem_method: BemMethod,
    pub frequencies: Vec<f64>,
    pub source_type: SourceType,
    pub materials: Vec<Material>,
    pub speed_of_sound: f64,
    pub density: f64,
}

#[derive(Debug)]
pub enum BemMethod {
    BurtonMiller,
    Kirchhoff,
}

#[derive(Debug)]
pub enum SourceType {
    PlaneWave { direction: [f64; 3] },
    PointSource { position: [f64; 3] },
}

impl NumCalcInput {
    pub fn write(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;

        // NC.inp format (reverse-engineered from examples)
        writeln!(file, "Title: {}", self.title)?;
        writeln!(file, "BEM Method: {:?}", self.bem_method)?;
        writeln!(file, "Speed of sound: {}", self.speed_of_sound)?;
        writeln!(file, "Density: {}", self.density)?;
        writeln!(file, "Number of frequencies: {}", self.frequencies.len())?;

        for freq in &self.frequencies {
            writeln!(file, "{}", freq)?;
        }

        // TODO: Complete format based on actual NC.inp structure
        // This will require studying existing NC.inp files

        Ok(())
    }
}
```

#### 2.5 Complete Project Export

```rust
// src-mesh2hrtf/src/mesh2input/project.rs

pub struct Mesh2HrtfProject {
    pub mesh: HeadMesh,
    pub materials: MaterialDatabase,
    pub evaluation_grids: Vec<EvaluationGrid>,
    pub nc_input: NumCalcInput,
}

impl Mesh2HrtfProject {
    pub fn export(&self, project_dir: impl AsRef<Path>) -> anyhow::Result<()> {
        let dir = project_dir.as_ref();

        // Create directory structure
        std::fs::create_dir_all(dir)?;
        std::fs::create_dir_all(dir.join("ObjectMeshes"))?;
        std::fs::create_dir_all(dir.join("EvaluationGrids"))?;
        std::fs::create_dir_all(dir.join("NumCalc"))?;

        // Export mesh
        let mesh_dir = dir.join("ObjectMeshes/Reference");
        std::fs::create_dir_all(&mesh_dir)?;
        self.mesh.export_nodes(mesh_dir.join("Nodes.txt"))?;
        self.mesh.export_elements(mesh_dir.join("Elements.txt"))?;

        // Export evaluation grids
        for grid in &self.evaluation_grids {
            let grid_dir = dir.join(format!("EvaluationGrids/{}", grid.name));
            std::fs::create_dir_all(&grid_dir)?;
            grid.export(&grid_dir)?;
        }

        // Export NC.inp
        self.nc_input.write(dir.join("NumCalc/NC.inp"))?;

        // Export parameters.json
        let params = serde_json::json!({
            "title": self.nc_input.title,
            "frequencies": self.nc_input.frequencies,
            "created_with": "mesh2hrtf-rust",
            "version": env!("CARGO_PKG_VERSION"),
        });
        std::fs::write(
            dir.join("parameters.json"),
            serde_json::to_string_pretty(&params)?
        )?;

        Ok(())
    }
}
```

---

## Phase 3: Output2HRTF in Rust (3-4 weeks)

### Implementation

#### 3.1 SOFA File Format

```rust
// src-mesh2hrtf/src/output2hrtf/sofa.rs

use hdf5::{File, Group};
use ndarray::{Array1, Array2, Array3};

#[derive(Debug)]
pub struct SofaFile {
    pub sample_rate: f64,
    pub data_ir: Array3<f64>,  // [M x R x N] = [measurements x receivers x samples]
    pub source_position: Array2<f64>,  // [M x C] = [measurements x coordinates]
    pub receiver_position: Array2<f64>,  // [R x C]
    pub listener_position: Array2<f64>,
}

impl SofaFile {
    pub fn write(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let file = File::create(path)?;

        // SOFA required attributes
        file.new_attr::<hdf5::types::VarLenUnicode>()
            .create("Conventions")?
            .write_scalar(&"SOFA".into())?;

        file.new_attr::<hdf5::types::VarLenUnicode>()
            .create("SOFAConventions")?
            .write_scalar(&"SimpleFreeFieldHRIR".into())?;

        file.new_attr::<hdf5::types::VarLenUnicode>()
            .create("DataType")?
            .write_scalar(&"FIR".into())?;

        // Write impulse responses
        let data_group = file.create_group("Data.IR")?;
        data_group.new_dataset::<f64>()
            .shape(self.data_ir.shape())
            .create("values")?
            .write(&self.data_ir)?;

        // Write source positions
        let src_group = file.create_group("SourcePosition")?;
        src_group.new_dataset::<f64>()
            .shape(self.source_position.shape())
            .create("values")?
            .write(&self.source_position)?;

        // Sample rate
        file.new_attr::<f64>()
            .create("Data.SamplingRate")?
            .write_scalar(&self.sample_rate)?;

        Ok(())
    }

    pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file = File::open(path)?;

        let data_ir: Array3<f64> = file.dataset("Data.IR")?.read()?;
        let source_position: Array2<f64> = file.dataset("SourcePosition")?.read()?;
        let receiver_position: Array2<f64> = file.dataset("ReceiverPosition")?.read()?;
        let listener_position: Array2<f64> = file.dataset("ListenerPosition")?.read()?;

        let sample_rate = file.attr("Data.SamplingRate")?.read_scalar::<f64>()?;

        Ok(SofaFile {
            sample_rate,
            data_ir,
            source_position,
            receiver_position,
            listener_position,
        })
    }
}
```

#### 3.2 DTF Computation

```rust
// src-mesh2hrtf/src/output2hrtf/dtf.rs

use ndarray::{Array1, Array2};
use num_complex::Complex64;

/// Compute Directional Transfer Function from NumCalc output
pub fn compute_dtf(
    pressure_field: &Array2<Complex64>,  // [frequencies x positions]
    reference_position: usize,
) -> Array2<Complex64> {
    let num_freqs = pressure_field.nrows();
    let num_positions = pressure_field.ncols();

    let mut dtf = Array2::<Complex64>::zeros((num_freqs, num_positions));

    // DTF = pressure / pressure_reference
    for freq_idx in 0..num_freqs {
        let p_ref = pressure_field[[freq_idx, reference_position]];

        for pos_idx in 0..num_positions {
            let p = pressure_field[[freq_idx, pos_idx]];
            dtf[[freq_idx, pos_idx]] = p / p_ref;
        }
    }

    dtf
}
```

#### 3.3 HRIR Computation (IFFT)

```rust
// src-mesh2hrtf/src/output2hrtf/hrir.rs

use rustfft::{FftPlanner, num_complex::Complex64};
use ndarray::{Array1, Array2};

pub fn compute_hrir(
    dtf: &Array2<Complex64>,  // [frequencies x positions]
    sample_rate: f64,
) -> Array2<f64> {
    let num_freqs = dtf.nrows();
    let num_positions = dtf.ncols();

    // Prepare inverse FFT
    let fft_size = (num_freqs - 1) * 2;  // Assuming single-sided spectrum
    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(fft_size);

    let mut hrir = Array2::<f64>::zeros((fft_size, num_positions));

    for pos_idx in 0..num_positions {
        // Extract frequency data for this position
        let mut freq_data = vec![Complex64::new(0.0, 0.0); fft_size];

        for freq_idx in 0..num_freqs {
            freq_data[freq_idx] = dtf[[freq_idx, pos_idx]];
        }

        // Mirror for negative frequencies (Hermitian symmetry)
        for i in 1..num_freqs {
            freq_data[fft_size - i] = freq_data[i].conj();
        }

        // Perform IFFT
        ifft.process(&mut freq_data);

        // Extract real part (time domain)
        for (i, val) in freq_data.iter().enumerate() {
            hrir[[i, pos_idx]] = val.re / fft_size as f64;
        }
    }

    hrir
}
```

#### 3.4 NumCalc Output Parser

```rust
// src-mesh2hrtf/src/output2hrtf/parser.rs

use ndarray::{Array2, Array1};
use num_complex::Complex64;
use std::path::Path;

pub struct NumCalcOutput {
    pub frequencies: Vec<f64>,
    pub pressure_field: Array2<Complex64>,  // [freqs x positions]
}

impl NumCalcOutput {
    pub fn read_from_directory(be_out_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let dir = be_out_dir.as_ref();

        // NumCalc outputs files like:
        // be.out/be_X_Y.txt where X is frequency index, Y is evaluation point

        let mut frequencies = Vec::new();
        let mut pressure_data = Vec::new();

        // Read all output files
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("txt") {
                let (freq, pressures) = Self::parse_output_file(&path)?;
                frequencies.push(freq);
                pressure_data.push(pressures);
            }
        }

        // Sort by frequency
        let mut indexed: Vec<_> = frequencies.iter()
            .zip(pressure_data.iter())
            .enumerate()
            .collect();
        indexed.sort_by(|a, b| a.1.0.partial_cmp(b.1.0).unwrap());

        let frequencies: Vec<f64> = indexed.iter().map(|(_, (f, _))| **f).collect();

        // Build 2D array
        let num_freqs = frequencies.len();
        let num_positions = pressure_data[0].len();
        let mut pressure_field = Array2::<Complex64>::zeros((num_freqs, num_positions));

        for (new_idx, (old_idx, _)) in indexed.iter().enumerate() {
            for pos_idx in 0..num_positions {
                pressure_field[[new_idx, pos_idx]] = pressure_data[*old_idx][pos_idx];
            }
        }

        Ok(NumCalcOutput {
            frequencies,
            pressure_field,
        })
    }

    fn parse_output_file(path: &Path) -> anyhow::Result<(f64, Vec<Complex64>)> {
        // Parse NumCalc output format
        // TODO: Implement based on actual format
        // Likely columns: position_id, real, imag, magnitude, phase

        use std::io::BufRead;
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);

        let mut frequency = 0.0;
        let mut pressures = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.starts_with("#") || line.trim().is_empty() {
                continue;
            }

            // Parse line (example format, adjust to actual)
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let real: f64 = parts[1].parse()?;
                let imag: f64 = parts[2].parse()?;
                pressures.push(Complex64::new(real, imag));
            }
        }

        Ok((frequency, pressures))
    }
}
```

#### 3.5 Complete Pipeline

```rust
// src-mesh2hrtf/src/output2hrtf/mod.rs

pub fn numcalc_to_sofa(
    numcalc_output_dir: impl AsRef<Path>,
    sofa_output_path: impl AsRef<Path>,
    sample_rate: f64,
) -> anyhow::Result<()> {
    // 1. Read NumCalc output
    let nc_output = NumCalcOutput::read_from_directory(
        numcalc_output_dir.as_ref().join("be.out")
    )?;

    // 2. Compute DTF (directional transfer function)
    let reference_position = 0;  // Typically center of head
    let dtf = compute_dtf(&nc_output.pressure_field, reference_position);

    // 3. Compute HRIR (impulse response via IFFT)
    let hrir = compute_hrir(&dtf, sample_rate);

    // 4. Format as SOFA
    let sofa = SofaFile {
        sample_rate,
        data_ir: hrir.insert_axis(ndarray::Axis(1)),  // Add receiver dimension
        source_position: Array2::zeros((hrir.ncols(), 3)),  // TODO: Get from project
        receiver_position: Array2::zeros((2, 3)),  // Left/right ear
        listener_position: Array2::zeros((1, 3)),
    };

    // 5. Write SOFA file
    sofa.write(sofa_output_path)?;

    Ok(())
}
```

---

## Phase 4: CLI Integration (1 week)

### Unified Binary

```rust
// src-mesh2hrtf/src/main.rs

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mesh2hrtf")]
#[command(about = "Generate HRTFs from 3D head meshes", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Prepare mesh and create project
    Input {
        /// Input mesh file (OBJ, STL, etc.)
        #[arg(short, long)]
        mesh: PathBuf,

        /// Output project directory
        #[arg(short, long)]
        output: PathBuf,

        /// Frequency range (Hz)
        #[arg(long, value_delimiter = ',')]
        frequencies: Vec<f64>,
    },

    /// Run NumCalc simulation
    Compute {
        /// Project directory
        #[arg(short, long)]
        project: PathBuf,

        /// Maximum concurrent instances
        #[arg(long, default_value = "4")]
        jobs: usize,
    },

    /// Convert NumCalc output to SOFA
    Output {
        /// Project directory
        #[arg(short, long)]
        project: PathBuf,

        /// Output SOFA file
        #[arg(short, long)]
        output: PathBuf,

        /// Sample rate (Hz)
        #[arg(long, default_value = "44100")]
        sample_rate: f64,
    },

    /// Run complete pipeline
    Full {
        /// Input mesh file
        #[arg(short, long)]
        mesh: PathBuf,

        /// Output SOFA file
        #[arg(short, long)]
        output: PathBuf,

        /// Frequencies (Hz)
        #[arg(long, value_delimiter = ',')]
        frequencies: Vec<f64>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Input { mesh, output, frequencies } => {
            // Load mesh
            let mesh = HeadMesh::from_obj(&mesh)?;

            // Create project
            let project = Mesh2HrtfProject {
                mesh,
                materials: MaterialDatabase::default(),
                evaluation_grids: vec![
                    EvaluationGrid::spherical_grid([0.0, 0.0, 0.0], 1.0, 72, 37)
                ],
                nc_input: NumCalcInput {
                    title: "Generated by mesh2hrtf-rust".to_string(),
                    bem_method: BemMethod::BurtonMiller,
                    frequencies,
                    source_type: SourceType::PlaneWave {
                        direction: [1.0, 0.0, 0.0],
                    },
                    speed_of_sound: 343.0,
                    density: 1.2,
                },
            };

            project.export(&output)?;
            println!("Project created at: {}", output.display());
        }

        Commands::Compute { project, jobs } => {
            let runner = ParallelNumCalc::new(&project)?;

            // Read number of frequencies from NC.inp
            // TODO: Parse NC.inp to get frequency count
            let num_freqs = 100;

            println!("Running NumCalc with {} parallel jobs", jobs);
            let results = tokio::runtime::Runtime::new()?
                .block_on(runner.run_all_frequencies(num_freqs))?;

            println!("Completed {} frequency steps", results.len());
        }

        Commands::Output { project, output, sample_rate } => {
            numcalc_to_sofa(&project, &output, sample_rate)?;
            println!("SOFA file written to: {}", output.display());
        }

        Commands::Full { mesh, output, frequencies } => {
            // Create temp directory
            let temp_dir = tempfile::tempdir()?;

            // Phase 1: Input
            println!("[1/3] Preparing mesh...");
            // ... (same as Input command)

            // Phase 2: Compute
            println!("[2/3] Running NumCalc...");
            // ... (same as Compute command)

            // Phase 3: Output
            println!("[3/3] Generating SOFA file...");
            // ... (same as Output command)

            println!("Complete! HRTF saved to: {}", output.display());
        }
    }

    Ok(())
}
```

---

## Timeline & Milestones

| Week | Phase | Deliverable |
|------|-------|-------------|
| 1-2 | NumCalc wrapper | Rust can call C++ NumCalc, run single frequency |
| 2-3 | Parallel execution | Run all frequencies with resource management |
| 3-4 | Mesh2Input (mesh) | Load OBJ/STL, export Nodes.txt/Elements.txt |
| 4-5 | Mesh2Input (NC.inp) | Generate complete NC.inp files |
| 5-6 | Output2HRTF (parser) | Read NumCalc output |
| 6-7 | Output2HRTF (DSP) | DTF computation, IFFT to HRIR |
| 7-8 | Output2HRTF (SOFA) | Write SOFA files |
| 8 | CLI integration | Unified `mesh2hrtf` command |
| 9-10 | Testing & validation | Compare with Python reference |
| 10-12 | SOTF integration | HRTF plugin for binaural playback |

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_load() {
        let mesh = HeadMesh::from_obj("tests/fixtures/head.obj").unwrap();
        assert!(mesh.nodes.len() > 0);
        assert!(mesh.elements.len() > 0);
    }

    #[test]
    fn test_dtf_computation() {
        // Use reference data from tests/references/
        let reference_pressure = load_reference_pressure();
        let dtf = compute_dtf(&reference_pressure, 0);

        let expected_dtf = load_reference_dtf();
        assert_approx_eq(&dtf, &expected_dtf, 1e-6);
    }

    #[test]
    fn test_sofa_roundtrip() {
        let sofa = create_test_sofa();
        sofa.write("/tmp/test.sofa").unwrap();

        let loaded = SofaFile::read("/tmp/test.sofa").unwrap();
        assert_eq!(sofa.sample_rate, loaded.sample_rate);
    }
}
```

### Integration Tests

Use existing Mesh2HRTF test data from `tests/references/`:

```bash
# Run against reference data
cargo run --bin mesh2hrtf -- full \
    --mesh tests/references/KU100.obj \
    --output /tmp/ku100.sofa \
    --frequencies 100,200,500,1000,2000,5000,10000

# Compare with Python-generated SOFA
python tests/compare_sofa.py \
    tests/references/KU100_reference.sofa \
    /tmp/ku100.sofa
```

---

## Dependencies

```toml
[dependencies]
# Mesh processing
meshx = "0.3"
# or: baby_shark = "0.1"

# Linear algebra
ndarray = "0.15"
num-complex = "0.4"

# FFT
rustfft = "6.0"  # Already in SOTF

# HDF5 (for SOFA files)
hdf5 = "0.8"

# CLI
clap = { version = "4.0", features = ["derive"] }

# Async runtime (for parallel NumCalc)
tokio = { version = "1", features = ["full"] }

# Utilities
anyhow = "1.0"
thiserror = "1.0"
log = "0.4"
env_logger = "0.11"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
csv = "1.3"

# System info (for RAM estimation)
sysinfo = "0.30"
num_cpus = "1.0"

# Process management
which = "6.0"  # Find NumCalc executable

[build-dependencies]
cc = "1.0"  # Compile C++ code
# or: cmake = "0.1"
```

---

## Next Steps

1. **Create workspace structure**:
   ```bash
   mkdir src-mesh2hrtf
   cd src-mesh2hrtf
   cargo init --lib
   ```

2. **Copy NumCalc C++ source**:
   ```bash
   git clone https://github.com/Any2HRTF/Mesh2HRTF.git /tmp/mesh2hrtf
   cp -r /tmp/mesh2hrtf/mesh2hrtf/NumCalc src-mesh2hrtf/
   ```

3. **Start with Phase 1** (NumCalc wrapper):
   - Build NumCalc using build.rs
   - Create subprocess wrapper
   - Test with example project

4. **Validate early**:
   - Use existing Mesh2HRTF test cases
   - Compare outputs bit-for-bit
   - Ensure NC.inp compatibility

---

## Advantages of This Approach

✅ **Fast time-to-market**: 8-12 weeks vs. 24+ months for full rewrite
✅ **Low risk**: Reuse proven BEM solver
✅ **Pure Rust I/O**: Mesh processing and SOFA generation in Rust
✅ **Cross-platform**: Build NumCalc on all platforms
✅ **Testable**: Validate against existing reference data
✅ **Incremental**: Can replace NumCalc later if needed

---

## Future Enhancements

**After initial release**:

1. **Pure Rust BEM** (optional, long-term):
   - Implement basic BEM in Rust
   - Compare performance with C++ NumCalc
   - Gradually replace if beneficial

2. **SOTF Integration**:
   - HRTF convolution plugin
   - Head tracking support
   - Real-time binaural rendering

3. **Web Assembly**:
   - Compile to WASM for browser-based HRTF generation
   - Interactive mesh editor in browser

4. **GPU Acceleration**:
   - Use `wgpu` for matrix operations
   - Accelerate BEM solve phase

---

**Ready to start implementation? Let's begin with Phase 1: NumCalc FFI wrapper.**
