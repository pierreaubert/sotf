#!/usr/bin/env python3
"""
Prepare AVA-Speech dataset for vocal detector training.

Reads the AVA-Speech labels CSV, downloads audio from YouTube via yt-dlp,
and outputs a TSV manifest with per-segment vocal/non-vocal labels.

AVA-Speech labels format (CSV without header):
    video_id, start_sec, end_sec, label

Labels:
    CLEAN_SPEECH        -> vocal
    SPEECH_WITH_MUSIC   -> vocal
    SPEECH_WITH_NOISE   -> vocal
    NO_SPEECH           -> non_vocal

Usage:
    python3 crates/math-audio/math-dsp/ml/prepare_ava_speech.py \\
        --csv /path/to/ava_speech_labels_v1.csv \\
        --output-dir /path/to/ava_wavs \\
        --output ava_speech_manifest.tsv

Download labels:
    wget https://research.google.com/ava/download/ava_speech_labels_v1.csv

Requires:
    pip install yt-dlp
    ffmpeg must be in PATH
"""

import argparse
import csv
import os
import shutil
import subprocess
import sys
from collections import defaultdict


DATA_ROOT = "/Volumes/data/Shared/ML"


# AVA-Speech videos are labeled from 902s to 1798s (15-minute segment starting at 15:02)
AVA_START_SEC = 902.0
AVA_END_SEC = 1798.0

# Label mapping
VOCAL_LABELS = {"CLEAN_SPEECH", "SPEECH_WITH_MUSIC", "SPEECH_WITH_NOISE"}
NON_VOCAL_LABELS = {"NO_SPEECH"}


def parse_ava_csv(csv_path: str) -> dict[str, list[tuple[float, float, str]]]:
    """
    Parse AVA-Speech labels CSV.

    Returns: mapping from video_id -> list of (start_sec, end_sec, label)
    """
    video_segments: dict[str, list[tuple[float, float, str]]] = defaultdict(list)

    with open(csv_path, encoding="utf-8") as f:
        reader = csv.reader(f)
        for row in reader:
            if len(row) < 4:
                continue
            video_id = row[0].strip()
            start_sec = float(row[1].strip())
            end_sec = float(row[2].strip())
            label = row[3].strip()
            video_segments[video_id].append((start_sec, end_sec, label))

    # Sort segments by start time for each video
    for video_id in video_segments:
        video_segments[video_id].sort(key=lambda x: x[0])

    return dict(video_segments)


def map_label(ava_label: str) -> str:
    """Map AVA-Speech label to vocal/non_vocal."""
    if ava_label in VOCAL_LABELS:
        return "vocal"
    if ava_label in NON_VOCAL_LABELS:
        return "non_vocal"
    raise ValueError(f"Unknown AVA-Speech label: {ava_label}")


def download_audio(video_id: str, output_dir: str) -> str | None:
    """
    Download audio from YouTube for a given video ID.

    Returns the path to the downloaded WAV file, or None on failure.
    """
    output_path = os.path.join(output_dir, f"{video_id}.wav")

    # Skip if already downloaded
    if os.path.exists(output_path):
        return output_path

    url = f"https://www.youtube.com/watch?v={video_id}"
    temp_path = os.path.join(output_dir, f"{video_id}_temp")

    try:
        # Download and extract audio as WAV at 44.1kHz mono
        subprocess.run(
            [
                "yt-dlp",
                "--extract-audio",
                "--audio-format", "wav",
                "--postprocessor-args", "ffmpeg:-ar 44100 -ac 1",
                "--output", f"{temp_path}.%(ext)s",
                "--no-playlist",
                "--quiet",
                url,
            ],
            check=True,
            capture_output=True,
            timeout=300,
        )

        # yt-dlp creates the file with .wav extension
        temp_wav = f"{temp_path}.wav"
        if not os.path.exists(temp_wav):
            print(f"  WARNING: yt-dlp did not produce {temp_wav}")
            return None

        # Trim to AVA segment (902s to 1798s) using ffmpeg
        subprocess.run(
            [
                "ffmpeg",
                "-y",
                "-i", temp_wav,
                "-ss", str(AVA_START_SEC),
                "-to", str(AVA_END_SEC),
                "-ar", "44100",
                "-ac", "1",
                "-sample_fmt", "s16",
                output_path,
            ],
            check=True,
            capture_output=True,
            timeout=120,
        )

        # Clean up temp file
        if os.path.exists(temp_wav):
            os.remove(temp_wav)

        return output_path

    except subprocess.CalledProcessError as e:
        stderr = e.stderr.decode("utf-8", errors="replace") if e.stderr else ""
        print(f"  ERROR downloading {video_id}: {stderr[:200]}")
        # Clean up any temp files
        for ext in [".wav", ".webm", ".m4a", ".opus"]:
            temp_file = f"{temp_path}{ext}"
            if os.path.exists(temp_file):
                os.remove(temp_file)
        return None
    except subprocess.TimeoutExpired:
        print(f"  ERROR: Timeout downloading {video_id}")
        return None


