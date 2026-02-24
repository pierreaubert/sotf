#!/usr/bin/env python3
"""
Train a tiny vocal detector model and export to ONNX.

Extracts MFCC features from WAV files (matching the Rust ml_features.rs implementation
exactly), uses Silero VAD for pseudo-labeling, trains a 3-layer MLP (~1857 params),
and exports to ONNX with sigmoid wrapper.

ONNX contract:
  Input:  "input"  [1, 40] float32 (20 MFCCs + 20 deltas)
  Output: "output" [1, 1] float32 (post-sigmoid probability)

Usage:
    # Demo-only mode (default, uses bundled WAV files + Silero VAD):
    python3 scripts/train_vocal_detector.py --demo-only

    # Train with external dataset manifests (from prepare_musan.py / prepare_ava_speech.py):
    python3 scripts/train_vocal_detector.py --data-dirs musan_manifest.tsv ava_speech_manifest.tsv

    # Combine demo data with external manifests:
    python3 scripts/train_vocal_detector.py --data-dirs musan_manifest.tsv --include-demo

Output:
    crates/plugins/models/vocal_detector.onnx
"""

import argparse
import os
import sys
import time
import wave

import numpy as np
import torch
import torch.nn as nn

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.dirname(SCRIPT_DIR)
AUDIO_DIR = os.path.join(PROJECT_ROOT, "crates", "app-gpui", "assets", "demo-audio")
OUTPUT_DIR = os.path.join(PROJECT_ROOT, "crates", "plugins", "models")
OUTPUT_PATH = os.path.join(OUTPUT_DIR, "vocal_detector.onnx")

AUDIO_FILES = [
    "female_vocal.wav",
    "country.wav",
    "classical.wav",
    "rock.wav",
    "piano.wav",
    "edm.wav",
    "jazz.wav",
]

# ---------------------------------------------------------------------------
# Constants matching Rust ml_features.rs
# ---------------------------------------------------------------------------
NUM_MEL_BANDS = 40
NUM_MFCCS = 20
FEATURE_SIZE = NUM_MFCCS + NUM_MFCCS  # 20 MFCCs + 20 deltas
FFT_SIZE = 2048
HOP_SIZE = FFT_SIZE // 2  # 1024, 50% overlap
SAMPLE_RATE = 44100


# ============================================================================
# MfccExtractor — Python port of ml_features.rs (line-by-line match)
# ============================================================================

def hz_to_mel(f: float) -> float:
    """HTK mel scale: mel(f) = 2595 * log10(1 + f/700)"""
    return 2595.0 * np.log10(1.0 + f / 700.0)


def mel_to_hz(m: float) -> float:
    """Inverse HTK mel scale"""
    return 700.0 * (10.0 ** (m / 2595.0) - 1.0)


