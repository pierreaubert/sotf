#!/usr/bin/env python3
"""
Prepare MUSDB18 dataset for vocal detector training.

MUSDB18 provides multi-track music with isolated stems:
    mixture.wav  — full mix (input for training)
    vocals.wav   — isolated vocal stem (ground truth for labels)
    bass.wav, drums.wav, other.wav — other stems

For each song, we analyze the vocals.wav stem to determine where vocals
are active (energy above threshold), then use mixture.wav as the audio
to extract features from.

Directory structure:
    musdb18/
        train/
            Song Name/
                mixture.wav
                vocals.wav
                ...
        test/
            Song Name/
                ...

Usage:
    python3 crates/math-audio/math-dsp/ml/prepare_musdb18.py \\
        --musdb-dir /Volumes/data/Shared/ML/musdb18 \\
        --output /Volumes/data/Shared/ML/musdb18_manifest.tsv

    # Use Silero VAD on vocals stem for finer segmentation:
    python3 crates/math-audio/math-dsp/ml/prepare_musdb18.py \\
        --musdb-dir /Volumes/data/Shared/ML/musdb18 \\
        --output /Volumes/data/Shared/ML/musdb18_segments_manifest.tsv \\
        --method silero

    # Only include test split:
    python3 crates/math-audio/math-dsp/ml/prepare_musdb18.py \\
        --musdb-dir /Volumes/data/Shared/ML/musdb18 \\
        --split test \\
        --output /Volumes/data/Shared/ML/musdb18_test_manifest.tsv
"""

import argparse
import os
import sys
import wave
from concurrent.futures import ProcessPoolExecutor

import numpy as np


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

DATA_ROOT = "/Volumes/data/Shared/ML"

# RMS energy threshold (linear) to consider a frame as having vocals.
# Vocals stem in MUSDB18 has near-zero energy when silent.
ENERGY_THRESHOLD = 0.005

# Frame size for energy analysis (in samples at native rate)
ENERGY_FRAME_SIZE = 2048
ENERGY_HOP_SIZE = 1024


# ---------------------------------------------------------------------------
# Audio utilities
# ---------------------------------------------------------------------------

def _load_wav_mono(path: str) -> tuple[np.ndarray, int]:
    """Load WAV as mono float32, return (samples, sample_rate)."""
    with wave.open(path, "rb") as wf:
        n_channels = wf.getnchannels()
        sampwidth = wf.getsampwidth()
        framerate = wf.getframerate()
        n_frames = wf.getnframes()
        raw = wf.readframes(n_frames)

    if sampwidth == 2:
        samples = np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0
    elif sampwidth == 3:
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

    if n_channels == 2:
        left = samples[0::2]
        right = samples[1::2]
        samples = (left + right) * 0.5
    elif n_channels > 2:
        samples = samples[0::n_channels]

    return samples, framerate


def _get_wav_duration(path: str) -> float:
    """Get WAV duration in seconds."""
    with wave.open(path, "rb") as wf:
        return wf.getnframes() / wf.getframerate()


# ---------------------------------------------------------------------------
# Energy-based vocal detection on isolated stem
# ---------------------------------------------------------------------------

def _energy_segments(
    vocals_path: str, threshold: float = ENERGY_THRESHOLD,
) -> tuple[list[tuple[float, float]], float]:
    """
    Detect vocal segments from isolated vocals stem using RMS energy.

    Returns (vocal_segments, duration).
    """
    samples, sr = _load_wav_mono(vocals_path)
    duration = len(samples) / sr

    # Compute per-frame RMS energy
    frame_energies: list[tuple[float, float]] = []  # (center_time, rms)
    pos = 0
    while pos + ENERGY_FRAME_SIZE <= len(samples):
        frame = samples[pos:pos + ENERGY_FRAME_SIZE]
        rms = np.sqrt(np.mean(frame ** 2))
        center_time = (pos + ENERGY_FRAME_SIZE / 2) / sr
        frame_energies.append((center_time, rms))
        pos += ENERGY_HOP_SIZE

    if not frame_energies:
        return [], duration

    # Find contiguous vocal regions
    segments: list[tuple[float, float]] = []
    in_vocal = False
    seg_start = 0.0

    for center_time, rms in frame_energies:
        frame_start = center_time - (ENERGY_FRAME_SIZE / 2) / sr
        frame_end = center_time + (ENERGY_FRAME_SIZE / 2) / sr

        if rms >= threshold:
            if not in_vocal:
                seg_start = frame_start
                in_vocal = True
        else:
            if in_vocal:
                segments.append((seg_start, frame_end))
                in_vocal = False

    if in_vocal:
        segments.append((seg_start, duration))

    return _merge_segments(segments, gap=0.3), duration


