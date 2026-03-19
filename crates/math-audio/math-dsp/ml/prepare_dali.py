#!/usr/bin/env python3
"""
Prepare DALI dataset for vocal detector training.

DALI (Dataset of synchronised Audio, LyrIcs and vocal notes) contains ~5300
songs with note-level vocal annotations. Audio is downloaded from YouTube.

Each annotation .gz file is a gzip-compressed pickle containing an object
with .annotations["annot"]["notes"] — a list of dicts with "time": [start, end].
Any frame covered by a note = vocal, everything else = non-vocal.

The DALI_DATA_INFO.gz file maps DALI IDs to YouTube URLs.

Usage:
    # First, download DALI_v1.0.zip from https://zenodo.org/record/2577915
    # and extract it to get the annotation .gz files.

    python3 crates/math-audio/math-dsp/ml/prepare_dali.py \\
        --annotations-dir /path/to/DALI_v1.0/ \\
        --output-dir /path/to/dali_wavs \\
        --output dali_manifest.tsv

    # Limit number of songs (for testing):
    python3 crates/math-audio/math-dsp/ml/prepare_dali.py \\
        --annotations-dir /path/to/DALI_v1.0/ \\
        --output-dir /path/to/dali_wavs \\
        --output dali_manifest.tsv \\
        --max-songs 100

Requires:
    pip install yt-dlp
    ffmpeg must be in PATH
"""

import argparse
import glob
import gzip
import os
import pickle
import shutil
import subprocess
import sys
from concurrent.futures import ProcessPoolExecutor


DATA_ROOT = "/Volumes/data/Shared/ML"


def load_annotations(gz_path: str) -> list[tuple[float, float]]:
    """
    Load vocal note time ranges from a DALI annotation .gz file.

    Returns list of (start_sec, end_sec) tuples for vocal regions.
    """
    with gzip.open(gz_path, "rb") as f:
        data = pickle.load(f)

    # Navigate the annotation structure
    try:
        annots = data.annotations["annot"]
    except (AttributeError, KeyError, TypeError):
        # Some files may have a different structure
        if isinstance(data, dict):
            annots = data.get("annotations", {}).get("annot", {})
        else:
            return []

    # Use "notes" granularity — finest level, each note has time range
    # Fall back to "words" or "lines" if notes not available
    for level in ("notes", "words", "lines", "paragraphs"):
        entries = annots.get(level, [])
        if entries:
            break
    else:
        return []

    segments: list[tuple[float, float]] = []
    for entry in entries:
        try:
            t = entry["time"]
            start = float(t[0])
            end = float(t[1])
            if end > start:
                segments.append((start, end))
        except (KeyError, TypeError, IndexError):
            continue

    return segments


def load_dali_info(annotations_dir: str) -> dict[str, str]:
    """
    Load DALI_DATA_INFO.gz to get YouTube URLs for each DALI ID.

    Returns mapping of DALI_ID -> YouTube URL.
    """
    info_path = os.path.join(annotations_dir, "DALI_DATA_INFO.gz")
    if not os.path.exists(info_path):
        # Search for it
        candidates = glob.glob(os.path.join(annotations_dir, "**", "DALI_DATA_INFO.gz"), recursive=True)
        if candidates:
            info_path = candidates[0]
        else:
            return {}

    with gzip.open(info_path, "rb") as f:
        info = pickle.load(f)

    url_map: dict[str, str] = {}
    if isinstance(info, dict):
        for dali_id, entry in info.items():
            yt_url = None
            if isinstance(entry, dict):
                # Could be {"YOUTUBE": "url", ...} or {"youtube": "url", ...}
                for key in ("YOUTUBE", "youtube", "url", "URL"):
                    if key in entry:
                        yt_url = entry[key]
                        break
            if yt_url:
                if not yt_url.startswith("http"):
                    yt_url = f"https://www.youtube.com/watch?v={yt_url}"
                url_map[str(dali_id)] = yt_url

    return url_map


def merge_segments(segments: list[tuple[float, float]]) -> list[tuple[float, float]]:
    """Merge overlapping/adjacent vocal segments to reduce manifest size."""
    if not segments:
        return []
    segments.sort()
    merged: list[tuple[float, float]] = [segments[0]]
    for start, end in segments[1:]:
        if start <= merged[-1][1] + 0.05:  # merge if gap < 50ms
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return merged


def segments_to_manifest_label(
    vocal_segments: list[tuple[float, float]], duration: float,
) -> str:
    """
    Convert vocal segments to manifest format with interleaved non-vocal gaps.

    Returns: "start-end:label,..." string
    """
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


def download_audio(youtube_url: str, output_path: str) -> bool:
    """Download audio from YouTube as 44.1kHz mono WAV."""
    if os.path.exists(output_path):
        return True

    temp_path = output_path + ".tmp"

    try:
        subprocess.run(
            [
                "yt-dlp",
                "--extract-audio",
                "--audio-format", "wav",
                "--postprocessor-args", "ffmpeg:-ar 44100 -ac 1 -sample_fmt s16",
                "--output", f"{temp_path}.%(ext)s",
                "--no-playlist",
                "--quiet",
                youtube_url,
            ],
            check=True,
            capture_output=True,
            timeout=300,
        )

        temp_wav = f"{temp_path}.wav"
        if os.path.exists(temp_wav):
            os.rename(temp_wav, output_path)
            return True
        return False

    except (subprocess.CalledProcessError, subprocess.TimeoutExpired):
        # Clean up temp files
        for ext in (".wav", ".webm", ".m4a", ".opus"):
            tmp = f"{temp_path}{ext}"
            if os.path.exists(tmp):
                os.remove(tmp)
        return False


