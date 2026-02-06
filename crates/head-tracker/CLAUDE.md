# head-tracker (lib: `sotf-head-tracker`)

Camera-based head tracking for spatial audio applications.

## Purpose

Tracks head position/rotation using camera input for binaural audio and XTC adjustments.

## Platform Support

- **macOS**: Apple Vision framework (default)
- **Linux**: v4l (Video4Linux)
- **Windows**: MSMF (Media Foundation)
- **Cross-platform**: ONNX Runtime (optional)

## Features

- `macos-vision` (default) - Apple Vision API for face/head detection
- `onnx` - ONNX Runtime for cross-platform ML inference

## Examples

```bash
cargo run --release --example head_tracker_demo -p head-tracker
cargo run --release --example vision_test -p head-tracker
cargo run --release --example xtc_integration -p head-tracker
```

## Testing

```bash
cargo check -p head-tracker && cargo clippy -p head-tracker
```

## Notes

- Integrates with the XTC (crosstalk cancellation) plugin for head-tracked spatial audio
- Vision framework requires macOS camera permissions
