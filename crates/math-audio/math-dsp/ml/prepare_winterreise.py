#!/usr/bin/env python3
"""
Prepare Schubert Winterreise Dataset for vocal detector training.

The Schubert Winterreise Dataset contains 24 songs (Lieder) for voice and piano,
with structural annotations identifying instrumental interludes vs. sung sections.

Structure annotations (CSV with ';' delimiter):
    start;end;structure
    1.26;13.4;"I"         <- instrumental (non_vocal)
    13.4;31.68;"A"        <- verse section (vocal)

Convention:
    Sections starting with "I" (I, I1, I2, ...) = instrumental interludes -> non_vocal
    All other sections (A, B, C, A1, Aa1, Ab, B1, C2, ...) = sung -> vocal

Audio: 22050 Hz mono WAV files in 01_RawData/audio_wav/
Only performers with matching audio files are processed (HU33, SC06).

Usage:
    python3 crates/math-audio/math-dsp/ml/prepare_winterreise.py \\
        --winterreise-dir /Volumes/data/Shared/ML/schubert_winterreise_dataseta \\
        --output /Volumes/data/Shared/ML/winterreise_manifest.tsv

    # Use Silero VAD for finer segmentation within sung sections:
    python3 crates/math-audio/math-dsp/ml/prepare_winterreise.py \\
        --winterreise-dir /Volumes/data/Shared/ML/schubert_winterreise_dataseta \\
        --output /Volumes/data/Shared/ML/winterreise_segments_manifest.tsv \\
        --refine-with-silero
"""

import argparse
import csv
import os
import subprocess
import sys
import wave
from concurrent.futures import ProcessPoolExecutor

import numpy as np


DATA_ROOT = "/Volumes/data/Shared/ML"


# ---------------------------------------------------------------------------
# Annotation parsing
# ---------------------------------------------------------------------------

def parse_structure_csv(csv_path: str) -> list[tuple[float, float, str]]:
    """
    Parse a Winterreise structure annotation CSV.

    Format: start;end;structure (semicolon-delimited, quoted structure labels)
    Returns list of (start_sec, end_sec, "vocal"|"non_vocal").
    """
    segments: list[tuple[float, float, str]] = []

    with open(csv_path, encoding="utf-8") as f:
        reader = csv.reader(f, delimiter=";")
        header = next(reader, None)
        if header is None:
            return []

        for row in reader:
            if len(row) < 3:
                continue
            start_sec = float(row[0])
            end_sec = float(row[1])
            structure = row[2].strip().strip('"')

            # Sections starting with "I" are instrumental interludes
            if structure.startswith("I"):
                label = "non_vocal"
            else:
                label = "vocal"

            segments.append((start_sec, end_sec, label))

    return segments


# ---------------------------------------------------------------------------
# Audio utilities
# ---------------------------------------------------------------------------

def _get_wav_duration(path: str) -> float:
    """Get WAV duration in seconds."""
    with wave.open(path, "rb") as wf:
        return wf.getnframes() / wf.getframerate()


def _convert_to_44100(input_path: str, output_path: str) -> bool:
    """Convert WAV to 44.1kHz mono 16-bit using ffmpeg."""
    if os.path.exists(output_path):
        return True
    try:
        subprocess.run(
            [
                "ffmpeg", "-y",
                "-i", input_path,
                "-ar", "44100",
                "-ac", "1",
                "-sample_fmt", "s16",
                output_path,
            ],
            check=True,
            capture_output=True,
            timeout=120,
        )
        return True
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as e:
        if isinstance(e, subprocess.CalledProcessError):
            stderr = e.stderr.decode("utf-8", errors="replace") if e.stderr else ""
            print(f"  ERROR converting {input_path}: {stderr[:200]}")
        else:
            print(f"  ERROR: Timeout converting {input_path}")
        return False


