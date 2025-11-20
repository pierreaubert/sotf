# Head Scanner

Professional 3D head scanning system using computer vision for HRTF (Head-Related Transfer Function) optimization.

## Features

✅ **Real-time camera capture** with live video display  
✅ **Automatic feature detection** using ML models or classical CV  
✅ **Camera calibration** with checkerboard patterns  
✅ **Bundle adjustment** for optimized 3D reconstruction  
✅ **Advanced mesh smoothing** (Laplacian, Taubin, Bilateral, HC)  
✅ **GPU acceleration** for ML inference  
✅ **Multi-format ONNX support** (NCHW/NHWC, YOLOv4, etc.)  
✅ **Professional quality** output with texture mapping  

---

## Table of Contents

- [Quick Start](#quick-start)
- [Installation](#installation)
- [Camera Calibration](#camera-calibration)
- [Scanning](#scanning)
- [GPU Acceleration](#gpu-acceleration)
- [Mesh Smoothing](#mesh-smoothing)
- [ONNX Model Support](#onnx-model-support)
- [Advanced Usage](#advanced-usage)
- [Architecture](#architecture)
- [Development](#development)
- [Troubleshooting](#troubleshooting)

---

## Quick Start

### 1. Install Dependencies

**macOS:**
```bash
brew install opencv
export DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Frameworks
```

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get update
sudo apt-get install -y libopencv-dev clang libclang-dev pkg-config
```

### 2. Build

```bash
cd src-head-scanner
cargo build --release --bin head-scanner-cli
```

### 3. Test Camera

```bash
./target/release/head-scanner-cli test --duration 5
./target/release/head-scanner-cli info
```

### 4. Run Your First Scan

```bash
# Quick test scan (30 seconds)
./target/release/head-scanner-cli scan \
  --output my_first_scan.obj \
  --max-duration 30 \
  --min-coverage 50

# View the result
open my_first_scan.obj  # macOS
meshlab my_first_scan.obj  # Linux
```

---

## Installation

### Prerequisites

#### macOS

```bash
# Install OpenCV
brew install opencv

# Set library path
export DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Frameworks

# Install Xcode command line tools (if needed)
xcode-select --install
```

#### Linux (Ubuntu/Debian)

```bash
# Install dependencies
sudo apt-get update
sudo apt-get install -y \
    libopencv-dev \
    clang \
    libclang-dev \
    pkg-config

# Verify OpenCV installation
pkg-config --modversion opencv4
```

### Build from Source

```bash
cd src-head-scanner

# Build the CLI
cargo build --release --bin head-scanner-cli

# Binary location
ls -lh target/release/head-scanner-cli
```

### Verify Installation

```bash
# Test camera connection
./target/release/head-scanner-cli test --duration 5

# Show camera info
./target/release/head-scanner-cli info
```

---

## Camera Calibration

Professional camera calibration using checkerboard patterns and Zhang's method.

### Why Calibrate?

Camera calibration determines:
- **Focal length** (fx, fy) - Camera magnification
- **Principal point** (cx, cy) - Optical center
- **Distortion coefficients** - Lens distortion correction

**Benefits:**
- ✅ Accurate 3D reconstruction
- ✅ Corrected lens distortion
- ✅ Professional quality results

### Quick Calibration

```bash
# Run calibration with default settings (9x6 checkerboard, 25mm squares)
./target/release/head-scanner-cli calibrate

# Custom checkerboard
./target/release/head-scanner-cli calibrate \
  --board-width 9 \
  --board-height 6 \
  --square-size 25.0 \
  --output my_calibration.json
```

### Calibration Process

**1. Prepare Checkerboard:**
- Print a 9x6 inner corners pattern (10x7 squares)
- Use 25mm squares (measure accurately!)
- Mount on flat, rigid surface

**Download patterns:**
- [OpenCV Checkerboard Generator](https://calib.io/pages/camera-calibration-pattern-generator)
- [Calibration.io](https://calibration.io/)

**2. Run Calibration:**

```bash
./target/release/head-scanner-cli calibrate
```

**3. Capture Frames:**
- Show checkerboard to camera from different angles
- Move slowly to avoid motion blur
- Cover all areas: corners, edges, center
- System captures automatically when detected
- Need 15-30 frames for good results

**4. Review Results:**

```
✓ Calibration successful!

Results:
  Focal length (fx, fy): 1536.42, 1538.91
  Principal point (cx, cy): 640.12, 360.45
  RMS error: 0.3421 pixels
  Frames used: 20
  Distortion coefficients:
    k1=-0.285432, k2=0.094521, p1=-0.000234, p2=0.000156, k3=-0.012345
```

**Quality Metrics:**
- **< 0.5 pixels**: Excellent
- **0.5-1.0 pixels**: Good
- **1.0-2.0 pixels**: Acceptable
- **> 2.0 pixels**: Poor (recalibrate)

### Using Calibration

```bash
# Use calibrated parameters in scan
./target/release/head-scanner-cli scan \
  --calibration camera_calibration.json \
  --output calibrated_scan.obj
```

### Calibration Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `--output` | `camera_calibration.json` | Output file path |
| `--board-width` | `9` | Inner corners width |
| `--board-height` | `6` | Inner corners height |
| `--square-size` | `25.0` | Square size in mm |
| `--min-frames` | `15` | Minimum frames required |
| `--max-frames` | `30` | Maximum frames to capture |

---

## Scanning

### Basic Scan

```bash
# Default scan (2 minutes, 85% coverage)
./target/release/head-scanner-cli scan --output scan.obj

# Quick test (30 seconds)
./target/release/head-scanner-cli scan \
  --output quick.obj \
  --max-duration 30 \
  --min-coverage 50
```

### High Quality Scan

```bash
./target/release/head-scanner-cli scan \
  --calibration camera_calibration.json \
  --width 1920 \
  --height 1080 \
  --min-coverage 95 \
  --bundle-adjustment \
  --smooth taubin \
  --smooth-iterations 10 \
  --gpu \
  --output high_quality.obj
```

### Scan Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `--output` | `scan.obj` | Output mesh file |
| `--camera` | `0` | Camera device index |
| `--width` | `1280` | Camera width |
| `--height` | `720` | Camera height |
| `--fps` | `30` | Camera FPS |
| `--min-coverage` | `85` | Minimum coverage % |
| `--max-duration` | `120` | Max scan duration (seconds) |
| `--calibration` | - | Calibration file (optional) |
| `--model` | - | ONNX model path (optional) |
| `--bundle-adjustment` | `true` | Enable optimization |
| `--display` | `true` | Show live video |
| `--gpu` | `true` | Enable GPU acceleration |
| `--smooth` | `taubin` | Smoothing algorithm |
| `--smooth-iterations` | `5` | Smoothing iterations |
| `--verbose` | `false` | Verbose logging |

### Scan Workflow

```
📹 Starting scan...
✓ Camera opened (1280x720 @ 30fps)
✓ Vision model loaded (GPU enabled)

🎥 Scanning in progress...
[████████████████████░░░░] 85% coverage | 120 frames | 60.0s | 🚀 GPU

✓ Scan complete!
  Coverage: 87.3%
  Frames: 145
  Duration: 72.5s

🔧 Running bundle adjustment...
  Initial error: 2.45 pixels
  Final error: 0.82 pixels
  Iterations: 15
  Time: 3.2s

✨ Applying mesh smoothing (taubin)...
  ✓ Taubin smoothing applied (5 iterations)
  Time: 0.8s

💾 Exporting mesh...
  Vertices: 12,543
  Triangles: 24,892
  File: scan.obj

✨ Scan complete!
```

---

## GPU Acceleration

GPU acceleration for ML model inference using ONNX Runtime.

### Status

**Current**: Placeholder implementation (CPU optimized)  
**Planned**: Full GPU support via ONNX Runtime execution providers

### Usage

```bash
# Enable GPU acceleration (currently uses optimized CPU)
./target/release/head-scanner-cli scan --gpu --model model.onnx

# Disable GPU
./target/release/head-scanner-cli scan --no-gpu --model model.onnx
```

### Platform Support

**macOS (Apple Silicon):**
- Target: CoreML / Neural Engine
- Status: Planned for future `ort` versions

**Windows:**
- Target: DirectML
- Status: Planned for future `ort` versions

**Linux:**
- Target: CUDA / ROCm
- Status: Planned for future `ort` versions

### Current Optimizations

Even without GPU providers, ONNX Runtime uses:
- ✅ SIMD instructions (AVX, NEON)
- ✅ Multi-threading
- ✅ Optimized kernels
- ✅ Graph optimizations

### Future Implementation

When `ort` crate supports execution providers:

```rust
// Future GPU configuration
let session = Session::builder()?
    .with_execution_providers([
        CUDAExecutionProvider::default(),
        CoreMLExecutionProvider::default(),
        DirectMLExecutionProvider::default(),
    ])?
    .commit_from_file(model_path)?;
```

---

## Mesh Smoothing

Advanced mesh smoothing algorithms for high-quality 3D models.

### Algorithms

**1. Laplacian Smoothing**
- Fast, simple smoothing
- May cause mesh shrinkage
- Good for quick cleanup

**2. Taubin Smoothing** (Recommended)
- Volume-preserving
- No mesh shrinkage
- Best general-purpose choice

**3. Bilateral Smoothing**
- Feature-preserving
- Maintains sharp edges
- Best for detailed models

**4. HC Smoothing**
- Humphrey's Classes method
- Balances smoothing and preservation
- Good for organic shapes

### Usage

```bash
# Taubin smoothing (default)
./target/release/head-scanner-cli scan \
  --smooth taubin \
  --smooth-iterations 5

# Bilateral smoothing (feature-preserving)
./target/release/head-scanner-cli scan \
  --smooth bilateral \
  --smooth-iterations 10

# Laplacian smoothing (fast)
./target/release/head-scanner-cli scan \
  --smooth laplacian \
  --smooth-iterations 3

# HC smoothing
./target/release/head-scanner-cli scan \
  --smooth hc \
  --smooth-iterations 5

# No smoothing
./target/release/head-scanner-cli scan --smooth none
```

### Parameters

**Laplacian:**
- Lambda: 0.5 (smoothing factor)
- Iterations: 3-5

**Taubin:**
- Lambda: 0.6 (inflation)
- Mu: -0.63 (deflation)
- Iterations: 5-10

**Bilateral:**
- Spatial sigma: 0.5
- Normal sigma: 0.3
- Iterations: 5-15

**HC:**
- Alpha: 0.5
- Beta: 0.65
- Iterations: 5-10

### Comparison

| Algorithm | Speed | Quality | Shrinkage | Features |
|-----------|-------|---------|-----------|----------|
| Laplacian | ⚡⚡⚡ | ⭐⭐ | ❌ High | Lost |
| Taubin | ⚡⚡ | ⭐⭐⭐⭐ | ✅ None | Smoothed |
| Bilateral | ⚡ | ⭐⭐⭐⭐⭐ | ✅ None | Preserved |
| HC | ⚡⚡ | ⭐⭐⭐⭐ | ✅ Low | Balanced |

**Recommendation:**
- **General use**: Taubin (5 iterations)
- **High detail**: Bilateral (10 iterations)
- **Fast preview**: Laplacian (3 iterations)
- **Organic shapes**: HC (5 iterations)

---

## ONNX Model Support

Support for various ONNX model formats and architectures.

### Supported Formats

**NCHW (Channels First)** - PyTorch/ONNX Standard
- Shape: `[batch, channels, height, width]`
- Example: `[1, 3, 224, 224]`
- Models: ResNet, MobileNet, EfficientNet

**NHWC (Channels Last)** - TensorFlow/YOLO Format
- Shape: `[batch, height, width, channels]`
- Example: `[1, 416, 416, 3]`
- Models: YOLOv4, YOLOv5, TensorFlow models

### Automatic Detection

The system automatically detects model format by:
1. Inspecting input tensor shape
2. Detecting if last dimension is 3 (channels) → NHWC
3. Detecting common sizes (416 for YOLO, 224 for others)
4. Preprocessing accordingly

### YOLOv4 Support

**Output Structure:**
```
Shape: [1, 52, 52, 3, 85]
       │  │    │   │  └─ 85 = 4 (bbox) + 1 (objectness) + 80 (COCO classes)
       │  │    │   └─ 3 anchors per grid cell
       │  │    └─ 52 grid width
       │  └─ 52 grid height
       └─ batch size
```

**Usage:**
```bash
./target/release/head-scanner-cli scan \
  --model ~/Downloads/yolov4.onnx \
  --output yolo_scan.obj \
  --verbose
```

### Supported Model Types

**Object Detection:**
- YOLOv3, YOLOv4, YOLOv5
- SSD, Faster R-CNN
- RetinaNet

**Keypoint Detection:**
- OpenPose
- MediaPipe
- Custom keypoint models

**Face Detection:**
- MTCNN
- RetinaFace
- Custom face models

### Without a Model

Classical computer vision fallback:

```bash
# Uses OpenCV Haar Cascades
./target/release/head-scanner-cli scan --output classical_scan.obj
```

### Model Requirements

1. **Input**: RGB image (normalized to [0, 1])
2. **Output**: One of:
   - Bounding boxes: `[batch, num_detections, 5+]`
   - Keypoints: `[batch, num_keypoints, 3]`
   - Feature maps: Any shape

### Creating Custom Models

```python
import torch
import torch.onnx

# Your PyTorch model
model = YourModel()
model.eval()

# Export to ONNX
dummy_input = torch.randn(1, 3, 416, 416)
torch.onnx.export(
    model,
    dummy_input,
    "your_model.onnx",
    input_names=['input'],
    output_names=['output'],
    dynamic_axes={'input': {0: 'batch'}, 'output': {0: 'batch'}}
)
```

---

## Advanced Usage

### Complete Production Pipeline

```bash
# 1. Calibrate camera (once)
./target/release/head-scanner-cli calibrate \
  --output production_calibration.json

# 2. High-quality scan
./target/release/head-scanner-cli scan \
  --calibration production_calibration.json \
  --model ~/models/face_detection.onnx \
  --width 1920 \
  --height 1080 \
  --min-coverage 95 \
  --max-duration 180 \
  --bundle-adjustment \
  --smooth bilateral \
  --smooth-iterations 15 \
  --gpu \
  --output production_scan.obj \
  --verbose
```

### Multiple Cameras

```bash
# Calibrate each camera
./target/release/head-scanner-cli calibrate --camera 0 --output cam0.json
./target/release/head-scanner-cli calibrate --camera 1 --output cam1.json

# Scan with specific camera
./target/release/head-scanner-cli scan --camera 1 --calibration cam1.json
```

### Batch Processing

```bash
#!/bin/bash
# Scan multiple subjects

for subject in subject1 subject2 subject3; do
  ./target/release/head-scanner-cli scan \
    --calibration camera_calibration.json \
    --output "${subject}_scan.obj" \
    --max-duration 90
done
```

### API Usage

```rust
use head_scanner::*;

// Create scanner configuration
let config = ScannerConfig {
    camera_index: 0,
    width: 1280,
    height: 720,
    fps: 30,
    min_coverage: 0.85,
    ..Default::default()
};

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

### Bundle Adjustment

```rust
use head_scanner::bundle_adjustment::BundleAdjuster;
use head_scanner::reconstruction::CameraIntrinsics;

// Initialize bundle adjuster
let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
let adjuster = BundleAdjuster::new(intrinsics);

// Optimize poses and points
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
```

---

## Architecture

### Modules

- **bundle_adjustment**: Bundle adjustment optimizer for SfM refinement
- **calibration**: Camera calibration with checkerboard patterns
- **camera**: Camera capture and frame management
- **convexhull**: 3D convex hull computation
- **coverage**: Scan coverage tracking
- **mesh**: 3D mesh generation, smoothing, and export
- **pointcloud**: Point cloud data structure and operations
- **reconstruction**: Structure-from-Motion and 3D reconstruction
- **stereo**: Stereo camera support and depth estimation
- **texture**: Texture mapping from camera frames
- **vision**: Computer vision features (ML and classical)

### Data Flow

```
Camera → Frame Capture → Feature Detection → 3D Reconstruction
                                                     ↓
                                            Bundle Adjustment
                                                     ↓
                                              Point Cloud
                                                     ↓
                                              Mesh Generation
                                                     ↓
                                              Mesh Smoothing
                                                     ↓
                                              Export (OBJ/STL)
```

### Dependencies

**System Libraries:**
- OpenCV: Computer vision operations
- ONNX Runtime: ML model inference

**Rust Crates:**
- `convexhull3d`: 3D convex hull computation
- `nalgebra`: Linear algebra
- `opencv`: Computer vision bindings
- `ort`: ONNX Runtime bindings
- `image`: Image processing
- `ndarray`: N-dimensional arrays
- `serde`: Serialization
- `tokio`: Async runtime
- `clap`: CLI parsing

---

## Development

### Running Tests

```bash
# All tests
cargo test --package head-scanner

# Integration tests only
cargo test --package head-scanner --test integration_tests

# Unit tests only
cargo test --package head-scanner --lib

# With output
cargo test -- --nocapture
```

### Running Benchmarks

```bash
cargo bench --package head-scanner
```

### Running Examples

```bash
# Simple scanning example
cargo run --example simple_scan

# Structure-from-Motion
cargo run --example sfm_reconstruction

# Stereo depth estimation
cargo run --example stereo_depth
```

### Code Style

```bash
# Format code
cargo fmt

# Lint code
cargo clippy -- -D warnings

# Type checking
cargo check
```

### Adding Features

1. Implement new module in `src/`
2. Add module to `lib.rs`
3. Write unit tests in the module
4. Add integration tests to `tests/`
5. Create example in `examples/` if appropriate
6. Update README

---

## Troubleshooting

### Camera Issues

**"Camera not found"**
- Check camera permissions (System Preferences on macOS)
- Try different camera index: `--camera 1`
- Ensure no other app is using the camera
- Test with: `./target/release/head-scanner-cli test`

**"Camera permission denied"**
- macOS: System Preferences → Security & Privacy → Camera
- Linux: Check `/dev/video*` permissions

### Build Issues

**"OpenCV build failed"**

macOS:
```bash
brew install opencv
export DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Frameworks
```

Linux:
```bash
sudo apt-get install libopencv-dev clang libclang-dev
```

**"Cannot find -lopencv"**
```bash
# Verify OpenCV installation
pkg-config --modversion opencv4
pkg-config --libs opencv4
```

### Calibration Issues

**"Checkerboard not detected"**
- Ensure good lighting
- Check pattern is flat and fully visible
- Verify correct board size parameters
- Try different angles
- Clean camera lens

**"High RMS error"**
- Recalibrate with more frames
- Verify square size measurement
- Use better quality printout
- Ensure pattern is perfectly flat

### Scanning Issues

**"Low coverage warning"**
- Move camera around subject more
- Increase scan duration: `--max-duration 180`
- Lower coverage requirement: `--min-coverage 70`

**"No features detected"**
- Improve lighting conditions
- Ensure subject is visible
- Try different ML model
- Use classical CV: remove `--model` flag

**"Mesh quality poor"**
- Calibrate camera first
- Enable bundle adjustment: `--bundle-adjustment`
- Use mesh smoothing: `--smooth taubin`
- Increase coverage: `--min-coverage 95`
- Use higher resolution: `--width 1920 --height 1080`

### Performance Issues

**"Slow inference"**
- Use smaller model (MobileNet, TinyYOLO)
- Reduce camera resolution
- Enable GPU acceleration (when available)

**"High memory usage"**
- Reduce scan duration
- Lower camera resolution
- Limit point cloud size

---

## Performance Tips

**Resolution:**
- Start with 1280x720
- Increase to 1920x1080 for better quality
- Higher resolution = slower but more detailed

**Coverage:**
- 85% recommended for good results
- 95% for high quality
- Lower for quick tests

**Duration:**
- 60-120 seconds for good results
- 30 seconds for quick tests
- Longer for complex subjects

**Lighting:**
- Even, bright lighting
- Avoid shadows
- No glare or reflections

**Movement:**
- Slow, steady rotation
- Cover all angles
- Maintain consistent distance

---

## License

GPL-3.0-or-later

## Authors

Pierre F. Aubert <pierre@spinorama.org>

---

## Summary

**Complete 3D Head Scanning Pipeline:**

1. **Calibrate** → `calibrate` command
2. **Scan** → `scan` command with calibration
3. **Optimize** → Bundle adjustment (automatic)
4. **Smooth** → Mesh smoothing (Taubin recommended)
5. **Export** → OBJ/STL format

**Recommended Workflow:**

```bash
# One-time calibration
./target/release/head-scanner-cli calibrate

# Production scan
./target/release/head-scanner-cli scan \
  --calibration camera_calibration.json \
  --smooth taubin \
  --gpu \
  --output production.obj
```

**Status:**
- ✅ Camera calibration: Fully implemented
- ✅ Bundle adjustment: Fully implemented
- ✅ Mesh smoothing: Fully implemented
- ✅ ONNX support: Fully implemented
- 🔄 GPU acceleration: Planned (CPU optimized)

**Build:** `cargo build --release --bin head-scanner-cli`  
**Docs:** This README  
**Examples:** `examples/` directory
