# ML Vocal Detection — Setup & Training Guide

## Overview

The upmixer uses an optional ML model to detect vocal/dialogue content in real
time, improving center-channel steering over the default heuristic detector.
The model is a small MLP that runs on a dedicated thread via ONNX Runtime,
completely off the audio thread.

```
Audio Thread                    Inference Thread
───────────                     ────────────────
FFT bins ──► MfccExtractor
             │
             ▼
         [5 × 64-feature context] ──► rtrb ring buffer ──► ONNX session
                                                          │
         V_prob (atomic f32) ◄────────────────────────────┘
```

## Architecture

| File              | Role                                                    |
|-------------------|---------------------------------------------------------|
| `ml_features.rs`  | MFCC + spatial feature extractor (320-element context, zero-alloc) |
| `ml_inference.rs` | Async ONNX inference thread (lock-free communication)   |
| `detection.rs`    | Dispatches between ML and heuristic vocal detection     |

### ONNX Contract

- **Input**: `"input"` — shape `[1, 320]` float32 (5 frames × 64 features)
- **Output**: `"output"` — shape `[1, 1]` float32 (post-sigmoid probability, 0.0–1.0)

### Feature Pipeline (must match exactly between Rust and Python)

1. **Windowing**: Hann window `w[n] = 0.5 * (1 - cos(2*pi*n/N))` with `N = fft_size` (not N-1), scaled by `1/sqrt(2)` headroom
2. **FFT**: Real-to-complex, unnormalized (RustFFT / `np.fft.rfft`)
3. **Power spectrum**: `|X[k]|^2` for L/R plus mono average `0.5 * (|L|^2 + |R|^2)`
4. **Mel filterbank**: 40 bands, HTK scale `mel(f) = 2595 * log10(1 + f/700)`, sparse triangular filters
5. **Log compression**: `ln(energy + 1e-10)` — natural log, NOT log10
6. **DCT-II**: Unnormalized `cos(PI * k * (n + 0.5) / 40)` — NOT scipy's ortho-normalized DCT
7. **Deltas**: First-order frame difference (current - previous), zeros for first frame
8. **Spatial/spectral cues**: mid/side energy, L/R balance, correlation, phase coherence, voice-band ratios, centroid, spread, flux, and coarse band energies
9. **Temporal context**: 5 frames flattened oldest-to-newest; startup context is zero-padded
10. **Normalization**: extractor returns raw features; exported ONNX models apply train-set feature standardization internally

## Prerequisites

```bash
# From the project root
source venv/bin/activate
pip install -r crates/math-audio/math-dsp/ml/requirements.txt
```

All dependencies are listed in `crates/math-audio/math-dsp/ml/requirements.txt`.

## Training

### Quick Start (Demo Data)

```bash
source venv/bin/activate
python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py --demo-only
```

