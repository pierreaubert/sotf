# Head Scanner

A 3D head scanning system using computer vision for HRTF (Head-Related Transfer Function) optimization.

## Prerequisites Setup

### macOS
```bash
# Install OpenCV
brew install opencv

# Set library path
export DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Frameworks

# Install Xcode command line tools (if not already installed)
xcode-select --install
```

### Linux (Ubuntu/Debian)
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

## Build

```bash
cd src-head-scanner

# Build the CLI
cargo build --release --bin head-scanner-cli

# Binary will be at: target/release/head-scanner-cli
```

## Quick Test

```bash
# Test camera connection
./target/release/head-scanner-cli test --duration 5

# Show camera info
./target/release/head-scanner-cli info
```

## Run Your First Scan

```bash
# Basic scan (will take ~2 minutes)
./target/release/head-scanner-cli scan --output my_first_scan.obj

# Quick scan for testing (30 seconds)
./target/release/head-scanner-cli scan \
  --output quick_test.obj \
  --max-duration 30 \
  --min-coverage 50
```

## View Results

### macOS
```bash
# Open in default 3D viewer
open my_first_scan.obj
```

### Linux
```bash
# Install MeshLab if not already installed
sudo apt-get install meshlab

# View the mesh
meshlab my_first_scan.obj
```

## Common Issues

### "Camera not found"
- Check camera permissions in System Preferences (macOS)
- Try different camera index: `--camera 1`
- Ensure no other app is using the camera

### "OpenCV build failed"
```bash
# macOS: Ensure OpenCV is installed
brew install opencv
export DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Frameworks

# Linux: Install development packages
sudo apt-get install libopencv-dev clang libclang-dev
```

### "Low coverage warning"
- Move camera around the subject more
- Increase scan duration: `--max-duration 180`
- Lower coverage requirement: `--min-coverage 70`

## Next Steps

1. **Read full documentation**: See `CLI_README.md`
2. **Run demo script**: `./examples/cli_demo.sh`
3. **Experiment with settings**: `head-scanner-cli scan --help`
4. **Enable bundle adjustment**: Already enabled by default for best quality

## Key Features

✅ Real-time camera capture
✅ Automatic feature detection
✅ Live progress tracking
✅ Bundle adjustment optimization (NEW!)
✅ OBJ mesh export
✅ GPU acceleration support

## Performance Tips

- **Resolution**: Start with 1280x720, increase for better quality
- **Coverage**: 85% is recommended, 95% for high quality
- **Duration**: 60-120 seconds for good results
- **Lighting**: Ensure even, bright lighting
- **Movement**: Slow, steady rotation around subject

## Example Commands

```bash
# High quality scan
head-scanner-cli scan --width 1920 --height 1080 --min-coverage 95

# Fast scan
head-scanner-cli scan --max-duration 30 --min-coverage 60

# External camera
head-scanner-cli scan --camera 1

# Verbose output
head-scanner-cli scan --verbose
```

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

## MODEL SUPPORT

p# ONNX Model Support

The Head Scanner CLI now supports both NCHW and NHWC format ONNX models with automatic detection.

## Supported Formats

### NCHW (Channels First) - PyTorch/ONNX Standard
- Shape: `[batch, channels, height, width]`
- Example: `[1, 3, 224, 224]`
- Common in: ResNet, MobileNet, EfficientNet

### NHWC (Channels Last) - TensorFlow/YOLO Format
- Shape: `[batch, height, width, channels]`
- Example: `[1, 416, 416, 3]`
- Common in: YOLOv4, YOLOv5, TensorFlow models

## Automatic Detection

The CLI automatically detects the model format by:
1. Inspecting the input tensor shape
2. Detecting if last dimension is 3 (channels) → NHWC
3. Detecting common sizes (416 for YOLO, 224 for others)
4. Preprocessing the image accordingly

## Usage with YOLOv4

Your YOLOv4 model expects `[1, 416, 416, 3]` format (NHWC).

```bash
# The CLI will automatically detect and use NHWC format
../target/release/head-scanner-cli scan \
  --model ~/Downloads/yolov4.onnx \
  --output yolo_scan.obj \
  --verbose
```

## Troubleshooting

### Dimension Mismatch Errors

