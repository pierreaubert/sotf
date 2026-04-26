#!/usr/bin/env python3
"""
Train a small vocal/dialog detector model and export to ONNX.

Extracts temporal MFCC + spatial features from WAV files (matching the Rust
ml_features.rs implementation), uses Silero VAD for pseudo-labeling, trains
a small MLP, and exports to ONNX with sigmoid wrapper.

ONNX contract:
  Input:  "input"  [1, 320] float32 (5 frames × 64 features)
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
import io
import os
import sys
import time
import wave
from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass
from typing import Any

import numpy as np
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
NUM_AUX_FEATURES = 24
FRAME_FEATURE_SIZE = NUM_MFCCS + NUM_MFCCS + NUM_AUX_FEATURES
CONTEXT_FRAMES = 5
FEATURE_SIZE = FRAME_FEATURE_SIZE * CONTEXT_FRAMES
FFT_SIZE = 2048
HOP_SIZE = FFT_SIZE // 2  # 1024, 50% overlap
SAMPLE_RATE = 48000

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

    Per frame:
    - 20 MFCCs
    - 20 MFCC deltas
    - 24 spatial/spectral features

    Output:
    - Flattened 5-frame context, oldest to newest: shape (320,)
    """

    def __init__(self, sample_rate: int, fft_size: int):
        self.sample_rate = sample_rate
        self.fft_size = fft_size
        self.spectrum_size = fft_size // 2 + 1
        nyquist = sample_rate / 2.0

        mel_low = hz_to_mel(0.0)
        mel_high = hz_to_mel(nyquist)
        num_points = NUM_MEL_BANDS + 2
        mel_points = np.array([
            mel_low + (mel_high - mel_low) * i / (num_points - 1)
            for i in range(num_points)
        ])
        hz_points = np.array([mel_to_hz(m) for m in mel_points])
        bin_points = hz_points * fft_size / sample_rate

        self.filter_weights: list[tuple[int, float]] = []
        self.mel_filters: list[tuple[int, int]] = []
        for band in range(NUM_MEL_BANDS):
            left = bin_points[band]
            center = bin_points[band + 1]
            right = bin_points[band + 2]
            bin_start = int(np.floor(left))
            bin_end = min(int(np.ceil(right)), self.spectrum_size - 1)
            offset = len(self.filter_weights)
            count = 0

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
                    self.filter_weights.append((b, weight))
                    count += 1
            self.mel_filters.append((offset, count))

        self.dct_matrix = np.zeros((NUM_MFCCS, NUM_MEL_BANDS), dtype=np.float64)
        for k in range(NUM_MFCCS):
            for n in range(NUM_MEL_BANDS):
                self.dct_matrix[k, n] = np.cos(
                    np.pi * k * (n + 0.5) / NUM_MEL_BANDS
                )

        self.prev_mfccs = np.zeros(NUM_MFCCS, dtype=np.float64)
        self.prev_power = np.zeros(self.spectrum_size, dtype=np.float64)
        self.context = np.zeros(FEATURE_SIZE, dtype=np.float32)
        self.has_prev = False

    def compute(self, left_spectrum: np.ndarray, right_spectrum: np.ndarray) -> np.ndarray:
        left_power = np.abs(left_spectrum) ** 2
        right_power = np.abs(right_spectrum) ** 2
        mono_power = (left_power + right_power) * 0.5

        mel_energies = np.zeros(NUM_MEL_BANDS, dtype=np.float64)
        for band, (offset, count) in enumerate(self.mel_filters):
            energy = 0.0
            for bin_idx, weight in self.filter_weights[offset:offset + count]:
                energy += mono_power[bin_idx] * weight
            mel_energies[band] = energy
        log_mel = np.log(mel_energies + 1e-10)
        mfccs = self.dct_matrix @ log_mel

        if self.has_prev:
            deltas = mfccs - self.prev_mfccs
        else:
            deltas = np.zeros(NUM_MFCCS, dtype=np.float64)
        self.prev_mfccs = mfccs.copy()

        aux = self.compute_aux_features(left_spectrum, right_spectrum, mono_power)
        frame = np.concatenate([mfccs, deltas, aux]).astype(np.float32)

        self.context[:-FRAME_FEATURE_SIZE] = self.context[FRAME_FEATURE_SIZE:]
        self.context[-FRAME_FEATURE_SIZE:] = frame
        self.has_prev = True
        return self.context.copy()

    def compute_aux_features(
        self,
        left_spectrum: np.ndarray,
        right_spectrum: np.ndarray,
        mono_power: np.ndarray,
    ) -> np.ndarray:
        eps = 1e-12
        freq_per_bin = self.sample_rate / self.fft_size
        nyquist = self.sample_rate * 0.5
        freqs = np.arange(self.spectrum_size, dtype=np.float64) * freq_per_bin

        left_power = np.abs(left_spectrum) ** 2
        right_power = np.abs(right_spectrum) ** 2
        mid = (left_spectrum + right_spectrum) * 0.5
        side = (left_spectrum - right_spectrum) * 0.5
        mid_power = np.abs(mid) ** 2
        side_power = np.abs(side) ** 2

        left_energy = float(left_power.sum())
        right_energy = float(right_power.sum())
        mid_energy = float(mid_power.sum())
        side_energy = float(side_power.sum())
        mono_total = float(mono_power.sum())
        total_energy = left_energy + right_energy

        voice = (freqs >= 200.0) & (freqs <= 5000.0)
        voice_energy = float(mono_power[voice].sum())
        voice_mid_energy = float(mid_power[voice].sum())
        voice_side_energy = float(side_power[voice].sum())
        voice_left_energy = float(left_power[voice].sum())
        voice_right_energy = float(right_power[voice].sum())

        cross = np.sum(left_spectrum * np.conj(right_spectrum))
        voice_cross = np.sum(left_spectrum[voice] * np.conj(right_spectrum[voice]))
        energy_root = np.sqrt(left_energy * right_energy)
        voice_energy_root = np.sqrt(voice_left_energy * voice_right_energy)
        correlation = float(np.real(cross) / energy_root) if energy_root > eps else 0.0
        phase_coherence = float(np.abs(cross) / energy_root) if energy_root > eps else 0.0
        voice_correlation = (
            float(np.real(voice_cross) / voice_energy_root) if voice_energy_root > eps else 0.0
        )
        voice_phase_coherence = (
            float(np.abs(voice_cross) / voice_energy_root) if voice_energy_root > eps else 0.0
        )

        centroid = float((freqs * mono_power).sum() / mono_total) if mono_total > eps else 0.0
        spread = (
            float(np.sqrt((((freqs - centroid) ** 2) * mono_power).sum() / mono_total))
            if mono_total > eps
            else 0.0
        )
        flux = (
            float(np.maximum(mono_power - self.prev_power, 0.0).sum() / (mono_total + eps))
            if self.has_prev
            else 0.0
        )
        self.prev_power = mono_power.copy()

        def band_ratio(lo: float, hi: float | None) -> float:
            if hi is None:
                mask = freqs > lo
            else:
                mask = (freqs > lo) & (freqs <= hi)
            return float(mono_power[mask].sum() / (mono_total + eps))

        aux = np.zeros(NUM_AUX_FEATURES, dtype=np.float64)
        aux[0] = np.log(mono_total + eps)
        aux[1] = np.log(mid_energy + eps)
        aux[2] = np.log(side_energy + eps)
        aux[3] = mid_energy / (mid_energy + side_energy + eps)
        aux[4] = side_energy / (mid_energy + side_energy + eps)
        aux[5] = (left_energy - right_energy) / (total_energy + eps)
        aux[6] = 1.0 - abs(left_energy - right_energy) / (total_energy + eps)
        aux[7] = np.clip(correlation, -1.0, 1.0)
        aux[8] = np.clip(phase_coherence, 0.0, 1.0)
        aux[9] = voice_energy / (mono_total + eps)
        aux[10] = voice_mid_energy / (voice_mid_energy + voice_side_energy + eps)
        aux[11] = voice_side_energy / (voice_mid_energy + voice_side_energy + eps)
        aux[12] = (voice_left_energy - voice_right_energy) / (
            voice_left_energy + voice_right_energy + eps
        )
        aux[13] = 1.0 - abs(voice_left_energy - voice_right_energy) / (
            voice_left_energy + voice_right_energy + eps
        )
        aux[14] = np.clip(voice_correlation, -1.0, 1.0)
        aux[15] = np.clip(voice_phase_coherence, 0.0, 1.0)
        aux[16] = centroid / max(nyquist, 1.0)
        aux[17] = spread / max(nyquist, 1.0)
        aux[18] = flux
        aux[19] = float(mono_power[freqs <= 250.0].sum() / (mono_total + eps))
        aux[20] = band_ratio(250.0, 500.0)
        aux[21] = band_ratio(500.0, 2000.0)
        aux[22] = band_ratio(2000.0, 5000.0)
        aux[23] = band_ratio(5000.0, None)
        return aux

    def reset(self):
        self.prev_mfccs.fill(0.0)
        self.prev_power.fill(0.0)
        self.context.fill(0.0)
        self.has_prev = False