class MfccExtractor:
    """
    Python port of ml_features.rs::MfccExtractor.

    Matches the Rust implementation exactly:
    - HTK mel scale, triangular filters with same bin iteration logic
    - Natural log compression with floor 1e-10
    - Unnormalized DCT-II: cos(PI * k * (n + 0.5) / 40)
    - First-order delta (current - previous), zeros for first frame
    """

    def __init__(self, sample_rate: int, fft_size: int):
        spectrum_size = fft_size // 2 + 1
        nyquist = sample_rate / 2.0

        # HTK mel scale
        mel_low = hz_to_mel(0.0)
        mel_high = hz_to_mel(nyquist)

        # Equally spaced mel points (NUM_MEL_BANDS + 2 for triangular filter edges)
        num_points = NUM_MEL_BANDS + 2
        mel_points = np.array([
            mel_low + (mel_high - mel_low) * i / (num_points - 1)
            for i in range(num_points)
        ])
        hz_points = np.array([mel_to_hz(m) for m in mel_points])

        # Convert Hz to FFT bin indices (fractional)
        bin_points = hz_points * fft_size / sample_rate

        # Build sparse triangular filters (same logic as Rust)
        # Store as list of lists of (bin_index, weight) per band
        self.mel_filters: list[list[tuple[int, float]]] = []

        for band in range(NUM_MEL_BANDS):
            left = bin_points[band]
            center = bin_points[band + 1]
            right = bin_points[band + 2]

            bin_start = int(np.floor(left))
            bin_end = min(int(np.ceil(right)), spectrum_size - 1)

            pairs: list[tuple[int, float]] = []
            for b in range(bin_start, bin_end + 1):
                bin_f = float(b)
                if bin_f <= left:
                    weight = 0.0
                elif bin_f <= center:
                    weight = (bin_f - left) / (center - left)
                elif bin_f <= right:
                    weight = (right - bin_f) / (right - center)
                else:
                    weight = 0.0

                if weight > 0.0:
                    pairs.append((b, weight))

            self.mel_filters.append(pairs)

        # Pre-compute DCT-II matrix: dct[k][n] = cos(PI * k * (n + 0.5) / N)
        # This is unnormalized — NOT scipy's ortho-normalized DCT
        self.dct_matrix = np.zeros((NUM_MFCCS, NUM_MEL_BANDS), dtype=np.float64)
        for k in range(NUM_MFCCS):
            for n in range(NUM_MEL_BANDS):
                self.dct_matrix[k, n] = np.cos(
                    np.pi * k * (n + 0.5) / NUM_MEL_BANDS
                )

        # State for delta computation
        self.prev_mfccs = np.zeros(NUM_MFCCS, dtype=np.float64)
        self.has_prev = False

    def compute(self, power_spectrum: np.ndarray) -> np.ndarray:
        """
        Compute MFCC features from a mono power spectrum.

        Args:
            power_spectrum: |X[k]|^2 for k in 0..spectrum_size
                            (already mono-averaged if stereo)

        Returns:
            features: array of shape (FEATURE_SIZE,) = 20 MFCCs + 20 deltas
        """
        # Step 1: Apply mel filterbank (sparse dot products)
        mel_energies = np.zeros(NUM_MEL_BANDS, dtype=np.float64)
        for band_idx, pairs in enumerate(self.mel_filters):
            energy = 0.0
            for (b, w) in pairs:
                energy += power_spectrum[b] * w
            mel_energies[band_idx] = energy

        # Step 2: Log compression (natural log, floor 1e-10)
        log_mel = np.log(mel_energies + 1e-10)

        # Step 3: DCT-II to get MFCCs
        mfccs = self.dct_matrix @ log_mel  # shape (NUM_MFCCS,)

        # Step 4: Delta MFCCs (first-order difference)
        if self.has_prev:
            deltas = mfccs - self.prev_mfccs
        else:
            deltas = np.zeros(NUM_MFCCS, dtype=np.float64)

        # Save for next frame
        self.prev_mfccs = mfccs.copy()
        self.has_prev = True

        features = np.concatenate([mfccs, deltas])
        return features.astype(np.float32)

    def reset(self):
        self.prev_mfccs = np.zeros(NUM_MFCCS, dtype=np.float64)
        self.has_prev = False


# ============================================================================
# WAV loading and feature extraction
# ============================================================================

def load_wav_mono_44100(path: str) -> np.ndarray:
    """Load a WAV file and return mono float32 samples at 44100 Hz."""
    with wave.open(path, "rb") as wf:
        n_channels = wf.getnchannels()
        sampwidth = wf.getsampwidth()
        framerate = wf.getframerate()
        n_frames = wf.getnframes()
        raw = wf.readframes(n_frames)

    if sampwidth == 2:
        samples = np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0
    elif sampwidth == 3:
        # 24-bit: read as bytes, convert manually
        raw_bytes = np.frombuffer(raw, dtype=np.uint8)
        n_samples = len(raw_bytes) // 3
        samples = np.zeros(n_samples, dtype=np.float32)
        for i in range(n_samples):
            b0 = int(raw_bytes[i * 3])
            b1 = int(raw_bytes[i * 3 + 1])
            b2 = int(raw_bytes[i * 3 + 2])
            val = b0 | (b1 << 8) | (b2 << 16)
            if val >= 0x800000:
                val -= 0x1000000
            samples[i] = val / 8388608.0
    elif sampwidth == 4:
        samples = np.frombuffer(raw, dtype=np.int32).astype(np.float32) / 2147483648.0
    else:
        raise ValueError(f"Unsupported sample width: {sampwidth}")

    # Deinterleave to mono
    if n_channels == 2:
        left = samples[0::2]
        right = samples[1::2]
        samples = (left + right) * 0.5
    elif n_channels > 2:
        samples = samples[0::n_channels]

    # Resample if needed (simple case: only support 44100)
    if framerate != SAMPLE_RATE:
        # Basic linear resampling
        ratio = SAMPLE_RATE / framerate
        n_out = int(len(samples) * ratio)
        x_old = np.linspace(0, 1, len(samples))
        x_new = np.linspace(0, 1, n_out)
        samples = np.interp(x_new, x_old, samples).astype(np.float32)

    return samples