# ---------------------------------------------------------------------------
# Silero VAD refinement
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
        samples = samples[0::n_channels]

    if framerate != 16000:
        n_out = int(len(samples) * 16000 / framerate)
        x_old = np.linspace(0, 1, len(samples))
        x_new = np.linspace(0, 1, n_out)
        samples = np.interp(x_new, x_old, samples).astype(np.float32)

    return samples


def refine_segments_with_silero(
    wav_path: str,
    structure_segments: list[tuple[float, float, str]],
) -> list[tuple[float, float, str]]:
    """
    Refine structural vocal segments using Silero VAD.

    Within each "vocal" structural section, use Silero to find actual
    speech/singing timestamps. Instrumental sections pass through as-is.
    """
    import torch

    model, utils = _get_silero()
    get_speech_timestamps = utils[0]

    samples_16k = _load_wav_16k(wav_path)
    sr = 16000

    refined: list[tuple[float, float, str]] = []

    for start_sec, end_sec, label in structure_segments:
        if label == "non_vocal":
            refined.append((start_sec, end_sec, "non_vocal"))
            continue

        # Extract the section's audio
        start_sample = int(start_sec * sr)
        end_sample = min(int(end_sec * sr), len(samples_16k))
        section = samples_16k[start_sample:end_sample]

        if len(section) < 512:
            refined.append((start_sec, end_sec, "vocal"))
            continue

        wav_tensor = torch.from_numpy(section)
        timestamps = get_speech_timestamps(
            wav_tensor, model, sampling_rate=sr, threshold=0.5
        )

        if not timestamps:
            # Silero found no voice — still mark as vocal (structure says so)
            refined.append((start_sec, end_sec, "vocal"))
            continue

        # Convert Silero timestamps to absolute times and interleave non-vocal gaps
        prev_end = start_sec
        for ts in timestamps:
            voc_start = start_sec + ts["start"] / sr
            voc_end = start_sec + ts["end"] / sr

            if voc_start > prev_end + 0.01:
                refined.append((prev_end, voc_start, "non_vocal"))
            refined.append((voc_start, voc_end, "vocal"))
            prev_end = voc_end

        if prev_end < end_sec - 0.01:
            refined.append((prev_end, end_sec, "non_vocal"))

    return refined


# ---------------------------------------------------------------------------
# Manifest formatting
# ---------------------------------------------------------------------------

def segments_to_manifest_label(
    segments: list[tuple[float, float, str]],
) -> str:
    """Convert segments to manifest format: 'start-end:label,...'."""
    parts: list[str] = []
    for start_sec, end_sec, label in segments:
        parts.append(f"{start_sec:.1f}-{end_sec:.1f}:{label}")
    return ",".join(parts)


# ---------------------------------------------------------------------------
# Per-file processing
# ---------------------------------------------------------------------------