# ============================================================================
# WAV loading and feature extraction
# ============================================================================

def _open_wave_tolerant(path: str) -> wave.Wave_read:
    """Open WAV files, including files with leading ID3 metadata before RIFF."""
    with open(path, "rb") as f:
        data = f.read()
    if not data.startswith(b"RIFF"):
        riff_start = data.find(b"RIFF")
        if riff_start < 0:
            raise ValueError(f"No RIFF header found in {path}")
        data = data[riff_start:]
    return wave.open(io.BytesIO(data), "rb")


def load_wav_stereo_model_rate(path: str) -> tuple[np.ndarray, np.ndarray]:
    """Load a WAV file and return stereo float32 samples at SAMPLE_RATE."""
    with _open_wave_tolerant(path) as wf:
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

    if n_channels == 1:
        left = samples
        right = samples
    elif n_channels == 2:
        left = samples[0::2]
        right = samples[1::2]
    elif n_channels > 2:
        left = samples[0::n_channels]
        right = samples[1::n_channels]

    if framerate != SAMPLE_RATE:
        ratio = SAMPLE_RATE / framerate
        n_out = int(len(left) * ratio)
        x_old = np.linspace(0, 1, len(left))
        x_new = np.linspace(0, 1, n_out)
        left = np.interp(x_new, x_old, left).astype(np.float32)
        right = np.interp(x_new, x_old, right).astype(np.float32)

    return left.astype(np.float32), right.astype(np.float32)


