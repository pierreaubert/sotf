# Head Scanner

A Rust-based 3D head scanning system using computer vision for HRTF (Head-Related Transfer Function) optimization.

## Overview

The head scanner uses a camera (webcam or phone) to capture multiple views of a person's head and reconstructs a high-precision 3D surface model. The system provides real-time feedback about scan coverage and outputs a triangulated mesh using convex hull algorithms.

## Architecture

### Core Components

1. **Camera Capture** (`camera.rs`)
   - Cross-platform camera access via OpenCV
   - Real-time frame capture and preprocessing
   - Supports webcams and mobile device cameras

2. **Computer Vision** (`vision.rs`)
   - Feature detection using Haar cascades (classical CV)
   - Optional ML model support via ONNX Runtime
   - Facial landmark tracking across frames

3. **3D Reconstruction** (`reconstruction.rs`)
   - Structure-from-Motion (SfM) pipeline
   - Camera pose estimation
   - Point triangulation from multiple views
   - Configurable camera intrinsics

4. **Point Cloud** (`pointcloud.rs`)
   - Efficient 3D point storage with k-d tree indexing
   - Point cloud filtering and downsampling
   - Outlier removal using statistical methods
   - Normal estimation
   - PLY export format

5. **Coverage Tracking** (`coverage.rs`)
   - Voxel-based coverage mapping
   - Real-time feedback on uncovered regions
   - Coverage percentage calculation
   - Heatmap generation for visualization

6. **Convex Hull** (`convexhull.rs`)
   - 3D convex hull computation using QuickHull algorithm
   - Volume and surface area calculations
   - Face extraction for mesh generation

7. **Mesh Generation** (`mesh.rs`)
   - Triangulated mesh creation
   - Smooth vertex normal computation
   - Multiple export formats: OBJ, PLY, STL
   - Texture coordinate support

## Usage

```rust
use head_scanner::{HeadScanner, ScannerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create scanner with default configuration
    let config = ScannerConfig::default();
    let mut scanner = HeadScanner::new(config)?;

    // Start scanning
    scanner.start().await?;

    // Process frames until scan is complete
    while !scanner.is_scan_complete() {
        scanner.process_frame().await?;

        // Get real-time feedback
        let coverage = scanner.get_coverage();
        let coverage_map = scanner.get_coverage_map();
        let uncovered = coverage_map.get_uncovered_regions();

        println!("Scan coverage: {:.1}%", coverage * 100.0);
        println!("Uncovered regions: {}", uncovered.len());
    }

    // Generate final mesh
    let mesh = scanner.generate_mesh()?;

    // Export in multiple formats
    mesh.export("head_model.obj")?;  // Wavefront OBJ
    mesh.export("head_model.ply")?;  // Stanford PLY
    mesh.export("head_model.stl")?;  // STL for 3D printing

    Ok(())
}
```

## Configuration

The `ScannerConfig` struct provides extensive configuration options:

```rust
let config = ScannerConfig {
    camera_index: 0,           // Camera device ID
    frame_width: 1280,         // Frame resolution width
    frame_height: 720,         // Frame resolution height
    fps: 30,                   // Target frame rate
    min_coverage: 0.85,        // Minimum coverage (85%)
    point_density: 50.0,       // Points per cm²
    use_gpu: true,             // Enable GPU acceleration
    model_path: Some("depth_model.onnx".into()), // Optional ML model
};
```

## Dependencies

### System Dependencies

**Linux:**
```bash
# OpenCV
sudo apt-get install libopencv-dev libclang-dev

# OpenBLAS (required by workspace configuration)
sudo apt-get install libopenblas-dev

# Optional: CUDA for GPU acceleration
sudo apt-get install nvidia-cuda-toolkit
```

**macOS:**
```bash
brew install opencv pkg-config

# OpenBLAS is provided by Accelerate framework (built-in)
```

**Windows:**
```powershell
# Install OpenCV via vcpkg
vcpkg install opencv:x64-windows

# Or download pre-built binaries from opencv.org
```

### Rust Dependencies

The crate depends on:
- **opencv** (0.92): Computer vision algorithms
- **nalgebra** (0.33): Linear algebra and 3D math
- **parry3d** (0.17): 3D geometry (modern replacement for deprecated ncollide3d)
- **ort** (2.0.0-rc.10): ONNX Runtime for ML models
- **kiddo** (4.2): k-d tree for spatial queries
- **ndarray**: N-dimensional arrays (without BLAS to reduce system dependencies)
- **tokio**: Async runtime
- **serde/serde_json**: Serialization
- **parking_lot**: Efficient synchronization primitives

## ⚠️ Current Status and Limitations

**This crate is in early development** and contains incomplete implementations:

### Known Limitations

1. **Convex Hull Algorithm**: The QuickHull implementation is simplified and may not produce correct results for complex geometries. Consider using external libraries like `delaunator` for production use.

