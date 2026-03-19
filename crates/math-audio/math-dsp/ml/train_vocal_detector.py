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
    python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py --demo-only

    # Train with external dataset manifests (from prepare_musan.py / prepare_ava_speech.py):
    python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py --data-dirs musan_manifest.tsv ava_speech_manifest.tsv

    # Combine demo data with external manifests:
    python3 crates/math-audio/math-dsp/ml/train_vocal_detector.py --data-dirs musan_manifest.tsv --include-demo

Output:
    crates/sotf-plugins/models/vocal_detector.onnx
"""

import argparse
import os
import sys
import time
import wave
from concurrent.futures import ProcessPoolExecutor
from typing import Any

import numpy as np
import scipy.sparse
import torch
import torch.nn as nn

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
# Navigate from crates/math-audio/math-dsp/ml/ up to project root
PROJECT_ROOT = os.path.normpath(os.path.join(SCRIPT_DIR, "..", "..", "..", ".."))
AUDIO_DIR = os.path.join(PROJECT_ROOT, "crates", "app-gpui", "assets", "demo-audio")
OUTPUT_DIR = os.path.join(PROJECT_ROOT, "crates", "sotf-plugins", "models")
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

# Pre-computed Hann window (module-level, matches Rust: w[n] = 0.5*(1 - cos(2*pi*n/N)))
_HANN_WINDOW = 0.5 * (1.0 - np.cos(2.0 * np.pi * np.arange(FFT_SIZE) / FFT_SIZE))
_HANN_WINDOW *= 1.0 / np.sqrt(2.0)  # headroom scale

# ---------------------------------------------------------------------------
# Silero VAD model cache — load once, reuse everywhere
# ---------------------------------------------------------------------------
_silero_cache: tuple[Any, tuple[Any, ...]] | None = None


def _get_silero() -> tuple[Any, tuple[Any, ...]]:
    global _silero_cache
    if _silero_cache is None:
        model, utils = torch.hub.load(
            repo_or_dir="snakers4/silero-vad",
            model="silero_vad",
            force_reload=False,
            onnx=False,
            trust_repo=True,
        )
        _silero_cache = (model, utils)
    return _silero_cache


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

        # Build sparse triangular filters as CSR matrix for vectorized compute()
        rows: list[int] = []
        cols: list[int] = []
        vals: list[float] = []

        for band in range(NUM_MEL_BANDS):
            left = bin_points[band]
            center = bin_points[band + 1]
            right = bin_points[band + 2]

            bin_start = int(np.floor(left))
            bin_end = min(int(np.ceil(right)), spectrum_size - 1)

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
                    rows.append(band)
                    cols.append(b)
                    vals.append(weight)

        self.mel_filter_matrix = scipy.sparse.csr_matrix(
            (vals, (rows, cols)),
            shape=(NUM_MEL_BANDS, spectrum_size),
            dtype=np.float64,
        )

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
        # Step 1: Apply mel filterbank (sparse matrix-vector multiply)
        mel_energies = self.mel_filter_matrix @ power_spectrum

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

    extractor = MfccExtractor(SAMPLE_RATE, FFT_SIZE)
    features_list = []

    # Process overlapping frames
    pos = 0
    while pos + FFT_SIZE <= len(samples):
        frame = samples[pos:pos + FFT_SIZE].astype(np.float64)

        # Apply pre-computed Hann window (module-level)
        windowed = frame * _HANN_WINDOW

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

    if not segments:
        return labels

    # Precompute all frame center times as numpy array
    frame_indices = np.arange(num_frames)
    times = (frame_indices * HOP_SIZE + FFT_SIZE / 2) / SAMPLE_RATE

    # Vectorized: boolean mask per segment
    for start_sec, end_sec, label_val in segments:
        mask = (times >= start_sec) & (times <= end_sec)
        labels[mask] = label_val

    return labels


def _process_manifest_entry(
    entry: tuple[str, str, str],
) -> tuple[np.ndarray, np.ndarray] | None:
    """Process a single manifest entry: extract features + labels. Thread-safe."""
    wav_path, label_type, label_value = entry

    if not os.path.exists(wav_path):
        return None

    features = extract_features_from_wav(wav_path)
    if len(features) == 0:
        return None

    if label_type == "whole_file":
        label_val = 1.0 if label_value == "vocal" else 0.0
        labels = np.full(len(features), label_val, dtype=np.float32)
    elif label_type == "segments":
        labels = labels_for_segments(len(features), label_value)
    else:
        return None

    return (features, labels)


def _parse_manifest_entries(tsv_path: str) -> list[tuple[str, str, str]]:
    """Parse a TSV manifest into (wav_path, label_type, label_value) tuples."""
    entries: list[tuple[str, str, str]] = []
    with open(tsv_path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            parts = line.split("\t")
            if len(parts) < 3:
                print(f"  WARNING: Malformed line: {line[:80]}")
                continue
            entries.append((parts[0], parts[1], parts[2]))
    return entries


def _process_entries(
    entries: list[tuple[str, str, str]], label: str = "",
) -> tuple[np.ndarray, np.ndarray]:
    """Process manifest entries in parallel. Returns (features, labels)."""
    all_features: list[np.ndarray] = []
    all_labels: list[np.ndarray] = []
    total_files = 0
    skipped = 0

    if label:
        print(f"  Processing {len(entries)} {label} files...")
    else:
        print(f"  Processing {len(entries)} files...")

    with ProcessPoolExecutor(max_workers=os.cpu_count()) as pool:
        results = list(pool.map(_process_manifest_entry, entries))

    for result in results:
        if result is None:
            skipped += 1
            continue
        features, labels = result
        all_features.append(features)
        all_labels.append(labels)
        total_files += 1

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


def load_manifest(tsv_path: str) -> tuple[np.ndarray, np.ndarray]:
    """
    Load features and labels from a TSV manifest file.

    Returns: (features, labels) arrays
    """
    entries = _parse_manifest_entries(tsv_path)
    print(f"  Loading {len(entries)} entries from {tsv_path}...")
    return _process_entries(entries)


def load_manifest_with_holdout(
    tsv_path: str, holdout: float,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    """
    Load manifest and split by file into train and holdout sets.

    Returns: (train_features, train_labels, holdout_features, holdout_labels)
    """
    entries = _parse_manifest_entries(tsv_path)
    print(f"  Loading {len(entries)} entries from {tsv_path} (holdout={holdout:.0%})...")

    np.random.seed(42)
    indices = np.arange(len(entries))
    np.random.shuffle(indices)
    n_holdout = max(1, int(len(entries) * holdout))

    holdout_idx = set(indices[:n_holdout].tolist())
    train_entries = [entries[i] for i in range(len(entries)) if i not in holdout_idx]
    holdout_entries = [entries[i] for i in holdout_idx]

    print(f"  Split: {len(train_entries)} train files, {len(holdout_entries)} holdout files")

    train_feat, train_lab = _process_entries(train_entries, "train")
    holdout_feat, holdout_lab = _process_entries(holdout_entries, "holdout")

    return train_feat, train_lab, holdout_feat, holdout_lab


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
    # Use cached model
    model, utils = _get_silero()
    (get_speech_timestamps, _, _read_audio, _, _) = utils

    # Load audio ourselves (bypasses torchaudio/torchcodec dependency)
    wav_16k = load_wav_16k(path)

    # Get speech timestamps (in samples at 16kHz)
    speech_timestamps = get_speech_timestamps(
        wav_16k, model, sampling_rate=16000, threshold=0.5
    )

    # Precompute all frame center times
    frame_indices = np.arange(num_frames)
    times = (frame_indices * HOP_SIZE + FFT_SIZE / 2) / SAMPLE_RATE

    labels = np.zeros(num_frames, dtype=np.float32)

    for ts in speech_timestamps:
        start_sec = ts["start"] / 16000.0
        end_sec = ts["end"] / 16000.0
        mask = (times >= start_sec) & (times <= end_sec)
        labels[mask] = 1.0

    return labels


# ============================================================================
# PyTorch Model
# ============================================================================

class VocalDetector(nn.Module):
    """
    MLP for vocal detection. Architecture configurable via hidden_sizes.
    Default: [128, 64] → Linear(40,128)->ReLU->Linear(128,64)->ReLU->Linear(64,1)
    """

    def __init__(self, hidden_sizes: list[int] | None = None):
        super().__init__()
        if hidden_sizes is None:
            hidden_sizes = [128, 64]

        layers: list[nn.Module] = []
        prev = FEATURE_SIZE
        for h in hidden_sizes:
            layers.append(nn.Linear(prev, h))
            layers.append(nn.ReLU())
            prev = h
        layers.append(nn.Linear(prev, 1))
        self.layers = nn.Sequential(*layers)

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
    hidden_sizes: list[int] | None = None,
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

    # MPS has too much kernel-launch overhead for a 1857-param model.
    # CPU + large batches is faster: the entire dataset fits in cache.
    device = torch.device("cpu")

    # Scale batch size to dataset: fewer Python loop iterations, better vectorization
    effective_batch = max(batch_size, min(len(train_idx) // 64, 8192))
    print(f"  Using device: CPU (batch_size={effective_batch})")

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

    model = VocalDetector(hidden_sizes)
    criterion = nn.BCEWithLogitsLoss(pos_weight=pos_weight)
    optimizer = torch.optim.Adam(model.parameters(), lr=lr)

    best_val_loss = float("inf")
    best_state = None
    patience_counter = 0

    for epoch in range(num_epochs):
        # Training
        model.train()
        perm = torch.randperm(len(X_train))
        epoch_loss = torch.tensor(0.0)
        n_batches = 0

        for i in range(0, len(X_train), effective_batch):
            idx = perm[i:i + effective_batch]
            xb = X_train[idx]
            yb = y_train[idx]

            optimizer.zero_grad()
            logits = model(xb)
            loss = criterion(logits, yb)
            loss.backward()
            optimizer.step()

            epoch_loss += loss.detach()
            n_batches += 1

        train_loss = (epoch_loss / max(n_batches, 1)).item()

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
        dynamic_axes={"input": {0: "batch"}, "output": {0: "batch"}},
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
    assert inp.shape[1] == FEATURE_SIZE, f"Expected input dim 1 = {FEATURE_SIZE}, got {inp.shape}"
    assert out.shape[1] == 1, f"Expected output dim 1 = 1, got {out.shape}"

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

def _process_demo_file(filename: str) -> tuple[str, np.ndarray, np.ndarray] | None:
    """Process a single demo file: extract features + Silero labels. Thread-safe."""
    path = os.path.join(AUDIO_DIR, filename)
    if not os.path.exists(path):
        return None

    features = extract_features_from_wav(path)
    if len(features) == 0:
        return None

    labels = generate_labels_silero(path, len(features))
    return (filename, features, labels)


def load_demo_data() -> tuple[np.ndarray, np.ndarray]:
    """Load features and Silero VAD labels from bundled demo audio files."""
    all_features: list[np.ndarray] = []
    all_labels: list[np.ndarray] = []

    with ProcessPoolExecutor(max_workers=os.cpu_count()) as pool:
        results = list(pool.map(_process_demo_file, AUDIO_FILES))

    for result in results:
        if result is None:
            continue
        filename, features, labels = result
        vocal_pct = labels.mean() * 100
        print(f"  {filename}: {len(features)} frames ({vocal_pct:.0f}% vocal)")
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
    group.add_argument(
        "--eval",
        nargs="+",
        metavar="TSV",
        help="Evaluate an existing ONNX model against manifest TSV files",
    )
    parser.add_argument(
        "--include-demo",
        action="store_true",
        help="Also include demo data when using --data-dirs",
    )
    parser.add_argument(
        "--hidden",
        nargs="+",
        type=int,
        default=[128, 64],
        metavar="N",
        help="Hidden layer sizes (default: 128 64). E.g. --hidden 256 128 64",
    )
    parser.add_argument(
        "--holdout",
        type=float,
        default=0.0,
        metavar="FRAC",
        help="Hold out a fraction of manifest files (by file, not frame) for eval after training (e.g. 0.2)",
    )
    parser.add_argument(
        "--output",
        default=OUTPUT_PATH,
        help=f"Output ONNX path (default: {OUTPUT_PATH})",
    )
    return parser.parse_args()


def evaluate_onnx(onnx_path: str, tsv_paths: list[str]) -> None:
    """Evaluate an ONNX model against manifest TSVs, reporting per-file and aggregate metrics."""
    import onnxruntime as ort

    if not os.path.exists(onnx_path):
        print(f"ERROR: ONNX model not found: {onnx_path}")
        sys.exit(1)

    sess = ort.InferenceSession(onnx_path)
    print(f"Loaded model: {onnx_path}")

    all_preds: list[np.ndarray] = []
    all_labels: list[np.ndarray] = []

    for tsv_path in tsv_paths:
        print(f"\nEvaluating against: {tsv_path}")
        features, labels = load_manifest(tsv_path)
        if len(features) == 0:
            print("  No data loaded, skipping.")
            continue

        # Detect if model supports batched inference
        inp_shape = sess.get_inputs()[0].shape
        dynamic_batch = not isinstance(inp_shape[0], int) or inp_shape[0] != 1

        preds = np.empty(len(features), dtype=np.float32)
        if dynamic_batch:
            chunk_size = 65536
            for i in range(0, len(features), chunk_size):
                chunk = features[i:i + chunk_size]
                out = sess.run(None, {"input": chunk})[0]
                preds[i:i + len(chunk)] = out[:, 0]
        else:
            for i in range(len(features)):
                out = sess.run(None, {"input": features[i:i + 1]})[0]
                preds[i] = out[0, 0]
                if i % 500_000 == 0 and i > 0:
                    print(f"    {i}/{len(features)} frames...")

        binary_preds = (preds > 0.5).astype(np.float32)

        tp = ((binary_preds == 1) & (labels == 1)).sum()
        fp = ((binary_preds == 1) & (labels == 0)).sum()
        fn = ((binary_preds == 0) & (labels == 1)).sum()
        tn = ((binary_preds == 0) & (labels == 0)).sum()

        accuracy = (tp + tn) / max(tp + fp + fn + tn, 1)
        precision = tp / max(tp + fp, 1)
        recall = tp / max(tp + fn, 1)
        f1 = 2 * precision * recall / max(precision + recall, 1e-10)

        print(f"  Frames:    {len(labels)} ({int(labels.sum())} vocal, {int(len(labels) - labels.sum())} non-vocal)")
        print(f"  Accuracy:  {accuracy:.4f}")
        print(f"  Precision: {precision:.4f}")
        print(f"  Recall:    {recall:.4f}")
        print(f"  F1:        {f1:.4f}")
        print(f"  Confusion: TP={tp}  FP={fp}  FN={fn}  TN={tn}")

        all_preds.append(binary_preds)
        all_labels.append(labels)

    if len(all_preds) > 1:
        preds_cat = np.concatenate(all_preds)
        labels_cat = np.concatenate(all_labels)
        tp = ((preds_cat == 1) & (labels_cat == 1)).sum()
        fp = ((preds_cat == 1) & (labels_cat == 0)).sum()
        fn = ((preds_cat == 0) & (labels_cat == 1)).sum()
        tn = ((preds_cat == 0) & (labels_cat == 0)).sum()
        accuracy = (tp + tn) / max(tp + fp + fn + tn, 1)
        precision = tp / max(tp + fp, 1)
        recall = tp / max(tp + fn, 1)
        f1 = 2 * precision * recall / max(precision + recall, 1e-10)
        print(f"\n  AGGREGATE ({len(labels_cat)} frames):")
        print(f"  Accuracy:  {accuracy:.4f}")
        print(f"  Precision: {precision:.4f}")
        print(f"  Recall:    {recall:.4f}")
        print(f"  F1:        {f1:.4f}")


def main() -> None:
    args = parse_args()

    if args.eval:
        print("=" * 60)
        print("Vocal Detector Evaluation")
        print("=" * 60)
        evaluate_onnx(args.output, args.eval)
        return

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

    holdout_features_list: list[np.ndarray] = []
    holdout_labels_list: list[np.ndarray] = []

    if args.data_dirs:
        print(f"\n[{step}/4] Loading manifest data...")
        for tsv_path in args.data_dirs:
            if not os.path.exists(tsv_path):
                print(f"  ERROR: Manifest not found: {tsv_path}")
                sys.exit(1)
            if args.holdout > 0:
                tf, tl, hf, hl = load_manifest_with_holdout(tsv_path, args.holdout)
                if len(tf) > 0:
                    all_features.append(tf)
                    all_labels.append(tl)
                if len(hf) > 0:
                    holdout_features_list.append(hf)
                    holdout_labels_list.append(hl)
            else:
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
    print(f"\n  Total train: {total_frames} frames ({total_vocal} vocal, "
          f"{total_frames - total_vocal} non-vocal)")

    # Step 2: Train model
    hidden = args.hidden
    print(f"\n[{step}/4] Training VocalDetector model...")
    param_count = sum(
        p.numel() for p in VocalDetector(hidden).parameters()
    )
    arch = "->ReLU->".join(
        [f"Linear({a},{b})" for a, b in zip([FEATURE_SIZE] + hidden, hidden + [1])]
    )
    print(f"  Architecture: {arch}")
    print(f"  Parameters:   {param_count}")
    step += 1

    model = train_model(features, labels, hidden_sizes=hidden)

    # Step 3: Export and validate
    output_path = args.output
    output_dir = os.path.dirname(output_path)

    print(f"\n[{step}/4] Exporting to ONNX...")
    if output_dir:
        os.makedirs(output_dir, exist_ok=True)
    export_onnx(model, output_path)

    print(f"\nModel saved to: {output_path}")

    # Step 4 (optional): Evaluate on holdout set
    if holdout_features_list:
        holdout_feat = np.concatenate(holdout_features_list)
        holdout_lab = np.concatenate(holdout_labels_list)

        print(f"\n[Holdout Evaluation]")
        model.eval()
        wrapped = VocalDetectorWithSigmoid(model)
        wrapped.eval()
        with torch.no_grad():
            preds = wrapped(torch.from_numpy(holdout_feat))
            preds = preds[:, 0].numpy()

        binary_preds = (preds > 0.5).astype(np.float32)
        tp = ((binary_preds == 1) & (holdout_lab == 1)).sum()
        fp = ((binary_preds == 1) & (holdout_lab == 0)).sum()
        fn = ((binary_preds == 0) & (holdout_lab == 1)).sum()
        tn = ((binary_preds == 0) & (holdout_lab == 0)).sum()

        accuracy = (tp + tn) / max(tp + fp + fn + tn, 1)
        precision = tp / max(tp + fp, 1)
        recall = tp / max(tp + fn, 1)
        f1 = 2 * precision * recall / max(precision + recall, 1e-10)

        print(f"  Frames:    {len(holdout_lab)} ({int(holdout_lab.sum())} vocal, "
              f"{int(len(holdout_lab) - holdout_lab.sum())} non-vocal)")
        print(f"  Accuracy:  {accuracy:.4f}")
        print(f"  Precision: {precision:.4f}")
        print(f"  Recall:    {recall:.4f}")
        print(f"  F1:        {f1:.4f}")
        print(f"  Confusion: TP={tp}  FP={fp}  FN={fn}  TN={tn}")


if __name__ == "__main__":
    main()