def load_wav_mono_model_rate(path: str) -> np.ndarray:
    left, right = load_wav_stereo_model_rate(path)
    return (left + right) * 0.5


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
    left, right = load_wav_stereo_model_rate(path)

    extractor = MfccExtractor(SAMPLE_RATE, FFT_SIZE)
    features_list = []

    # Process overlapping frames
    pos = 0
    while pos + FFT_SIZE <= len(left):
        left_frame = left[pos:pos + FFT_SIZE].astype(np.float64)
        right_frame = right[pos:pos + FFT_SIZE].astype(np.float64)

        # Apply pre-computed Hann window (module-level)
        left_windowed = left_frame * _HANN_WINDOW
        right_windowed = right_frame * _HANN_WINDOW

        # Forward FFT (real-to-complex, unnormalized like RustFFT)
        left_spectrum = np.fft.rfft(left_windowed)
        right_spectrum = np.fft.rfft(right_windowed)

        features = extractor.compute(left_spectrum, right_spectrum)
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
    samples = load_wav_mono_model_rate(path)
    # Resample model rate -> 16k
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

@dataclass
class BinaryMetrics:
    threshold: float
    accuracy: float
    precision: float
    recall: float
    f1: float
    tp: int
    fp: int
    fn: int
    tn: int


@dataclass
class TrainingResult:
    model: "VocalDetector"
    feature_mean: np.ndarray
    feature_std: np.ndarray
    threshold: float
    val_metrics: BinaryMetrics