# ---------------------------------------------------------------------------
# Silero VAD on vocals stem
# ---------------------------------------------------------------------------

_silero_cache = None


def _get_silero():
    """Load Silero VAD model once per process."""
    global _silero_cache
    if _silero_cache is None:
        import torch
        model, utils = torch.hub.load(
            repo_or_dir="snakers4/silero-vad",
            model="silero_vad",
            force_reload=False,
            onnx=False,
            trust_repo=True,
        )
        _silero_cache = (model, utils)
    return _silero_cache


def _silero_segments(vocals_path: str) -> tuple[list[tuple[float, float]], float]:
    """Detect vocal segments from isolated vocals stem using Silero VAD."""
    import torch

    samples, sr = _load_wav_mono(vocals_path)
    duration = len(samples) / sr

    # Resample to 16kHz for Silero
    if sr != 16000:
        n_out = int(len(samples) * 16000 / sr)
        x_old = np.linspace(0, 1, len(samples))
        x_new = np.linspace(0, 1, n_out)
        samples_16k = np.interp(x_new, x_old, samples).astype(np.float32)
    else:
        samples_16k = samples

    model, utils = _get_silero()
    get_speech_timestamps = utils[0]

    wav_tensor = torch.from_numpy(samples_16k)
    timestamps = get_speech_timestamps(
        wav_tensor, model, sampling_rate=16000, threshold=0.5
    )

    segments: list[tuple[float, float]] = []
    for ts in timestamps:
        start = ts["start"] / 16000.0
        end = ts["end"] / 16000.0
        segments.append((start, end))

    return _merge_segments(segments, gap=0.1), duration


# ---------------------------------------------------------------------------
# Segment utilities
# ---------------------------------------------------------------------------

def _merge_segments(
    segments: list[tuple[float, float]], gap: float = 0.3,
) -> list[tuple[float, float]]:
    """Merge segments with gaps smaller than threshold."""
    if not segments:
        return []
    segments.sort()
    merged: list[tuple[float, float]] = [segments[0]]
    for start, end in segments[1:]:
        if start <= merged[-1][1] + gap:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return merged


def _segments_to_label(
    vocal_segments: list[tuple[float, float]], duration: float,
) -> str:
    """Convert vocal segments to manifest format with interleaved non-vocal gaps."""
    parts: list[str] = []
    prev_end = 0.0

    for start, end in vocal_segments:
        if start > prev_end + 0.01:
            parts.append(f"{prev_end:.1f}-{start:.1f}:non_vocal")
        parts.append(f"{start:.1f}-{end:.1f}:vocal")
        prev_end = end

    if prev_end < duration - 0.01:
        parts.append(f"{prev_end:.1f}-{duration:.1f}:non_vocal")

    return ",".join(parts)


# ---------------------------------------------------------------------------
# Per-song processing
# ---------------------------------------------------------------------------