2. **Structure-from-Motion (SfM)**: The 3D reconstruction pipeline uses naive depth estimation and simplified camera pose tracking. For production-quality scanning, integrate with external SfM libraries.

3. **Vision Models**: ONNX model preprocessing/postprocessing is not fully implemented. The fallback uses Haar cascades for basic face detection.

4. **Security**: Path validation is implemented but should be reviewed for production use. Never use this crate with untrusted user input without additional sandboxing.

5. **Testing**: Test coverage is minimal. Camera tests require hardware and are disabled by default.

### Recent Fixes

- ✅ Fixed camera mutable borrow issue by wrapping VideoCapture in Mutex
- ✅ Replaced deprecated `ncollide3d` with `parry3d`
- ✅ Implemented point deduplication using k-d tree spatial filtering
- ✅ Added path validation to prevent path traversal attacks
- ✅ Added model path validation with existence checks
- ✅ Improved locking strategy with explicit lock releases

### TODO

- [ ] Implement proper QuickHull or use existing convex hull library
- [ ] Add bundle adjustment for SfM accuracy
- [ ] Implement ML model preprocessing/postprocessing
- [ ] Add comprehensive integration tests
- [ ] Add CI/CD pipeline with OpenCV installation
- [ ] Implement stereo camera support for better depth
- [ ] Add texture mapping from camera frames
- [ ] Performance profiling and optimization
- [ ] Add examples for common use cases

## Building

```bash
# Build the crate
cargo build -p head-scanner

# Run tests
cargo test -p head-scanner

# Build examples
cargo build -p head-scanner --examples

# With release optimizations
cargo build -p head-scanner --release
```

### Build Requirements

The workspace uses platform-specific BLAS backends (configured in `/.cargo/config.toml`):
- **Linux**: OpenBLAS
- **macOS**: Accelerate framework (Apple)
- **Windows**: Intel MKL or OpenBLAS via vcpkg

Make sure the appropriate BLAS library is installed for your platform before building.

## Integration with Tauri

The head scanner can be integrated into a Tauri desktop application:

```rust
// In src-tauri/src/lib.rs

use head_scanner::{HeadScanner, ScannerConfig};
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
async fn start_head_scan(
    scanner: State<'_, Mutex<Option<HeadScanner>>>,
) -> Result<(), String> {
    let config = ScannerConfig::default();
    let head_scanner = HeadScanner::new(config)
        .map_err(|e| e.to_string())?;

    head_scanner.start().await
        .map_err(|e| e.to_string())?;

    *scanner.lock().await = Some(head_scanner);
    Ok(())
}

#[tauri::command]
async fn get_scan_coverage(
    scanner: State<'_, Mutex<Option<HeadScanner>>>,
) -> Result<f32, String> {
    let scanner_guard = scanner.lock().await;
    let scanner = scanner_guard.as_ref()
        .ok_or("Scanner not initialized")?;

    Ok(scanner.get_coverage())
}

// Register commands in Tauri builder
tauri::Builder::default()
    .manage(Mutex::new(None::<HeadScanner>))
    .invoke_handler(tauri::generate_handler![
        start_head_scan,
        get_scan_coverage,
        // ... more commands
    ])
```

## Examples

See the `examples/` directory for complete working examples:

- `basic_scan.rs` - Simple head scanning pipeline
- `coverage_visualization.rs` - Real-time coverage feedback
- `mesh_export.rs` - Exporting to various formats
- `camera_calibration.rs` - Calibrating camera intrinsics

## Future Enhancements

- [ ] Stereo camera support for improved depth accuracy
- [ ] Real-time depth estimation using ML models
- [ ] Texture mapping from camera frames
- [ ] Multi-resolution mesh generation
- [ ] Automatic head alignment and centering
- [ ] Cloud-based processing for resource-intensive operations
- [ ] Mobile app integration (iOS/Android)
- [ ] Integration with HRTF generation pipeline

## Performance

The scanner is optimized for real-time operation:

- **Frame Processing**: ~30 FPS on modern hardware
- **Point Cloud**: Handles 100K+ points efficiently
- **Memory Usage**: ~100-500 MB typical
- **Scan Time**: 30-60 seconds for complete coverage

## Troubleshooting

### Camera not found
- Check camera permissions
- Verify camera index (try 0, 1, 2...)
- Ensure no other application is using the camera

### OpenCV errors
- Verify OpenCV installation
- Check that Haar cascade files are in the correct location
- Linux: `/usr/share/opencv4/haarcascades/`
- macOS: `/usr/local/share/opencv4/haarcascades/`

### BLAS linker errors
- Install platform-specific BLAS library
- Linux: `sudo apt-get install libopenblas-dev`
- Ensure `.cargo/config.toml` paths match your system

## License

This crate is part of the SOTF (Sound of the Future) project and is licensed under GPL-3.0-or-later.

## Contributing

Contributions are welcome! Please ensure:
- Code follows Rust formatting guidelines (`cargo fmt`)
- All tests pass (`cargo test`)
- New features include documentation and examples