def get_wav_duration(wav_path: str) -> float:
    """Get duration of a WAV file in seconds using ffprobe."""
    try:
        result = subprocess.run(
            [
                "ffprobe", "-v", "quiet",
                "-show_entries", "format=duration",
                "-of", "csv=p=0",
                wav_path,
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
        return float(result.stdout.strip())
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, ValueError):
        return 0.0


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Prepare DALI dataset manifest for vocal detector training"
    )
    parser.add_argument(
        "--annotations-dir",
        default=os.path.join(DATA_ROOT, "dali", "DALI_v1.0"),
        help="Path to extracted DALI annotation .gz files",
    )
    parser.add_argument(
        "--output-dir",
        default=os.path.join(DATA_ROOT, "dali", "wavs"),
        help="Directory to store downloaded WAV files",
    )
    parser.add_argument(
        "--output",
        default=os.path.join(DATA_ROOT, "dali_manifest.tsv"),
        help="Output TSV manifest path",
    )
    parser.add_argument(
        "--max-songs",
        type=int,
        default=0,
        help="Limit number of songs to process (0 = all)",
    )
    parser.add_argument(
        "--skip-download",
        action="store_true",
        help="Skip downloading, only process already-downloaded WAVs",
    )
    args = parser.parse_args()

    # Validate dependencies
    if not args.skip_download:
        if shutil.which("yt-dlp") is None:
            print("ERROR: yt-dlp not found in PATH. Install with: pip install yt-dlp")
            sys.exit(1)
    if shutil.which("ffprobe") is None:
        print("ERROR: ffprobe not found in PATH (comes with ffmpeg).")
        sys.exit(1)

    # Find annotation files
    gz_files = sorted(glob.glob(os.path.join(args.annotations_dir, "*.gz")))
    # Exclude the info file
    gz_files = [f for f in gz_files if "DALI_DATA_INFO" not in os.path.basename(f)]

    if not gz_files:
        # Try subdirectories
        gz_files = sorted(glob.glob(os.path.join(args.annotations_dir, "**", "*.gz"), recursive=True))
        gz_files = [f for f in gz_files if "DALI_DATA_INFO" not in os.path.basename(f)]

    print(f"Found {len(gz_files)} annotation files in {args.annotations_dir}")

    if not gz_files:
        print("ERROR: No annotation .gz files found.")
        sys.exit(1)

    # Load YouTube URL mapping
    url_map = load_dali_info(args.annotations_dir)
    print(f"Loaded {len(url_map)} YouTube URLs from DALI_DATA_INFO.gz")

    if not url_map and not args.skip_download:
        print("WARNING: No YouTube URLs found. Will only process existing WAVs.")
        args.skip_download = True

    # Limit songs if requested
    if args.max_songs > 0:
        gz_files = gz_files[:args.max_songs]
        print(f"Limiting to {args.max_songs} songs")

    os.makedirs(args.output_dir, exist_ok=True)

    manifest_entries: list[tuple[str, str, str]] = []
    processed = 0
    skipped_no_url = 0
    skipped_download = 0
    skipped_no_segments = 0

    print(f"\nProcessing {len(gz_files)} songs...")
    for i, gz_path in enumerate(gz_files):
        dali_id = os.path.splitext(os.path.basename(gz_path))[0]
        # Strip .gz if double extension
        if dali_id.endswith(".gz"):
            dali_id = dali_id[:-3]

        print(f"  [{i+1}/{len(gz_files)}] {dali_id}...", end=" ", flush=True)

        # Load annotations
        segments = load_annotations(gz_path)
        if not segments:
            print("SKIP (no vocal segments)")
            skipped_no_segments += 1
            continue

        # Download audio
        wav_path = os.path.join(args.output_dir, f"{dali_id}.wav")

        if not os.path.exists(wav_path):
            if args.skip_download:
                print("SKIP (no WAV)")
                skipped_download += 1
                continue

            yt_url = url_map.get(dali_id)
            if not yt_url:
                print("SKIP (no YouTube URL)")
                skipped_no_url += 1
                continue

            if not download_audio(yt_url, wav_path):
                print("FAILED (download)")
                skipped_download += 1
                continue

        # Get duration for trailing non-vocal segment
        duration = get_wav_duration(wav_path)
        if duration <= 0:
            print("SKIP (bad WAV)")
            skipped_download += 1
            continue

        # Merge overlapping segments and create manifest label
        merged = merge_segments(segments)
        segment_label = segments_to_manifest_label(merged, duration)

        manifest_entries.append((wav_path, "segments", segment_label))
        processed += 1

        vocal_secs = sum(e - s for s, e in merged)
        print(f"OK ({len(merged)} vocal regions, {vocal_secs:.0f}s / {duration:.0f}s)")

    # Write manifest
    with open(args.output, "w", encoding="utf-8") as f:
        for wav_path, label_type, label_value in manifest_entries:
            f.write(f"{wav_path}\t{label_type}\t{label_value}\n")

    # Summary
    print(f"\nManifest written to: {args.output}")
    print(f"  Songs processed:    {processed}")
    print(f"  Skipped (no URL):   {skipped_no_url}")
    print(f"  Skipped (download): {skipped_download}")
    print(f"  Skipped (no vocal): {skipped_no_segments}")
    print(f"  Manifest entries:   {len(manifest_entries)}")


if __name__ == "__main__":
    main()