def extract_features_from_wav(path: str) -> np.ndarray:
    """
    Extract per-frame MFCC features from a WAV file.

    Windowing matches fft.rs:
    - Hann window: w[n] = 0.5 * (1 - cos(2*pi*n/N)) using N (not N-1)
    - Headroom scale: 1/sqrt(2)
    - Hop size: FFT_SIZE / 2 = 1024
    - FFT: np.fft.rfft (unnormalized, same as RustFFT)

    Returns: array of shape (num_frames, FEATURE_SIZE)
    """
    samples = load_wav_mono_44100(path)

    # Hann window matching Rust: w[n] = 0.5 * (1 - cos(2*pi*n/N)) with N = FFT_SIZE
    window = np.array([
        0.5 * (1.0 - np.cos(2.0 * np.pi * i / FFT_SIZE))
        for i in range(FFT_SIZE)
    ], dtype=np.float64)

    # Headroom scale: 1/sqrt(2) — matches fft.rs
    headroom = 1.0 / np.sqrt(2.0)
    window *= headroom

    extractor = MfccExtractor(SAMPLE_RATE, FFT_SIZE)
    features_list = []

    # Process overlapping frames
    pos = 0
    while pos + FFT_SIZE <= len(samples):
        frame = samples[pos:pos + FFT_SIZE].astype(np.float64)

        # Apply window
        windowed = frame * window

        # Forward FFT (real-to-complex, unnormalized like RustFFT)
        spectrum = np.fft.rfft(windowed)

        # Power spectrum: |X[k]|^2
        # In Rust, compute() averages L+R: 0.5 * (|L|^2 + |R|^2)
        # Since we loaded mono, |mono|^2 is equivalent (the 0.5 factor
        # would cancel if L == R, which is the mono case)
        power = np.abs(spectrum) ** 2

        features = extractor.compute(power)
        features_list.append(features)

        pos += HOP_SIZE

    if not features_list:
        return np.zeros((0, FEATURE_SIZE), dtype=np.float32)

    return np.stack(features_list)


# ============================================================================
# Manifest loading (for external datasets: MUSAN, AVA-Speech, etc.)
# ============================================================================

def labels_for_segments(
    num_frames: int, segment_str: str
) -> np.ndarray:
    """
    Convert a segment label string to per-frame binary labels.

    Segment format: "start-end:label,start-end:label,..."
    where start/end are in seconds and label is "vocal" or "non_vocal".

    Each MFCC frame's center time is mapped to the appropriate segment.
    Frames not covered by any segment default to 0 (non-vocal).
    """
    labels = np.zeros(num_frames, dtype=np.float32)

    segments: list[tuple[float, float, float]] = []
    for part in segment_str.split(","):
        part = part.strip()
        if not part:
            continue
        time_range, label = part.rsplit(":", 1)
        start_str, end_str = time_range.split("-", 1)
        start_sec = float(start_str)
        end_sec = float(end_str)
        label_val = 1.0 if label == "vocal" else 0.0
        segments.append((start_sec, end_sec, label_val))

    # Sort by start time for efficient lookup
    segments.sort(key=lambda x: x[0])

    for i in range(num_frames):
        frame_center_sec = (i * HOP_SIZE + FFT_SIZE / 2) / SAMPLE_RATE
        for start_sec, end_sec, label_val in segments:
            if start_sec <= frame_center_sec <= end_sec:
                labels[i] = label_val
                break

    return labels


