# Head Scanner CLI - Quick Start

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

## Support

For issues or questions:
- Check `CLI_README.md` for detailed documentation
- Review troubleshooting section above
- Check OpenCV installation: `pkg-config --modversion opencv4`
