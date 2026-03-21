#!/usr/bin/env python3
"""
Prepare MUSAN dataset (OpenSLR-17) for vocal detector training.

Walks the MUSAN directory structure:
  musan/speech/**/*.wav  -> whole_file:vocal
  musan/noise/**/*.wav   -> whole_file:non_vocal
  musan/music/**/*.wav   -> whole_file:vocal or non_vocal (from ANNOTATIONS)

With --segment, uses Silero VAD to generate per-segment timestamps instead of
whole-file labels. This produces much better training data:
  - music/ vocal files   -> segments with vocal/non_vocal timestamps from Silero
  - music/ non-vocal     -> whole_file:non_vocal (no VAD needed)
  - speech/ files        -> segments with vocal/non_vocal timestamps from Silero
  - noise/ files         -> whole_file:non_vocal (no VAD needed)

Outputs a TSV manifest compatible with train_vocal_detector.py --data-dirs.

Usage:
    # Whole-file labels (original behavior):
    python3 crates/math-audio/math-dsp/ml/prepare_musan.py --musan-dir /path/to/musan --output musan_manifest.tsv

    # Segment-level labels via Silero VAD (recommended):
    python3 crates/math-audio/math-dsp/ml/prepare_musan.py --musan-dir /path/to/musan --output musan_segments_manifest.tsv --segment

Download MUSAN:
    wget https://openslr.org/resources/17/musan.tar.gz
    tar xzf musan.tar.gz
"""

import argparse
import os
import sys
import wave
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np


DATA_ROOT = "/Volumes/data/Shared/ML"