def load_manifest(tsv_path: str) -> tuple[np.ndarray, np.ndarray]:
    """
    Load features and labels from a TSV manifest file.

    Manifest format (tab-separated, no header):
        wav_path\\tlabel_type\\tlabel_value

    label_type is "whole_file" or "segments".
    For whole_file: label_value is "vocal" or "non_vocal".
    For segments: label_value is "start-end:label,..." pairs.

    Returns: (features, labels) arrays
    """
    all_features: list[np.ndarray] = []
    all_labels: list[np.ndarray] = []
    total_files = 0
    skipped = 0

    with open(tsv_path, encoding="utf-8") as f:
        lines = [line.strip() for line in f if line.strip()]

    print(f"  Loading {len(lines)} entries from {tsv_path}...")

    for line in lines:
        parts = line.split("\t")
        if len(parts) < 3:
            print(f"  WARNING: Malformed line: {line[:80]}")
            skipped += 1
            continue

        wav_path, label_type, label_value = parts[0], parts[1], parts[2]

        if not os.path.exists(wav_path):
            skipped += 1
            continue

        features = extract_features_from_wav(wav_path)
        if len(features) == 0:
            skipped += 1
            continue

        total_files += 1

        if label_type == "whole_file":
            label_val = 1.0 if label_value == "vocal" else 0.0
            labels = np.full(len(features), label_val, dtype=np.float32)
        elif label_type == "segments":
            labels = labels_for_segments(len(features), label_value)
        else:
            print(f"  WARNING: Unknown label_type '{label_type}', skipping")
            skipped += 1
            continue

        all_features.append(features)
        all_labels.append(labels)

        if total_files % 100 == 0:
            print(f"    Processed {total_files} files...")

    if not all_features:
        return np.zeros((0, FEATURE_SIZE), dtype=np.float32), np.zeros(0, dtype=np.float32)

    features = np.concatenate(all_features)
    labels = np.concatenate(all_labels)

    vocal_frames = int(labels.sum())
    non_vocal_frames = len(labels) - vocal_frames
    print(f"  Loaded {total_files} files: {len(features)} frames "
          f"({vocal_frames} vocal, {non_vocal_frames} non-vocal)")
    if skipped > 0:
        print(f"  Skipped {skipped} entries (missing files or errors)")

    return features, labels


# ============================================================================
# Silero VAD pseudo-labeling
# ============================================================================

def load_wav_16k(path: str) -> torch.Tensor:
    """Load WAV as mono float32 tensor at 16kHz for Silero VAD."""
    samples = load_wav_mono_44100(path)
    # Resample 44100 -> 16000
    ratio = 16000 / SAMPLE_RATE
    n_out = int(len(samples) * ratio)
    x_old = np.linspace(0, 1, len(samples))
    x_new = np.linspace(0, 1, n_out)
    resampled = np.interp(x_new, x_old, samples).astype(np.float32)
    return torch.from_numpy(resampled)


def generate_labels_silero(path: str, num_frames: int) -> np.ndarray:
    """
    Generate frame-level vocal/non-vocal labels using Silero VAD.

    Silero processes at 16kHz with 512-sample chunks (~32ms).
    We map its decisions to our MFCC frame times (hop_size / sample_rate apart).

    Returns: binary labels of shape (num_frames,)
    """
    # Load model
    model, utils = torch.hub.load(
        repo_or_dir="snakers4/silero-vad",
        model="silero_vad",
        force_reload=False,
        onnx=False,
        trust_repo=True,
    )
    (get_speech_timestamps, _, _read_audio, _, _) = utils

    # Load audio ourselves (bypasses torchaudio/torchcodec dependency)
    wav_16k = load_wav_16k(path)

    # Get speech timestamps (in samples at 16kHz)
    speech_timestamps = get_speech_timestamps(
        wav_16k, model, sampling_rate=16000, threshold=0.5
    )

    # Convert speech timestamps to a binary mask at our MFCC frame rate
    # Each MFCC frame center is at (frame_idx * HOP_SIZE + FFT_SIZE/2) / SAMPLE_RATE seconds
    labels = np.zeros(num_frames, dtype=np.float32)

    for ts in speech_timestamps:
        start_sec = ts["start"] / 16000.0
        end_sec = ts["end"] / 16000.0

        for i in range(num_frames):
            frame_center_sec = (i * HOP_SIZE + FFT_SIZE / 2) / SAMPLE_RATE
            if start_sec <= frame_center_sec <= end_sec:
                labels[i] = 1.0

    return labels