This will:
1. Extract temporal MFCC + spatial features from all 7 demo WAV files in `crates/app-gpui/assets/demo-audio/`
2. Generate pseudo-labels using [Silero VAD](https://github.com/snakers4/silero-vad)
3. Train a slightly larger MLP with early stopping
4. Export to ONNX with feature normalization and sigmoid wrappers
5. Validate shapes, range, and latency with ONNX Runtime

Output: `crates/sotf-plugins/models/vocal_detector.onnx`

### Demo Training Data

The pipeline uses the 7 bundled demo audio files:

| File               | Content          | Expected vocal % |
|--------------------|------------------|------------------|
| `female_vocal.wav` | Female vocals    | ~80%             |
| `country.wav`      | Country music    | ~50%             |
| `classical.wav`    | Classical music  | ~80%             |
| `rock.wav`         | Rock music       | ~90%             |
| `piano.wav`        | Piano solo       | ~0%              |
| `edm.wav`          | Electronic dance | ~0%              |
| `jazz.wav`         | Jazz             | ~0%              |

Silero VAD provides frame-level speech/non-speech labels. The training uses an
80/20 stratified split with class-weighted BCE loss, or a file-level holdout
when `--holdout` is used with manifests.

### Training with External Datasets (MUSAN + AVA-Speech)

For better generalization, train with large open-source datasets that have
real annotations instead of Silero VAD pseudo-labels.

#### MUSAN (OpenSLR-17)

~109 hours of audio with directory-level and per-file annotations:
- `speech/` — all vocal (~60h)
- `noise/` — all non-vocal (~6h)
- `music/` — per-file `vocal_activity: yes/no` in ANNOTATIONS files (~42h)

```bash
# Download (~11GB)
wget https://openslr.org/resources/17/musan.tar.gz
tar xzf musan.tar.gz

# Prepare manifest
python3 crates/math-audio/math-dsp/ml/prepare_musan.py --musan-dir /path/to/musan --output musan_manifest.tsv
```

#### AVA-Speech (Google)

~40 hours of movie audio from YouTube with precise per-segment timestamps:
- Labels: `CLEAN_SPEECH`, `SPEECH_WITH_MUSIC`, `SPEECH_WITH_NOISE` (all → vocal), `NO_SPEECH` (→ non-vocal)

```bash
# Download labels CSV
wget https://research.google.com/ava/download/ava_speech_labels_v1.csv

# Prepare manifest (downloads audio from YouTube via yt-dlp)
python3 crates/math-audio/math-dsp/ml/prepare_ava_speech.py \
    --csv ava_speech_labels_v1.csv \
    --output-dir /path/to/ava_wavs \
    --output ava_speech_manifest.tsv

# Use --max-videos N for testing with a subset
python3 crates/math-audio/math-dsp/ml/prepare_ava_speech.py \
    --csv ava_speech_labels_v1.csv \
    --output-dir /path/to/ava_wavs \
    --output ava_speech_manifest.tsv \
    --max-videos 10
```

#### Training with Manifests

```bash
# Train with MUSAN only
python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py --data-dirs musan_manifest.tsv

# Train with both MUSAN and AVA-Speech
python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py --data-dirs musan_manifest.tsv ava_speech_manifest.tsv

# Combine external data with demo data
python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py --data-dirs musan_manifest.tsv --include-demo

# Smaller model if CPU budget is tight
python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py --data-dirs musan_manifest.tsv --hidden 128 64

# File-level holdout for less optimistic validation
python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py --data-dirs musan_manifest.tsv --holdout 0.2

# Evaluate an exported model and sweep for the best F1 threshold
python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py \
    --eval ava_speech_manifest.tsv \
    --output crates/sotf-plugins/models/vocal_detector.onnx \
    --sweep-thresholds
```

When `--threshold` is omitted during evaluation, the script uses the ONNX
`recommended_threshold` metadata when present, then falls back to `0.5`.

#### Manifest TSV Format

Each prepare script outputs a tab-separated file (no header):

```
wav_path\tlabel_type\tlabel_value
```

- `label_type=whole_file`: entire file is `vocal` or `non_vocal`
- `label_type=segments`: comma-separated `start-end:label` pairs in seconds

### Model Architecture

```
Linear(320, 256) → ReLU → Linear(256, 128) → ReLU → Linear(128, 64) → ReLU → Linear(64, 1)
```

- ~123k parameters by default
- Trained with `BCEWithLogitsLoss` (sigmoid applied only at ONNX export)
- Train-set feature mean/std are embedded in the ONNX graph, so the plugin feeds raw realtime features
- Adam optimizer, lr=1e-3, batch_size=64
- Early stopping with patience=10
- Validation sweeps thresholds and stores the best-F1 threshold in ONNX metadata
- The plugin validates ONNX shape plus metadata (`feature_size`, `frame_feature_size`, `context_frames`) before starting inference
- Prefer manifest/file-level holdout metrics over random frame validation
- Inference remains off the audio thread; use the diagnostic CSV to verify control-signal stability

## Enabling in the Plugin

The ML detector is controlled by two plugin parameters:

| Parameter              | Type   | Default | Description                    |
|------------------------|--------|---------|--------------------------------|
| `enable_ml_detection`  | bool   | false   | Toggle ML vs heuristic         |
| `ml_model_path`        | string | ""      | Absolute path to `.onnx` file  |

### From the UI

In the GPUI or TUI interface, find "ML Detection" in the Diagnostic section
of the upmixer plugin and toggle it on. The model path must be set via the
plugin configuration (JSON preset or programmatic API).

### From a Preset (JSON)

```json
{
  "plugin_type": "upmixer",
  "parameters": {
    "speaker_config": "5.1",
    "enable_ml_detection": true,
    "ml_model_path": "/absolute/path/to/vocal_detector.onnx"
  }
}
```

### Programmatic (Rust)

```rust
use sotf_plugins::UpmixerPluginParams;

let params = UpmixerPluginParams {
    enable_ml_detection: true,
    ml_model_path: "crates/sotf-plugins/models/vocal_detector.onnx".into(),
    ..Default::default()
};
```

## Verification

### Verify the ONNX model

```bash
source venv/bin/activate
python3 -c "
import onnxruntime as ort
import numpy as np
s = ort.InferenceSession('crates/sotf-plugins/models/vocal_detector.onnx')
print('Input:', s.get_inputs()[0].name, s.get_inputs()[0].shape)
print('Output:', s.get_outputs()[0].name, s.get_outputs()[0].shape)
print('Metadata:', s.get_modelmeta().custom_metadata_map)
r = s.run(None, {'input': np.zeros((1,320), dtype=np.float32)})[0]
print('Test output:', r[0,0], '(should be 0.0-1.0)')
"
```

### Run Rust tests

```bash
# ML feature extraction tests
cargo test -p sotf-plugin-upmixer --features onnx --lib ml_features

# ML inference tests (requires dummy model in test_data/)
cargo test -p sotf-plugin-upmixer --features onnx --lib ml_inference

# Full upmixer test suite
cargo test -p sotf-plugin-upmixer --no-default-features
```

## Troubleshooting

**Model fails to load at runtime**
- Check the path is absolute or relative to the working directory
- Verify the file exists and is a valid ONNX model
- Ensure it was exported with the current `[1, 320]` feature contract
- Check logs for `"Failed to load ONNX model"` messages
- The plugin falls back to heuristic detection automatically

**Training script fails with `ModuleNotFoundError`**
- Ensure you activated the venv: `source venv/bin/activate`
- Install all deps: `pip install torch onnx onnxruntime onnxscript torchaudio`

**Silero VAD download fails**
- The model is cached in `~/.cache/torch/hub/` after first download
- Requires internet access on first run

**Feature mismatch between Rust and Python**
- The Python `MfccExtractor` class in `train_vocal_detector.py` is a line-by-line
  port of `ml_features.rs`. Do not use `librosa.mfcc()` — it uses different
  defaults (log10, normalized DCT, different window conventions).