def _process_song(args: tuple[str, str]) -> tuple[str, str, str] | None:
    """Process a single MUSDB18 song directory. Returns manifest entry."""
    song_dir, method = args
    mixture_path = os.path.join(song_dir, "mixture.wav")
    vocals_path = os.path.join(song_dir, "vocals.wav")

    if not os.path.exists(mixture_path) or not os.path.exists(vocals_path):
        return None

    try:
        if method == "silero":
            segments, duration = _silero_segments(vocals_path)
        else:
            segments, duration = _energy_segments(vocals_path)

        if not segments:
            # No vocals detected — entire file is non-vocal
            return (mixture_path, "whole_file", "non_vocal")

        label = _segments_to_label(segments, duration)
        return (mixture_path, "segments", label)
    except Exception as e:
        print(f"  WARNING: Failed to process {song_dir}: {e}")
        return None


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Prepare MUSDB18 dataset manifest for vocal detector training"
    )
    parser.add_argument(
        "--musdb-dir",
        default=os.path.join(DATA_ROOT, "musdb18"),
        help="Path to MUSDB18 root directory (contains train/, test/)",
    )
    parser.add_argument(
        "--output",
        default=os.path.join(DATA_ROOT, "musdb18_manifest.tsv"),
        help="Output TSV manifest path",
    )
    parser.add_argument(
        "--split",
        choices=["all", "train", "test"],
        default="all",
        help="Which split to include (default: all)",
    )
    parser.add_argument(
        "--method",
        choices=["energy", "silero"],
        default="energy",
        help="Vocal detection method on the vocals stem (default: energy)",
    )
    args = parser.parse_args()

    musdb_dir = args.musdb_dir

    # Validate directory structure
    train_dir = os.path.join(musdb_dir, "train")
    test_dir = os.path.join(musdb_dir, "test")

    if not os.path.isdir(train_dir) and not os.path.isdir(test_dir):
        print(f"ERROR: Neither train/ nor test/ found in {musdb_dir}")
        sys.exit(1)

    # Collect song directories
    song_dirs: list[str] = []
    if args.split in ("all", "train") and os.path.isdir(train_dir):
        for name in sorted(os.listdir(train_dir)):
            d = os.path.join(train_dir, name)
            if os.path.isdir(d):
                song_dirs.append(d)
    if args.split in ("all", "test") and os.path.isdir(test_dir):
        for name in sorted(os.listdir(test_dir)):
            d = os.path.join(test_dir, name)
            if os.path.isdir(d):
                song_dirs.append(d)

    print(f"MUSDB18 directory: {musdb_dir}")
    print(f"Found {len(song_dirs)} songs (split={args.split})")
    print(f"Detection method: {args.method}")

    # Process songs
    task_args = [(d, args.method) for d in song_dirs]

    print(f"\nProcessing {len(song_dirs)} songs...")
    with ProcessPoolExecutor(max_workers=os.cpu_count()) as pool:
        results = list(pool.map(_process_song, task_args))

    manifest_entries: list[tuple[str, str, str]] = []
    failed = 0
    for i, result in enumerate(results):
        song_name = os.path.basename(song_dirs[i])
        if result is None:
            print(f"  FAILED: {song_name}")
            failed += 1
        else:
            manifest_entries.append(result)
            if result[1] == "segments":
                n_segs = len([s for s in result[2].split(",") if "vocal" in s and "non_vocal" not in s])
                print(f"  OK: {song_name} ({n_segs} vocal segments)")
            else:
                print(f"  OK: {song_name} (no vocals)")

    # Write manifest
    with open(args.output, "w", encoding="utf-8") as f:
        for wav_path, label_type, label_value in manifest_entries:
            f.write(f"{wav_path}\t{label_type}\t{label_value}\n")

    # Summary
    segmented = sum(1 for _, t, _ in manifest_entries if t == "segments")
    non_vocal = sum(1 for _, t, l in manifest_entries if t == "whole_file" and l == "non_vocal")
    print(f"\nManifest written to: {args.output}")
    print(f"  Songs processed: {len(manifest_entries)}")
    print(f"  With vocals:     {segmented}")
    print(f"  No vocals:       {non_vocal}")
    print(f"  Failed:          {failed}")


if __name__ == "__main__":
    main()