def binary_metrics(
    preds: np.ndarray,
    labels: np.ndarray,
    threshold: float = 0.5,
) -> BinaryMetrics:
    binary_preds = (preds >= threshold).astype(np.float32)
    tp = int(((binary_preds == 1) & (labels == 1)).sum())
    fp = int(((binary_preds == 1) & (labels == 0)).sum())
    fn = int(((binary_preds == 0) & (labels == 1)).sum())
    tn = int(((binary_preds == 0) & (labels == 0)).sum())

    accuracy = (tp + tn) / max(tp + fp + fn + tn, 1)
    precision = tp / max(tp + fp, 1)
    recall = tp / max(tp + fn, 1)
    f1 = 2 * precision * recall / max(precision + recall, 1e-10)
    return BinaryMetrics(threshold, accuracy, precision, recall, f1, tp, fp, fn, tn)


def best_f1_threshold(
    preds: np.ndarray,
    labels: np.ndarray,
    min_threshold: float = 0.05,
    max_threshold: float = 0.95,
    steps: int = 91,
) -> BinaryMetrics:
    thresholds = np.linspace(min_threshold, max_threshold, steps)
    best = binary_metrics(preds, labels, threshold=0.5)
    for threshold in thresholds:
        metrics = binary_metrics(preds, labels, threshold=float(threshold))
        if metrics.f1 > best.f1:
            best = metrics
    return best