If you see errors like:
```
Got: 3 Expected: 416
```

This means the model detection failed. The fix has been applied to:
- Detect NHWC format (channels last)
- Automatically resize to 416x416 for YOLO models
- Properly arrange data in memory

### Model Input Inspection

When running with `--verbose`, you'll see:
```
Model input: name='input_1:0', shape=...
Preprocessing for 416x416, channels_last=true
```

This confirms the format detection.

## Supported Model Types

### Object Detection
- YOLOv3, YOLOv4, YOLOv5
- SSD, Faster R-CNN
- RetinaNet

### Keypoint Detection
- OpenPose
- MediaPipe
- Custom keypoint models

### Face Detection
- MTCNN
- RetinaFace
- Custom face models

## Model Requirements

1. **Input**: RGB image (normalized to [0, 1])
2. **Output**: One of:
   - Bounding boxes: `[batch, num_detections, 5+]` (x, y, w, h, conf, ...)
   - Keypoints: `[batch, num_keypoints, 3]` (x, y, conf)
   - Feature maps: Any shape (will use center point)

## Example Models

### YOLOv4 (NHWC)
```bash
# Input: [1, 416, 416, 3]
# Output: [1, num_boxes, 85] (80 classes + 5 bbox params)
../target/release/head-scanner-cli scan --model yolov4.onnx
```

### MobileNet (NCHW)
```bash
# Input: [1, 3, 224, 224]
# Output: [1, 1000] (classification)
../target/release/head-scanner-cli scan --model mobilenet.onnx
```

### MediaPipe Face (NCHW)
```bash
# Input: [1, 3, 192, 192]
# Output: [1, 468, 3] (face landmarks)
../target/release/head-scanner-cli scan --model face_landmarks.onnx
```

## Without a Model (Classical CV)

If you don't have an ONNX model, the CLI falls back to classical computer vision:

```bash
# Uses OpenCV Haar Cascades for face detection
../target/release/head-scanner-cli scan --output classical_scan.obj
```

## Performance Tips

1. **GPU Acceleration**: Models run on CPU by default. For GPU:
   - Ensure CUDA/ROCm is installed
   - Use models optimized for your hardware

2. **Model Size**: Smaller models (MobileNet, TinyYOLO) are faster

3. **Input Resolution**: Lower resolution = faster processing
   ```bash
   --width 640 --height 480  # Faster
   --width 1920 --height 1080  # Better quality
   ```

## Creating Your Own Model

To create a compatible ONNX model:

```python
import torch
import torch.onnx

# Your PyTorch model
model = YourModel()
model.eval()

# Export to ONNX
dummy_input = torch.randn(1, 3, 416, 416)  # NCHW
torch.onnx.export(
    model,
    dummy_input,
    "your_model.onnx",
    input_names=['input'],
    output_names=['output'],
    dynamic_axes={'input': {0: 'batch'}, 'output': {0: 'batch'}}
)
```

## Debugging

Enable verbose logging to see model details:

```bash
RUST_LOG=debug ../target/release/head-scanner-cli scan \
  --model your_model.onnx \
  --verbose
```

This will show:
- Model input shape and format
- Preprocessing parameters
- Inference results
- Feature detection output

## Next Steps

1. Test with your model: `--model path/to/model.onnx`
2. Adjust camera settings: `--width`, `--height`, `--fps`
3. Configure coverage: `--min-coverage 85`
4. Enable bundle adjustment: `--bundle-adjustment` (default: on)

# YOLOv4 Support - Now Working! ✅

The Head Scanner CLI now fully supports YOLOv4 and other YOLO models with proper output parsing.

## YOLOv4 Output Structure

```
Shape: [1, 52, 52, 3, 85]
       │  │    │   │  └─ 85 = 4 (bbox) + 1 (objectness) + 80 (COCO classes)
       │  │    │   └─ 3 anchors per grid cell
       │  │    └─ 52 grid width
       │  └─ 52 grid height
       └─ batch size
```

The parser:
1. Iterates through each grid cell (52x52)
2. Checks each anchor (3 per cell)
3. Extracts objectness score
4. Finds best class (max of 80 COCO classes)
5. Combines: `confidence = objectness × class_score`
6. Filters by threshold (0.5)
7. Converts to image coordinates

## Usage