def parse_annotations(annotations_path: str) -> dict[str, bool]:
    """
    Parse a MUSAN ANNOTATIONS file.

    Each file entry looks like:
        file_id: ...
        vocal_activity: yes/no
        ...

    Returns: mapping from file_id -> is_vocal (True/False)
    """
    result: dict[str, bool] = {}
    current_file_id: str | None = None

    with open(annotations_path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                current_file_id = None
                continue

            if line.startswith("file_id:"):
                current_file_id = line.split(":", 1)[1].strip()
            elif line.startswith("vocal_activity:") and current_file_id is not None:
                value = line.split(":", 1)[1].strip().lower()
                result[current_file_id] = value == "yes"

    return result


def find_wav_files(directory: str) -> list[str]:
    """Recursively find all .wav files in a directory."""
    wav_files: list[str] = []
    for root, _dirs, files in os.walk(directory):
        for f in sorted(files):
            if f.lower().endswith(".wav"):
                wav_files.append(os.path.join(root, f))
    return wav_files


# ---------------------------------------------------------------------------
# Silero VAD segmentation
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


def _load_wav_16k(path: str) -> "np.ndarray":
    """Load WAV as mono float32 at 16kHz."""
    with wave.open(path, "rb") as wf:
        n_channels = wf.getnchannels()
        sampwidth = wf.getsampwidth()
        framerate = wf.getframerate()
        n_frames = wf.getnframes()
        raw = wf.readframes(n_frames)

    if sampwidth == 2:
        samples = np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0
    elif sampwidth == 4:
        samples = np.frombuffer(raw, dtype=np.int32).astype(np.float32) / 2147483648.0
    else:
        raise ValueError(f"Unsupported sample width: {sampwidth}")

    if n_channels >= 2:
        samples = samples[0::n_channels]  # take first channel

    # Resample to 16kHz
    if framerate != 16000:
        n_out = int(len(samples) * 16000 / framerate)
        x_old = np.linspace(0, 1, len(samples))
        x_new = np.linspace(0, 1, n_out)
        samples = np.interp(x_new, x_old, samples).astype(np.float32)

    return samples


def _get_wav_duration(path: str) -> float:
    """Get WAV duration in seconds."""
    with wave.open(path, "rb") as wf:
        return wf.getnframes() / wf.getframerate()


def _silero_segment(wav_path: str) -> list[tuple[float, float]]:
    """Run Silero VAD on a file and return vocal segment timestamps."""
    import torch

    model, utils = _get_silero()
    get_speech_timestamps = utils[0]

    samples = _load_wav_16k(wav_path)
    wav_tensor = torch.from_numpy(samples)

    timestamps = get_speech_timestamps(
        wav_tensor, model, sampling_rate=16000, threshold=0.5
    )

    segments: list[tuple[float, float]] = []
    for ts in timestamps:
        start = ts["start"] / 16000.0
        end = ts["end"] / 16000.0
        segments.append((start, end))

    return segments


def _merge_segments(
    segments: list[tuple[float, float]], gap: float = 0.1,
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
    """Convert vocal segments to manifest segment format with interleaved non-vocal gaps."""
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


def _segment_one_file(wav_path: str) -> tuple[str, str, str] | None:
    """Run Silero VAD on a single file and return a manifest entry."""
    try:
        duration = _get_wav_duration(wav_path)
        if duration <= 0:
            return None

        segments = _silero_segment(wav_path)
        merged = _merge_segments(segments)

        if not merged:
            # Silero found no voice — entire file is non-vocal
            return (wav_path, "whole_file", "non_vocal")

        label = _segments_to_label(merged, duration)
        return (wav_path, "segments", label)
    except Exception as e:
        print(f"  WARNING: Failed to segment {wav_path}: {e}")
        return None


def _segment_files_parallel(
    wav_files: list[str], desc: str,
) -> list[tuple[str, str, str]]:
    """Run Silero VAD on multiple files in parallel."""
    entries: list[tuple[str, str, str]] = []

    print(f"  Segmenting {len(wav_files)} {desc} files with Silero VAD...")

    with ProcessPoolExecutor(max_workers=os.cpu_count()) as pool:
        results = list(pool.map(_segment_one_file, wav_files))

    segment_count = 0
    for result in results:
        if result is not None:
            entries.append(result)
            if result[1] == "segments":
                segment_count += 1

    print(f"  {segment_count} files with vocal segments, "
          f"{len(entries) - segment_count} whole-file non-vocal")

    return entries


# ---------------------------------------------------------------------------
# Directory processors
# ---------------------------------------------------------------------------

def process_speech_dir(speech_dir: str, segment: bool) -> list[tuple[str, str, str]]:
    """Speech files are vocal. With --segment, use Silero to find pauses."""
    wav_files = find_wav_files(speech_dir)
    if segment:
        return _segment_files_parallel(wav_files, "speech")
    return [(wav, "whole_file", "vocal") for wav in wav_files]


def process_noise_dir(noise_dir: str) -> list[tuple[str, str, str]]:
    """All noise files are non-vocal (no segmentation needed)."""
    return [(wav, "whole_file", "non_vocal") for wav in find_wav_files(noise_dir)]


def process_music_dir(music_dir: str, segment: bool) -> list[tuple[str, str, str]]:
    """
    Music files use ANNOTATIONS for vocal_activity label.
    With --segment, runs Silero VAD on vocal music files.
    """
    entries: list[tuple[str, str, str]] = []
    vocal_wav_files: list[str] = []
    skipped = 0

    for group_name in sorted(os.listdir(music_dir)):
        group_path = os.path.join(music_dir, group_name)
        if not os.path.isdir(group_path):
            continue

        annotations_path = os.path.join(group_path, "ANNOTATIONS")
        if not os.path.exists(annotations_path):
            wav_count = len(find_wav_files(group_path))
            if wav_count > 0:
                print(f"  WARNING: No ANNOTATIONS in {group_name}, skipping {wav_count} files")
                skipped += wav_count
            continue

        vocal_map = parse_annotations(annotations_path)

        for wav_path in find_wav_files(group_path):
            file_id = Path(wav_path).stem

            if file_id in vocal_map:
                if vocal_map[file_id]:
                    if segment:
                        vocal_wav_files.append(wav_path)
                    else:
                        entries.append((wav_path, "whole_file", "vocal"))
                else:
                    entries.append((wav_path, "whole_file", "non_vocal"))
            else:
                print(f"  WARNING: No annotation for {file_id} in {group_name}")
                skipped += 1

    if segment and vocal_wav_files:
        segmented = _segment_files_parallel(vocal_wav_files, "vocal music")
        entries.extend(segmented)

    if skipped > 0:
        print(f"  Skipped {skipped} music files without annotations")

    return entries


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Prepare MUSAN dataset manifest for vocal detector training"
    )
    parser.add_argument(
        "--musan-dir",
        default=os.path.join(DATA_ROOT, "musan"),
        help="Path to extracted MUSAN root directory (contains speech/, music/, noise/)",
    )
    parser.add_argument(
        "--output",
        default=os.path.join(DATA_ROOT, "musan_manifest.tsv"),
        help="Output TSV manifest path",
    )
    parser.add_argument(
        "--segment",
        action="store_true",
        help="Use Silero VAD to generate segment-level labels instead of whole-file labels",
    )
    args = parser.parse_args()

    musan_dir = args.musan_dir
    output_path = args.output

    # Validate directory structure
    speech_dir = os.path.join(musan_dir, "speech")
    music_dir = os.path.join(musan_dir, "music")
    noise_dir = os.path.join(musan_dir, "noise")

    missing = []
    if not os.path.isdir(speech_dir):
        missing.append("speech/")
    if not os.path.isdir(music_dir):
        missing.append("music/")
    if not os.path.isdir(noise_dir):
        missing.append("noise/")

    if missing:
        print(f"ERROR: Missing directories in {musan_dir}: {', '.join(missing)}")
        print("Expected structure: musan/{speech,music,noise}/")
        sys.exit(1)

    mode = "segment-level (Silero VAD)" if args.segment else "whole-file"
    print(f"Preparing MUSAN manifest ({mode})...")
    all_entries: list[tuple[str, str, str]] = []

    # Process each subdirectory
    print(f"\n[1/3] Processing speech/ ({'segmented' if args.segment else 'all vocal'})...")
    speech_entries = process_speech_dir(speech_dir, args.segment)
    print(f"  Found {len(speech_entries)} speech files")
    all_entries.extend(speech_entries)

    print("\n[2/3] Processing noise/ (all non-vocal)...")
    noise_entries = process_noise_dir(noise_dir)
    print(f"  Found {len(noise_entries)} noise files")
    all_entries.extend(noise_entries)

    print(f"\n[3/3] Processing music/ ({'segmented' if args.segment else 'from ANNOTATIONS'})...")
    music_entries = process_music_dir(music_dir, args.segment)
    vocal_music = sum(1 for _, t, _ in music_entries if t == "whole_file" and _ == "vocal")
    non_vocal_music = sum(1 for _, t, l in music_entries if l == "non_vocal" and t == "whole_file")
    segmented_music = sum(1 for _, t, _ in music_entries if t == "segments")
    print(f"  Found {len(music_entries)} music files", end="")
    if segmented_music:
        print(f" ({segmented_music} segmented, {non_vocal_music} non-vocal)")
    else:
        print(f" ({vocal_music} vocal, {non_vocal_music} non-vocal)")
    all_entries.extend(music_entries)

    # Write manifest
    with open(output_path, "w", encoding="utf-8") as f:
        for wav_path, label_type, label_value in all_entries:
            f.write(f"{wav_path}\t{label_type}\t{label_value}\n")

    # Summary
    total_segments = sum(1 for _, t, _ in all_entries if t == "segments")
    total_whole_vocal = sum(1 for _, t, l in all_entries if t == "whole_file" and l == "vocal")
    total_non_vocal = sum(1 for _, _, l in all_entries if l == "non_vocal" and _ == "whole_file")
    print(f"\nManifest written to: {output_path}")
    print(f"  Total: {len(all_entries)} files")
    if total_segments:
        print(f"  Segmented: {total_segments} files")
    if total_whole_vocal:
        print(f"  Whole-file vocal: {total_whole_vocal} files")
    print(f"  Whole-file non-vocal: {total_non_vocal} files")


if __name__ == "__main__":
    main()