def _process_file(
    args: tuple[str, str, str, bool],
) -> tuple[str, str, str] | None:
    """Process a single Winterreise audio + annotation pair."""
    wav_path, annotation_path, wav_dir, use_silero = args

    try:
        segments = parse_structure_csv(annotation_path)
        if not segments:
            return None

        # Convert to 44.1kHz if needed
        stem = os.path.splitext(os.path.basename(wav_path))[0]
        target_path = os.path.join(wav_dir, f"{stem}.wav")

        with wave.open(wav_path, "rb") as wf:
            sr = wf.getframerate()

        if sr != 44100:
            if not _convert_to_44100(wav_path, target_path):
                return None
            final_path = target_path
        else:
            final_path = wav_path

        if use_silero:
            segments = refine_segments_with_silero(wav_path, segments)

        label = segments_to_manifest_label(segments)
        return (final_path, "segments", label)

    except Exception as e:
        print(f"  WARNING: Failed to process {wav_path}: {e}")
        return None


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Prepare Schubert Winterreise Dataset manifest for vocal detector training"
    )
    parser.add_argument(
        "--winterreise-dir",
        default=os.path.join(DATA_ROOT, "schubert_winterreise_dataseta"),
        help="Path to Winterreise dataset root",
    )
    parser.add_argument(
        "--output",
        default=os.path.join(DATA_ROOT, "winterreise_manifest.tsv"),
        help="Output TSV manifest path",
    )
    parser.add_argument(
        "--refine-with-silero",
        action="store_true",
        help="Use Silero VAD to refine vocal boundaries within structural sections",
    )
    args = parser.parse_args()

    winterreise_dir = args.winterreise_dir
    audio_dir = os.path.join(winterreise_dir, "01_RawData", "audio_wav")
    structure_dir = os.path.join(winterreise_dir, "02_Annotations", "ann_audio_structure")

    if not os.path.isdir(audio_dir):
        print(f"ERROR: Audio directory not found: {audio_dir}")
        sys.exit(1)
    if not os.path.isdir(structure_dir):
        print(f"ERROR: Annotations directory not found: {structure_dir}")
        sys.exit(1)

    # Find matching audio + annotation pairs
    annotation_files = sorted(f for f in os.listdir(structure_dir) if f.endswith(".csv"))

    pairs: list[tuple[str, str]] = []
    for ann_file in annotation_files:
        stem = os.path.splitext(ann_file)[0]  # e.g., Schubert_D911-01_HU33
        wav_file = f"{stem}.wav"
        wav_path = os.path.join(audio_dir, wav_file)
        ann_path = os.path.join(structure_dir, ann_file)

        if os.path.exists(wav_path):
            pairs.append((wav_path, ann_path))

    print(f"Winterreise directory: {winterreise_dir}")
    print(f"Found {len(pairs)} audio+annotation pairs "
          f"(out of {len(annotation_files)} annotations)")

    if not pairs:
        print("ERROR: No matching audio files found for annotations.")
        sys.exit(1)

    # Create WAV output directory for resampled files
    wav_dir = os.path.join(winterreise_dir, "wavs_44100")
    os.makedirs(wav_dir, exist_ok=True)

    # Process files
    task_args = [
        (wav_path, ann_path, wav_dir, args.refine_with_silero)
        for wav_path, ann_path in pairs
    ]

    print(f"\nProcessing {len(pairs)} files...")
    manifest_entries: list[tuple[str, str, str]] = []
    failed = 0

    # Sequential for Silero (model not fork-safe), parallel for structure-only
    if args.refine_with_silero:
        for ta in task_args:
            result = _process_file(ta)
            stem = os.path.splitext(os.path.basename(ta[0]))[0]
            if result is None:
                print(f"  FAILED: {stem}")
                failed += 1
            else:
                manifest_entries.append(result)
                n_vocal = sum(1 for s in result[2].split(",") if s.endswith(":vocal"))
                print(f"  OK: {stem} ({n_vocal} vocal segments)")
    else:
        with ProcessPoolExecutor(max_workers=os.cpu_count()) as pool:
            results = list(pool.map(_process_file, task_args))

        for i, result in enumerate(results):
            stem = os.path.splitext(os.path.basename(pairs[i][0]))[0]
            if result is None:
                print(f"  FAILED: {stem}")
                failed += 1
            else:
                manifest_entries.append(result)
                n_vocal = sum(1 for s in result[2].split(",") if s.endswith(":vocal"))
                n_non_vocal = sum(1 for s in result[2].split(",") if s.endswith(":non_vocal"))
                print(f"  OK: {stem} ({n_vocal} vocal, {n_non_vocal} instrumental)")

    # Write manifest
    with open(args.output, "w", encoding="utf-8") as f:
        for wav_path, label_type, label_value in manifest_entries:
            f.write(f"{wav_path}\t{label_type}\t{label_value}\n")

    # Summary
    print(f"\nManifest written to: {args.output}")
    print(f"  Files processed: {len(manifest_entries)}")
    print(f"  Failed:          {failed}")


if __name__ == "__main__":
    main()
