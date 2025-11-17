# Head Scanner

A comprehensive 3D head scanning system using computer vision for HRTF (Head-Related Transfer Function) optimization.

## Recent Enhancements

This crate has been significantly enhanced with the following improvements:

### 1. **Convex Hull Integration**
- Replaced homemade convex hull implementation with the robust `src-convexhull3d` crate
- Provides better accuracy and performance for 3D mesh generation
- Full integration with existing point cloud infrastructure

### 2. **Bundle Adjustment for SfM**
- Added `bundle_adjustment` module implementing Levenberg-Marquardt optimization
- Simultaneously refines camera poses and 3D point positions
- Minimizes reprojection error across all views
- Configurable iteration count and convergence thresholds

### 3. **ML Model Support**
- **Preprocessing**: Image normalization, resizing, and NCHW format conversion for ONNX models
- **Postprocessing**: Support for multiple output formats (detection boxes, keypoints)
- **Non-Maximum Suppression**: Removes overlapping detections
- Fully integrated with ONNX Runtime for neural network inference

### 4. **Stereo Camera Support**
- Complete stereo vision pipeline for accurate depth estimation
- Stereo matching with epipolar constraints
- Triangulation from stereo correspondences
- Dense depth map computation
- Calibration framework (ready for implementation)

### 5. **Texture Mapping**
- Project camera images onto 3D meshes
- Spherical UV mapping for head geometry
- Multi-view texture blending
- Best-camera selection per triangle based on viewing angle
- Export textured meshes with OBJ/MTL format

### 6. **Comprehensive Integration Tests**
- Full workflow testing from point cloud to mesh export
- SfM reconstruction pipeline tests
- Bundle adjustment verification
- Stereo depth estimation tests
- Texture mapping validation
- Coverage tracking and mesh export tests

### 7. **Performance Profiling & Benchmarking**
- Dedicated benchmark suite (`benches/performance.rs`)
- Benchmarks for:
  - Convex hull computation (100 to 10,000 points)
  - SfM reconstruction (10 to 100 frames)
  - Bundle adjustment (10 to 500 points)
  - Point cloud operations (1,000 to 50,000 points)
  - Texture mapping initialization
- Run with: `cargo bench --package head-scanner`

### 8. **Examples for Common Use Cases**
- **simple_scan.rs**: Basic scanning workflow with synthetic data
- **sfm_reconstruction.rs**: Structure-from-Motion example
- **stereo_depth.rs**: Stereo depth estimation demo
- **textured_mesh.rs**: Texture mapping example

## Architecture

The head scanner consists of several key modules:

- **bundle_adjustment**: Bundle adjustment optimizer for SfM refinement
- **camera**: Camera capture and frame management
- **convexhull**: 3D convex hull computation (uses `src-convexhull3d`)
- **coverage**: Scan coverage tracking
- **mesh**: 3D mesh generation and export
- **pointcloud**: Point cloud data structure and operations
- **reconstruction**: Structure-from-Motion and 3D reconstruction
- **stereo**: Stereo camera support and depth estimation
- **texture**: Texture mapping from camera frames
- **vision**: Computer vision features (classical and ML-based)

## Usage

### Basic Scanning

```rust
use head_scanner::*;

// Create scanner configuration
let config = ScannerConfig::default();
let mut scanner = HeadScanner::new(config)?;

// Start scanning
scanner.start().await?;

// Process frames
while !scanner.is_scan_complete() {
    scanner.process_frame().await?;
    let coverage = scanner.get_coverage();
    println!("Coverage: {:.1}%", coverage * 100.0);
}

// Generate mesh
let mesh = scanner.generate_mesh()?;
mesh.export("head_model.obj")?;
```

### Structure-from-Motion with Bundle Adjustment

```rust
use head_scanner::*;
use reconstruction::{CameraIntrinsics, SfMReconstructor};
use bundle_adjustment::BundleAdjuster;

// Initialize SfM
let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
let mut sfm = SfMReconstructor::new(intrinsics.clone());

// Add frames
for features in frame_features {
    sfm.add_frame(features)?;
}

// Refine with bundle adjustment
let adjuster = BundleAdjuster::new(intrinsics);
let (refined_poses, refined_points) = adjuster.optimize(&poses, &points)?;
```

### Stereo Depth Estimation

```rust
use head_scanner::stereo::{StereoConfig, StereoDepthEstimator};

// Configure stereo system
let config = StereoConfig::default_webcam_stereo(1280, 720, 6.0);
let estimator = StereoDepthEstimator::new(config);

// Compute depth map
let depth_map = estimator.compute_depth_map(&left_frame, &right_frame)?;

// Or triangulate specific points
let points_3d = estimator.triangulate_points(&left_features, &right_features)?;
```

### Texture Mapping

```rust
use head_scanner::texture::TextureMapper;

// Create texture mapper
let mapper = TextureMapper::new(1024, 1024);

// Apply texture from multiple views
let textured_mesh = mapper.apply_multi_frame(&mesh, &frames_with_poses)?;

// Export with textures
textured_mesh.export_obj("model.obj", "material.mtl", "texture.png")?;
```

## Running Examples

```bash
# Simple scanning example
cargo run --example simple_scan

# Structure-from-Motion
cargo run --example sfm_reconstruction

# Stereo depth estimation
cargo run --example stereo_depth

# Textured mesh generation
cargo run --example textured_mesh
```

## Running Benchmarks

```bash
cargo bench --package head-scanner
```

## Running Tests

```bash
# All tests
cargo test --package head-scanner

# Integration tests only
cargo test --package head-scanner --test integration_tests

# Unit tests only
cargo test --package head-scanner --lib
```

## Dependencies

### Required System Libraries
- **OpenCV**: Computer vision operations
- **ONNX Runtime**: ML model inference

### Key Rust Dependencies
- `convexhull3d`: Robust 3D convex hull computation
- `nalgebra`: Linear algebra
- `opencv`: Computer vision
- `ort`: ONNX Runtime bindings
- `image`: Image processing
- `ndarray`: N-dimensional arrays (without BLAS to avoid system dependencies)

## Development

### Adding New Features

1. Implement new module in `src/`
2. Add module to `lib.rs`
3. Write unit tests in the module
4. Add integration tests to `tests/integration_tests.rs`
5. Create example in `examples/` if appropriate
6. Update this README

### Performance Optimization

Use the benchmark suite to identify bottlenecks:

```bash
cargo bench --package head-scanner
```

Profile with:

```bash
cargo build --release --package head-scanner
perf record --call-graph dwarf ./target/release/examples/simple_scan
perf report
```

## Future Enhancements

- [ ] GPU acceleration for stereo matching
- [ ] Real-time ML model inference on GPU
- [ ] Advanced mesh smoothing algorithms
- [ ] Automatic camera calibration
- [ ] Support for multiple camera systems
- [ ] Cloud-based 3D reconstruction
- [ ] Integration with audio HRTF measurement systems

## License

GPL-3.0-or-later

## Authors

Pierre F. Aubert <pierre@spinorama.org>