def segments_to_manifest_label(
    segments: list[tuple[float, float, str]],
) -> str:
    """
    Convert AVA-Speech segments to manifest segment format.

    Adjusts timestamps relative to AVA_START_SEC (since we trim the audio).

    Returns: comma-separated "start-end:label" string
    """
    parts: list[str] = []
    for start_sec, end_sec, ava_label in segments:
        # Adjust to trimmed audio timeline
        adj_start = start_sec - AVA_START_SEC
        adj_end = end_sec - AVA_START_SEC

        # Skip segments outside our trimmed range
        if adj_end <= 0 or adj_start >= (AVA_END_SEC - AVA_START_SEC):
            continue

        # Clamp to valid range
        adj_start = max(0.0, adj_start)
        adj_end = min(AVA_END_SEC - AVA_START_SEC, adj_end)

        label = map_label(ava_label)
        parts.append(f"{adj_start:.1f}-{adj_end:.1f}:{label}")

    return ",".join(parts)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Prepare AVA-Speech dataset manifest for vocal detector training"
    )
    parser.add_argument(
        "--csv",
        default=os.path.join(DATA_ROOT, "ava", "ava_speech_labels_v1.csv"),
        help="Path to ava_speech_labels_v1.csv",
    )
    parser.add_argument(
        "--output-dir",
        default=os.path.join(DATA_ROOT, "ava", "ava_wavs"),
        help="Directory to store downloaded/trimmed WAV files",
    )
    parser.add_argument(
        "--output",
        default=os.path.join(DATA_ROOT, "ava_speech_manifest.tsv"),
        help="Output TSV manifest path",
    )
    parser.add_argument(
        "--max-videos",
        type=int,
        default=0,
        help="Limit number of videos to download (0 = all, useful for testing)",
    )
    args = parser.parse_args()

    # Validate dependencies
    if shutil.which("yt-dlp") is None:
        print("ERROR: yt-dlp not found in PATH. Install with: pip install yt-dlp")
        sys.exit(1)
    if shutil.which("ffmpeg") is None:
        print("ERROR: ffmpeg not found in PATH.")
        sys.exit(1)

    # Parse labels
    print(f"Parsing AVA-Speech labels from: {args.csv}")
    video_segments = parse_ava_csv(args.csv)
    print(f"  Found {len(video_segments)} videos with labels")

    # Create output directory
    os.makedirs(args.output_dir, exist_ok=True)

    # Process videos
    video_ids = sorted(video_segments.keys())
    if args.max_videos > 0:
        video_ids = video_ids[: args.max_videos]
        print(f"  Limiting to {args.max_videos} videos")

    manifest_entries: list[tuple[str, str, str]] = []
    downloaded = 0
    failed = 0

    print(f"\nDownloading and processing {len(video_ids)} videos...")
    for i, video_id in enumerate(video_ids):
        print(f"  [{i+1}/{len(video_ids)}] {video_id}...", end=" ", flush=True)

        wav_path = download_audio(video_id, args.output_dir)
        if wav_path is None:
            failed += 1
            print("FAILED")
            continue

        segments = video_segments[video_id]
        segment_label = segments_to_manifest_label(segments)

        if segment_label:
            manifest_entries.append((wav_path, "segments", segment_label))
            downloaded += 1
            n_segs = len(segment_label.split(","))
            print(f"OK ({n_segs} segments)")
        else:
            print("SKIPPED (no valid segments)")

    # Write manifest
    with open(args.output, "w", encoding="utf-8") as f:
        for wav_path, label_type, label_value in manifest_entries:
            f.write(f"{wav_path}\t{label_type}\t{label_value}\n")

    # Summary
    print(f"\nManifest written to: {args.output}")
    print(f"  Videos downloaded: {downloaded}")
    print(f"  Videos failed:     {failed}")
    print(f"  Manifest entries:  {len(manifest_entries)}")


if __name__ == "__main__":
    main()