# ============================================================================
# PyTorch Model
# ============================================================================

class VocalDetector(nn.Module):
    """
    Tiny MLP for vocal detection.
    Linear(40, 32) -> ReLU -> Linear(32, 16) -> ReLU -> Linear(16, 1)
    1857 parameters total.
    """

    def __init__(self):
        super().__init__()
        self.layers = nn.Sequential(
            nn.Linear(FEATURE_SIZE, 32),   # 40*32 + 32 = 1312
            nn.ReLU(),
            nn.Linear(32, 16),             # 32*16 + 16 = 528
            nn.ReLU(),
            nn.Linear(16, 1),              # 16*1 + 1 = 17
        )                                  # Total: 1857

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.layers(x)


class VocalDetectorWithSigmoid(nn.Module):
    """Wrapper that adds sigmoid for ONNX export (model outputs probability)."""

    def __init__(self, model: VocalDetector):
        super().__init__()
        self.model = model
        self.sigmoid = nn.Sigmoid()

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.sigmoid(self.model(x))


# ============================================================================
# Training
# ============================================================================

def train_model(
    features: np.ndarray,
    labels: np.ndarray,
    num_epochs: int = 100,
    patience: int = 10,
    val_split: float = 0.2,
    lr: float = 1e-3,
    batch_size: int = 64,
) -> VocalDetector:
    """
    Train VocalDetector with BCEWithLogitsLoss, Adam, early stopping.

    Uses stratified 80/20 train/val split.
    """
    # Stratified split
    pos_idx = np.where(labels == 1.0)[0]
    neg_idx = np.where(labels == 0.0)[0]
    np.random.seed(42)
    np.random.shuffle(pos_idx)
    np.random.shuffle(neg_idx)

    n_pos_val = max(1, int(len(pos_idx) * val_split))
    n_neg_val = max(1, int(len(neg_idx) * val_split))

    val_idx = np.concatenate([pos_idx[:n_pos_val], neg_idx[:n_neg_val]])
    train_idx = np.concatenate([pos_idx[n_pos_val:], neg_idx[n_neg_val:]])
    np.random.shuffle(train_idx)
    np.random.shuffle(val_idx)

    X_train = torch.from_numpy(features[train_idx])
    y_train = torch.from_numpy(labels[train_idx]).unsqueeze(1)
    X_val = torch.from_numpy(features[val_idx])
    y_val = torch.from_numpy(labels[val_idx]).unsqueeze(1)

    pos_count = y_train.sum().item()
    neg_count = len(y_train) - pos_count
    print(f"  Train: {len(X_train)} samples ({int(pos_count)} pos, {int(neg_count)} neg)")
    print(f"  Val:   {len(X_val)} samples")

    # Class weight for imbalanced data
    if pos_count > 0 and neg_count > 0:
        pos_weight = torch.tensor([neg_count / pos_count])
    else:
        pos_weight = torch.tensor([1.0])

    model = VocalDetector()
    criterion = nn.BCEWithLogitsLoss(pos_weight=pos_weight)
    optimizer = torch.optim.Adam(model.parameters(), lr=lr)

    best_val_loss = float("inf")
    best_state = None
    patience_counter = 0

    for epoch in range(num_epochs):
        # Training
        model.train()
        perm = torch.randperm(len(X_train))
        train_loss = 0.0
        n_batches = 0

        for i in range(0, len(X_train), batch_size):
            idx = perm[i:i + batch_size]
            xb = X_train[idx]
            yb = y_train[idx]

            optimizer.zero_grad()
            logits = model(xb)
            loss = criterion(logits, yb)
            loss.backward()
            optimizer.step()

            train_loss += loss.item()
            n_batches += 1

        train_loss /= max(n_batches, 1)

        # Validation
        model.eval()
        with torch.no_grad():
            val_logits = model(X_val)
            val_loss = criterion(val_logits, y_val).item()
            val_preds = (torch.sigmoid(val_logits) > 0.5).float()
            val_acc = (val_preds == y_val).float().mean().item()

        if (epoch + 1) % 10 == 0 or epoch == 0:
            print(
                f"  Epoch {epoch+1:3d}/{num_epochs}: "
                f"train_loss={train_loss:.4f}  val_loss={val_loss:.4f}  "
                f"val_acc={val_acc:.3f}"
            )

        # Early stopping
        if val_loss < best_val_loss:
            best_val_loss = val_loss
            best_state = {k: v.clone() for k, v in model.state_dict().items()}
            patience_counter = 0
        else:
            patience_counter += 1
            if patience_counter >= patience:
                print(f"  Early stopping at epoch {epoch+1} (patience={patience})")
                break

    if best_state is not None:
        model.load_state_dict(best_state)

    # Final validation metrics
    model.eval()
    with torch.no_grad():
        val_logits = model(X_val)
        val_preds = (torch.sigmoid(val_logits) > 0.5).float()
        val_acc = (val_preds == y_val).float().mean().item()
        print(f"  Best val accuracy: {val_acc:.3f}")

    return model