```bash
# Your YOLOv4 model now works end-to-end!
../target/release/head-scanner-cli scan \
  --model ~/Downloads/yolov4.onnx \
  --output yolo_scan.obj \
  --verbose \
  --camera 0
```

## Expected Output

With `--verbose`, you'll see:

```
[DEBUG] Model input shape: Tensor { ty: Float32, shape: [-1, 416, 416, 3], ... }
[DEBUG] Preprocessing for 416x416, channels_last=true
[INFO]  Detected YOLOv4 output format: [1, 52, 52, 3, 85]
[INFO]  Extracted 15 features from YOLOv4 output
```

## COCO Classes

YOLOv4 detects 80 COCO object classes:
- Class 0: person (most relevant for head scanning!)
- Class 1: bicycle
- Class 2: car
- ... (see COCO dataset for full list)

For head scanning, the model will primarily detect:
- **Class 0 (person)**: Full body/head detection
- Face landmarks can be extracted from bounding boxes

## Multi-Scale Detection

YOLOv4 typically outputs 3 scales:
- `[1, 52, 52, 3, 85]` - Small objects (close-up)
- `[1, 26, 26, 3, 85]` - Medium objects
- `[1, 13, 13, 3, 85]` - Large objects

The CLI processes the first output. For multi-scale, it would iterate through all outputs.

## Performance

**Inference Speed** (CPU):
- 416x416 input: ~100-200ms per frame
- 1920x1080 camera: Downsampled to 416x416

**Detection Quality**:
- Confidence threshold: 0.5 (adjustable in code)
- Good for head/person detection
- May need fine-tuning for specific use cases

## Troubleshooting

### No Features Detected

If you see "Extracted 0 features":
1. **Check camera view**: Ensure person is visible
2. **Lighting**: Improve lighting conditions
3. **Distance**: Move closer to camera
4. **Lower threshold**: Edit `confidence_threshold` in code (line 448)

### Too Many Features

If too many detections:
1. **Increase threshold**: Change from 0.5 to 0.7
2. **Filter by class**: Only keep class 0 (person)
3. **Apply NMS**: Non-maximum suppression (already included)

### Wrong Coordinates

If features are in wrong positions:
- Check camera resolution matches preprocessing
- Verify grid coordinate conversion
- Enable debug logging to inspect values

## Advanced: Custom YOLO Models

### YOLOv3
Same format, works automatically:
```bash
--model yolov3.onnx
```

### YOLOv5
May use different output format. Check shape and adjust parser if needed.

### Tiny YOLO
Faster, less accurate:
```bash
--model yolov4-tiny.onnx
```

### Custom Classes
If you trained YOLO on custom classes:
- Adjust `num_classes` in parser (line 463)
- Update class names in output

## Integration with Bundle Adjustment

The detected features feed into:
1. **Feature tracking** across frames
2. **3D reconstruction** via triangulation
3. **Bundle adjustment** for pose optimization
4. **Mesh generation** from point cloud

YOLOv4 provides robust person detection, which helps:
- Initialize head tracking
- Maintain focus on subject
- Filter background noise
- Improve reconstruction quality

## Next Steps

1. **Test with your camera**:
   ```bash
   ../target/release/head-scanner-cli scan --model yolov4.onnx --verbose
   ```

2. **Adjust parameters**:
   - Camera resolution: `--width 1920 --height 1080`
   - Coverage: `--min-coverage 85`
   - Duration: `--max-duration 120`

3. **View results**:
   ```bash
   open yolo_scan.obj  # macOS
   meshlab yolo_scan.obj  # Linux
   ```

## Code References

**Input preprocessing**: `src/vision.rs:340-410`
- Format detection
- NHWC layout
- 416x416 resizing

**Output parsing**: `src/vision.rs:439-491`
- 5D tensor handling
- Grid coordinate conversion
- Confidence filtering

**Feature extraction**: `src/vision.rs:412-545`
- Multi-format support
- NMS application
- Feature tracking

---

**Status**: ✅ Fully functional with YOLOv4
**Build**: `../target/release/head-scanner-cli`
**Docs**: See `MODEL_SUPPORT.md` for more model types


## License

GPL-3.0-or-later

## Authors

Pierre F. Aubert <pierre@spinorama.org>
