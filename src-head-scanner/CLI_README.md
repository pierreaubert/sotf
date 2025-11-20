# Head Scanner CLI

Command-line interface for 3D head scanning with real-time camera capture, feature detection, bundle adjustment, and mesh generation.

## Features

- 🎥 **Real-time camera capture** from webcam or external cameras
- 🔍 **Feature detection** using classical CV or ML models
- 📊 **Live progress tracking** with coverage visualization
- 🔧 **Bundle adjustment** optimization for accurate 3D reconstruction
- 💾 **Mesh export** in OBJ format
- ⚡ **GPU acceleration** support

## Installation

### Prerequisites

1. **Rust toolchain** (1.70+)
2. **OpenCV** (4.x)
   ```bash
   # macOS
   brew install opencv
   export DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Frameworks
   
   # Ubuntu/Debian
   sudo apt-get install libopencv-dev clang libclang-dev
   
   # Fedora
   sudo dnf install opencv-devel clang clang-devel
   ```

3. **Clang/LLVM** (for OpenCV bindings)
   ```bash
   # macOS
   xcode-select --install
   
   # Linux
   sudo apt-get install clang libclang-dev
   ```

### Build

```bash
cd src-head-scanner
cargo build --release --bin head-scanner-cli
```

## Usage

### Quick Start

```bash
# Start a scan with default settings
./target/release/head-scanner-cli scan

# Scan with custom output file
./target/release/head-scanner-cli scan --output my_head.obj

# Scan with higher resolution
./target/release/head-scanner-cli scan --width 1920 --height 1080
```

### Commands

#### `scan` - Start a new head scan

```bash
head-scanner-cli scan [OPTIONS]

Options:
  -o, --output <FILE>           Output file path [default: head_scan.obj]
      --width <PIXELS>          Camera resolution width [default: 1280]
      --height <PIXELS>         Camera resolution height [default: 720]
      --fps <FPS>               Frame rate [default: 30]
      --min-coverage <PERCENT>  Minimum coverage percentage [default: 85]
      --bundle-adjustment       Enable bundle adjustment [default: true]
      --max-duration <SECONDS>  Maximum scan duration [default: 120]
      --model <PATH>            Path to vision model (ONNX)
  -c, --camera <INDEX>          Camera device index [default: 0]
  -v, --verbose                 Enable verbose logging
```

#### `test` - Test camera connection

```bash
head-scanner-cli test [OPTIONS]

Options:
  -d, --duration <SECONDS>  Test duration [default: 5]
  -c, --camera <INDEX>      Camera device index [default: 0]
```

#### `info` - Show camera information

```bash
head-scanner-cli info [OPTIONS]

Options:
  -c, --camera <INDEX>  Camera device index [default: 0]
```

## Examples

### Basic Scan

```bash
# Test camera first
head-scanner-cli test --duration 10

# Run scan with default settings
head-scanner-cli scan --output head_model.obj
```

### High-Quality Scan

```bash
head-scanner-cli scan \
  --output high_quality_head.obj \
  --width 1920 \
  --height 1080 \
  --fps 60 \
  --min-coverage 95 \
  --max-duration 180
```

### Quick Scan (Lower Quality)

```bash
head-scanner-cli scan \
  --output quick_scan.obj \
  --width 640 \
  --height 480 \
  --min-coverage 70 \
  --max-duration 60
```

### Using External Camera

```bash
# Check available cameras
head-scanner-cli info --camera 0
head-scanner-cli info --camera 1

# Scan with external camera
head-scanner-cli scan --camera 1 --output external_cam_scan.obj
```

### With ML Model

```bash
head-scanner-cli scan \
  --model path/to/face_detection_model.onnx \
  --output ml_enhanced_scan.obj
```

## Scanning Tips

1. **Lighting**: Ensure good, even lighting on the subject
2. **Movement**: Slowly rotate around the subject or have them rotate
3. **Distance**: Maintain 50-100cm distance from the subject
4. **Coverage**: Aim for 360° coverage for best results
5. **Stability**: Use a tripod or stable surface for the camera

## Output Format

The CLI exports meshes in Wavefront OBJ format, which includes:
- Vertex positions (v)
- Vertex normals (vn)
- Faces (f)

The OBJ file can be imported into:
- Blender
- MeshLab
- CloudCompare
- Any 3D modeling software

## Troubleshooting

### Camera Not Found

```bash
# List available cameras
head-scanner-cli info --camera 0
head-scanner-cli info --camera 1
```

### Low Coverage

- Move closer to the subject
- Ensure good lighting
- Increase scan duration
- Lower min-coverage threshold

### Build Errors

**OpenCV not found:**
```bash
# macOS
brew install opencv
export DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Frameworks

# Linux
sudo apt-get install libopencv-dev
```

**Clang not found:**
```bash
# macOS
xcode-select --install

# Linux
sudo apt-get install clang libclang-dev
```

## Performance

- **CPU**: Multi-core recommended (4+ cores)
- **RAM**: 4GB minimum, 8GB+ recommended
- **GPU**: Optional, improves feature detection speed
- **Camera**: 720p minimum, 1080p+ recommended

## Architecture

The CLI integrates several components:

1. **Camera Capture** (`camera.rs`) - OpenCV-based frame capture
2. **Feature Detection** (`vision.rs`) - Classical CV or ML-based
3. **3D Reconstruction** (`reconstruction.rs`) - Structure-from-Motion
4. **Bundle Adjustment** (`bundle_adjustment.rs`) - Full Jacobian optimization
5. **Mesh Generation** (`mesh.rs`) - Triangulation and export

## Development

### Run Tests

```bash
cargo test --package head-scanner
```

### Run with Logging

```bash
RUST_LOG=debug head-scanner-cli scan --verbose
```

### Profile Performance

```bash
cargo build --release --bin head-scanner-cli
cargo flamegraph --bin head-scanner-cli -- scan --max-duration 30
```

## License

See LICENSE.md in the repository root.