# ============================================================================
# ONNX Export
# ============================================================================

def export_onnx(model: VocalDetector, path: str):
    """Export model with sigmoid wrapper to ONNX, validate with onnxruntime."""
    import onnx
    import onnxruntime as ort

    wrapped = VocalDetectorWithSigmoid(model)
    wrapped.eval()

    dummy_input = torch.randn(1, FEATURE_SIZE)

    torch.onnx.export(
        wrapped,
        dummy_input,
        path,
        input_names=["input"],
        output_names=["output"],
        dynamic_axes=None,  # fixed shapes
        opset_version=18,
        do_constant_folding=True,
    )

    # Validate with onnx
    onnx_model = onnx.load(path)
    onnx.checker.check_model(onnx_model)

    # Validate with onnxruntime
    sess = ort.InferenceSession(path)

    inp = sess.get_inputs()[0]
    out = sess.get_outputs()[0]
    assert inp.name == "input", f"Expected input name 'input', got '{inp.name}'"
    assert out.name == "output", f"Expected output name 'output', got '{out.name}'"
    assert inp.shape == [1, FEATURE_SIZE], f"Expected input shape [1, {FEATURE_SIZE}], got {inp.shape}"
    assert out.shape == [1, 1], f"Expected output shape [1, 1], got {out.shape}"

    # Range check: sigmoid output must be in [0, 1]
    test_input = np.random.randn(1, FEATURE_SIZE).astype(np.float32)
    result = sess.run(None, {"input": test_input})[0]
    assert result.shape == (1, 1), f"Output shape mismatch: {result.shape}"
    assert 0.0 <= result[0, 0] <= 1.0, f"Output out of range: {result[0, 0]}"

    # Latency benchmark
    times = []
    for _ in range(100):
        inp_data = np.random.randn(1, FEATURE_SIZE).astype(np.float32)
        t0 = time.perf_counter()
        sess.run(None, {"input": inp_data})
        times.append(time.perf_counter() - t0)

    mean_ms = np.mean(times) * 1000
    p99_ms = np.percentile(times, 99) * 1000

    file_size = os.path.getsize(path)

    print(f"\nONNX Validation:")
    print(f"  Input:   '{inp.name}' {inp.shape}")
    print(f"  Output:  '{out.name}' {out.shape}")
    print(f"  Size:    {file_size:,} bytes ({file_size / 1024:.1f} KB)")
    print(f"  Latency: {mean_ms:.2f} ms mean, {p99_ms:.2f} ms p99 (100 runs)")


# ============================================================================
# Main
# ============================================================================

