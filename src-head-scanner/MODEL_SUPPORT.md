# ONNX Model Support

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