class VocalDetector(nn.Module):
    """
    MLP for vocal detection. Architecture configurable via hidden_sizes.
    Default: [256, 128, 64].
    """

    def __init__(self, hidden_sizes: list[int] | None = None):
        super().__init__()
        if hidden_sizes is None:
            hidden_sizes = [256, 128, 64]

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
    """Wrapper that normalizes raw features and adds sigmoid for ONNX export."""

    def __init__(
        self,
        model: VocalDetector,
        feature_mean: np.ndarray | None = None,
        feature_std: np.ndarray | None = None,
    ):
        super().__init__()
        self.model = model
        self.sigmoid = nn.Sigmoid()
        if feature_mean is None:
            feature_mean = np.zeros(FEATURE_SIZE, dtype=np.float32)
        if feature_std is None:
            feature_std = np.ones(FEATURE_SIZE, dtype=np.float32)
        self.register_buffer("feature_mean", torch.from_numpy(feature_mean.astype(np.float32)))
        self.register_buffer("feature_std", torch.from_numpy(feature_std.astype(np.float32)))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = (x - self.feature_mean) / self.feature_std
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
) -> TrainingResult:
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

    # MPS has too much kernel-launch overhead for this small MLP.
    # CPU + large batches is faster: the entire dataset fits in cache.
    device = torch.device("cpu")

    # Scale batch size to dataset: fewer Python loop iterations, better vectorization
    effective_batch = max(batch_size, min(len(train_idx) // 64, 8192))
    print(f"  Using device: CPU (batch_size={effective_batch})")

    feature_mean = features[train_idx].mean(axis=0).astype(np.float32)
    feature_std = features[train_idx].std(axis=0).astype(np.float32)
    feature_std = np.maximum(feature_std, 1e-4).astype(np.float32)

    features_norm = ((features - feature_mean) / feature_std).astype(np.float32)
    print(
        "  Feature normalization: "
        f"mean range [{feature_mean.min():.3f}, {feature_mean.max():.3f}], "
        f"std range [{feature_std.min():.3f}, {feature_std.max():.3f}]"
    )

    X_train = torch.from_numpy(features_norm[train_idx])
    y_train = torch.from_numpy(labels[train_idx]).unsqueeze(1)
    X_val = torch.from_numpy(features_norm[val_idx])
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
        val_probs = torch.sigmoid(val_logits)[:, 0].numpy()
        val_labels = y_val[:, 0].numpy()
        metrics_05 = binary_metrics(val_probs, val_labels, threshold=0.5)
        best_metrics = best_f1_threshold(val_probs, val_labels)
        print(f"  Best val accuracy @0.50: {metrics_05.accuracy:.3f}")
        print(
            f"  Best val F1: threshold={best_metrics.threshold:.2f} "
            f"F1={best_metrics.f1:.3f} precision={best_metrics.precision:.3f} "
            f"recall={best_metrics.recall:.3f}"
        )

    return TrainingResult(
        model=model,
        feature_mean=feature_mean,
        feature_std=feature_std,
        threshold=best_metrics.threshold,
        val_metrics=best_metrics,
    )


# ============================================================================
# ONNX Export
# ============================================================================

def export_onnx(result: TrainingResult, path: str):
    """Export model with sigmoid wrapper to ONNX, validate with onnxruntime."""
    import onnx
    import onnxruntime as ort

    wrapped = VocalDetectorWithSigmoid(
        result.model,
        feature_mean=result.feature_mean,
        feature_std=result.feature_std,
    )
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

    onnx_model = onnx.load(path)
    metadata = {
        "feature_size": str(FEATURE_SIZE),
        "frame_feature_size": str(FRAME_FEATURE_SIZE),
        "context_frames": str(CONTEXT_FRAMES),
        "sample_rate": str(SAMPLE_RATE),
        "fft_size": str(FFT_SIZE),
        "hop_size": str(HOP_SIZE),
        "recommended_threshold": f"{result.threshold:.6f}",
        "validation_f1": f"{result.val_metrics.f1:.6f}",
        "validation_precision": f"{result.val_metrics.precision:.6f}",
        "validation_recall": f"{result.val_metrics.recall:.6f}",
        "normalization": "input = (raw_features - feature_mean) / feature_std",
    }
    for key, value in metadata.items():
        prop = onnx_model.metadata_props.add()
        prop.key = key
        prop.value = value
    onnx.save(onnx_model, path)

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
    print(f"  Threshold: {result.threshold:.2f} recommended by validation F1 sweep")


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
        description="Train a small vocal/dialog detector MLP and export to ONNX"
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
        default=[256, 128, 64],
        metavar="N",
        help="Hidden layer sizes (default: 256 128 64). Use --hidden 128 64 for a smaller model.",
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
    parser.add_argument(
        "--threshold",
        type=float,
        default=None,
        metavar="P",
        help="Probability threshold for --eval metrics (default: ONNX metadata, then 0.5)",
    )
    parser.add_argument(
        "--sweep-thresholds",
        action="store_true",
        help="During --eval, also report the best F1 threshold on each eval set",
    )
    return parser.parse_args()


def evaluate_onnx(
    onnx_path: str,
    tsv_paths: list[str],
    threshold: float | None = None,
    sweep_thresholds: bool = False,
) -> None:
    """Evaluate an ONNX model against manifest TSVs, reporting per-file and aggregate metrics."""
    import onnxruntime as ort

    if not os.path.exists(onnx_path):
        print(f"ERROR: ONNX model not found: {onnx_path}")
        sys.exit(1)

    sess = ort.InferenceSession(onnx_path)
    print(f"Loaded model: {onnx_path}")
    eval_threshold = threshold
    if eval_threshold is None:
        metadata = sess.get_modelmeta().custom_metadata_map
        if "recommended_threshold" in metadata:
            eval_threshold = float(metadata["recommended_threshold"])
            print(f"Using metadata threshold: {eval_threshold:.2f}")
        else:
            eval_threshold = 0.5
            print("Using default threshold: 0.50")

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

        metrics = binary_metrics(preds, labels, threshold=eval_threshold)

        print(f"  Frames:    {len(labels)} ({int(labels.sum())} vocal, {int(len(labels) - labels.sum())} non-vocal)")
        print(f"  Threshold: {metrics.threshold:.2f}")
        print(f"  Accuracy:  {metrics.accuracy:.4f}")
        print(f"  Precision: {metrics.precision:.4f}")
        print(f"  Recall:    {metrics.recall:.4f}")
        print(f"  F1:        {metrics.f1:.4f}")
        print(f"  Confusion: TP={metrics.tp}  FP={metrics.fp}  "
              f"FN={metrics.fn}  TN={metrics.tn}")
        if sweep_thresholds:
            best = best_f1_threshold(preds, labels)
            print(
                f"  Best F1:   threshold={best.threshold:.2f} F1={best.f1:.4f} "
                f"precision={best.precision:.4f} recall={best.recall:.4f}"
            )

        all_preds.append(preds)
        all_labels.append(labels)

    if len(all_preds) > 1:
        preds_cat = np.concatenate(all_preds)
        labels_cat = np.concatenate(all_labels)
        metrics = binary_metrics(preds_cat, labels_cat, threshold=eval_threshold)
        print(f"\n  AGGREGATE ({len(labels_cat)} frames):")
        print(f"  Threshold: {metrics.threshold:.2f}")
        print(f"  Accuracy:  {metrics.accuracy:.4f}")
        print(f"  Precision: {metrics.precision:.4f}")
        print(f"  Recall:    {metrics.recall:.4f}")
        print(f"  F1:        {metrics.f1:.4f}")
        if sweep_thresholds:
            best = best_f1_threshold(preds_cat, labels_cat)
            print(
                f"  Best F1:   threshold={best.threshold:.2f} F1={best.f1:.4f} "
                f"precision={best.precision:.4f} recall={best.recall:.4f}"
            )


def main() -> None:
    args = parse_args()

    if args.eval:
        print("=" * 60)
        print("Vocal Detector Evaluation")
        print("=" * 60)
        evaluate_onnx(
            args.output,
            args.eval,
            threshold=args.threshold,
            sweep_thresholds=args.sweep_thresholds,
        )
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
        print(f"\n[{step}/4] Extracting temporal spatial features from demo audio...")
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

    result = train_model(features, labels, hidden_sizes=hidden)

    # Step 3: Export and validate
    output_path = args.output
    output_dir = os.path.dirname(output_path)

    print(f"\n[{step}/4] Exporting to ONNX...")
    if output_dir:
        os.makedirs(output_dir, exist_ok=True)
    export_onnx(result, output_path)

    print(f"\nModel saved to: {output_path}")

    # Step 4 (optional): Evaluate on holdout set
    if holdout_features_list:
        holdout_feat = np.concatenate(holdout_features_list)
        holdout_lab = np.concatenate(holdout_labels_list)

        print(f"\n[Holdout Evaluation]")
        result.model.eval()
        wrapped = VocalDetectorWithSigmoid(
            result.model,
            feature_mean=result.feature_mean,
            feature_std=result.feature_std,
        )
        wrapped.eval()
        with torch.no_grad():
            preds = wrapped(torch.from_numpy(holdout_feat))
            preds = preds[:, 0].numpy()

        metrics = binary_metrics(preds, holdout_lab, threshold=result.threshold)

        print(f"  Frames:    {len(holdout_lab)} ({int(holdout_lab.sum())} vocal, "
              f"{int(len(holdout_lab) - holdout_lab.sum())} non-vocal)")
        print(f"  Threshold: {metrics.threshold:.2f}")
        print(f"  Accuracy:  {metrics.accuracy:.4f}")
        print(f"  Precision: {metrics.precision:.4f}")
        print(f"  Recall:    {metrics.recall:.4f}")
        print(f"  F1:        {metrics.f1:.4f}")
        print(f"  Confusion: TP={metrics.tp}  FP={metrics.fp}  "
              f"FN={metrics.fn}  TN={metrics.tn}")


if __name__ == "__main__":
    main()