def load_demo_data() -> tuple[np.ndarray, np.ndarray]:
    """Load features and Silero VAD labels from bundled demo audio files."""
    all_features: list[np.ndarray] = []
    all_labels: list[np.ndarray] = []

    for filename in AUDIO_FILES:
        path = os.path.join(AUDIO_DIR, filename)
        if not os.path.exists(path):
            print(f"  WARNING: {filename} not found, skipping")
            continue

        features = extract_features_from_wav(path)
        if len(features) == 0:
            print(f"  WARNING: {filename} produced 0 frames, skipping")
            continue

        print(f"  {filename}: {len(features)} frames", end="")

        labels = generate_labels_silero(path, len(features))
        vocal_pct = labels.mean() * 100
        print(f" ({vocal_pct:.0f}% vocal)")

        all_features.append(features)
        all_labels.append(labels)

    if not all_features:
        return np.zeros((0, FEATURE_SIZE), dtype=np.float32), np.zeros(0, dtype=np.float32)

    return np.concatenate(all_features), np.concatenate(all_labels)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Train a tiny vocal detector MLP and export to ONNX"
    )
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--demo-only",
        action="store_true",
        help="Train using only bundled demo audio files with Silero VAD labels",
    )
    group.add_argument(
        "--data-dirs",
        nargs="+",
        metavar="TSV",
        help="TSV manifest files from prepare_musan.py / prepare_ava_speech.py",
    )
    parser.add_argument(
        "--include-demo",
        action="store_true",
        help="Also include demo data when using --data-dirs",
    )
    parser.add_argument(
        "--output",
        default=OUTPUT_PATH,
        help=f"Output ONNX path (default: {OUTPUT_PATH})",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()

    print("=" * 60)
    print("Vocal Detector Training Pipeline")
    print("=" * 60)

    all_features: list[np.ndarray] = []
    all_labels: list[np.ndarray] = []

    use_demo = args.demo_only or args.include_demo

    # Step 1: Load data
    step = 1

    if use_demo:
        print(f"\n[{step}/4] Extracting MFCC features from demo audio...")
        demo_features, demo_labels = load_demo_data()
        if len(demo_features) > 0:
            all_features.append(demo_features)
            all_labels.append(demo_labels)
            print(f"  Demo: {len(demo_features)} frames "
                  f"({int(demo_labels.sum())} vocal, "
                  f"{len(demo_labels) - int(demo_labels.sum())} non-vocal)")
        step += 1

    if args.data_dirs:
        print(f"\n[{step}/4] Loading manifest data...")
        for tsv_path in args.data_dirs:
            if not os.path.exists(tsv_path):
                print(f"  ERROR: Manifest not found: {tsv_path}")
                sys.exit(1)
            manifest_features, manifest_labels = load_manifest(tsv_path)
            if len(manifest_features) > 0:
                all_features.append(manifest_features)
                all_labels.append(manifest_labels)
        step += 1

    if not all_features:
        print("ERROR: No data loaded. Aborting.")
        sys.exit(1)

    features = np.concatenate(all_features)
    labels = np.concatenate(all_labels)

    total_frames = len(features)
    total_vocal = int(labels.sum())
    print(f"\n  Total: {total_frames} frames ({total_vocal} vocal, "
          f"{total_frames - total_vocal} non-vocal)")

    # Step 2: Train model
    print(f"\n[{step}/4] Training VocalDetector model...")
    param_count = sum(
        p.numel() for p in VocalDetector().parameters()
    )
    print(f"  Architecture: Linear(40,32)->ReLU->Linear(32,16)->ReLU->Linear(16,1)")
    print(f"  Parameters:   {param_count}")
    step += 1

    model = train_model(features, labels)

    # Step 3: Export and validate
    output_path = args.output
    output_dir = os.path.dirname(output_path)

    print(f"\n[{step}/4] Exporting to ONNX...")
    if output_dir:
        os.makedirs(output_dir, exist_ok=True)
    export_onnx(model, output_path)

    print(f"\nDone! Model saved to: {output_path}")


if __name__ == "__main__":
    main()
