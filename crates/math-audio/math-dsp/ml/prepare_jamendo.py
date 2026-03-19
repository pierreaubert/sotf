#!/usr/bin/env python3
"""
Prepare Jamendo Corpus for vocal detector training.

Downloads the Jamendo Corpus from Zenodo (93 Creative-Commons songs with
sing/nosing annotations), converts audio to 44.1kHz mono WAV, and outputs
a TSV manifest compatible with train_vocal_detector.py.

Jamendo .lab annotation format (space-separated, one segment per line):
    start_sec end_sec label
    0.000000 12.345678 nosing
    12.345678 45.678901 sing

Labels:
    sing    -> vocal
    nosing  -> non_vocal

The corpus comes pre-split into train/validation/test sets (61/16/16 files).

Usage:
    python3 crates/math-audio/math-dsp/ml/prepare_jamendo.py \\
        --output-dir /path/to/jamendo \\
        --output jamendo_manifest.tsv

    # Only convert to eval manifest (test set), skip train/val:
    python3 crates/math-audio/math-dsp/ml/prepare_jamendo.py \\
        --output-dir /path/to/jamendo \\
        --output jamendo_test_manifest.tsv \\
        --split test

Requires:
    pip install requests
    ffmpeg must be in PATH
"""

import argparse
import glob
import os
import shutil
import subprocess
import sys
import zipfile


DATA_ROOT = "/Volumes/data/Shared/ML"


ZENODO_URL = "https://zenodo.org/api/records/2585988/files/jamando.zip/content"


def download_and_extract(output_dir: str) -> str:
    """Download jamendo.zip from Zenodo and extract it. Returns path to extracted dir."""
    os.makedirs(output_dir, exist_ok=True)
    zip_path = os.path.join(output_dir, "jamando.zip")

    if not os.path.exists(zip_path):
        import requests

        print(f"Downloading {ZENODO_URL} ...")
        resp = requests.get(ZENODO_URL, stream=True, timeout=300)
        resp.raise_for_status()
        total = int(resp.headers.get("content-length", 0))
        downloaded = 0
        with open(zip_path, "wb") as f:
            for chunk in resp.iter_content(chunk_size=1 << 20):
                f.write(chunk)
                downloaded += len(chunk)
                if total > 0:
                    pct = downloaded * 100 // total
                    print(f"\r  {downloaded >> 20} / {total >> 20} MB ({pct}%)", end="", flush=True)
        print()
    else:
        print(f"Using cached {zip_path}")

    # Extract
    extract_dir = os.path.join(output_dir, "jamendo")
    if not os.path.exists(extract_dir):
        print(f"Extracting to {extract_dir} ...")
        with zipfile.ZipFile(zip_path, "r") as zf:
            zf.extractall(output_dir)
        # The zip may extract to a subdirectory; find it
        if not os.path.exists(extract_dir):
            # Check if files were extracted directly
            candidates = [
                d for d in os.listdir(output_dir)
                if os.path.isdir(os.path.join(output_dir, d)) and d != "__MACOSX"
            ]
            if len(candidates) == 1 and candidates[0] != "jamendo":
                os.rename(os.path.join(output_dir, candidates[0]), extract_dir)
    else:
        print(f"Using cached {extract_dir}")

    return extract_dir


def find_audio_and_labels(jamendo_dir: str) -> list[tuple[str, str]]:
    """
    Find matching audio + .lab annotation pairs.

    Jamendo directory structure (typical):
        jamendo/
            audio/          (or mp3/, ogg/)
                song1.mp3
            labels/         (or annotations/)
                song1.lab

    Returns list of (audio_path, lab_path) tuples.
    """
    # Search for annotation files
    lab_files: list[str] = []
    for ext in ("*.lab", "*.txt"):
        lab_files.extend(glob.glob(os.path.join(jamendo_dir, "**", ext), recursive=True))

    if not lab_files:
        print(f"ERROR: No .lab annotation files found in {jamendo_dir}")
        print("  Directory contents:")
        for item in sorted(os.listdir(jamendo_dir))[:20]:
            print(f"    {item}")
        sys.exit(1)

    # For each lab file, find matching audio
    pairs: list[tuple[str, str]] = []
    audio_exts = (".mp3", ".ogg", ".wav", ".flac", ".m4a")

    for lab_path in sorted(lab_files):
        stem = os.path.splitext(os.path.basename(lab_path))[0]

        # Search for audio with same stem anywhere in the tree
        audio_path = None
        for aext in audio_exts:
            candidates = glob.glob(
                os.path.join(jamendo_dir, "**", stem + aext), recursive=True
            )
            if candidates:
                audio_path = candidates[0]
                break

        if audio_path is None:
            print(f"  WARNING: No audio found for {os.path.basename(lab_path)}, skipping")
            continue

        pairs.append((audio_path, lab_path))

    return pairs


def parse_lab_file(lab_path: str) -> list[tuple[float, float, str]]:
    """
    Parse a Jamendo .lab annotation file.

    Format: "start_sec end_sec label" per line.
    Labels: "sing" or "nosing" (sometimes "singing" / "no_singing").
    """
    segments: list[tuple[float, float, str]] = []

    with open(lab_path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            parts = line.split()
            if len(parts) < 3:
                continue
            start_sec = float(parts[0])
            end_sec = float(parts[1])
            raw_label = parts[2].lower()

            if raw_label in ("sing", "singing", "voice", "1"):
                label = "vocal"
            elif raw_label in ("nosing", "no_singing", "novoice", "0"):
                label = "non_vocal"
            else:
                print(f"  WARNING: Unknown label '{parts[2]}' in {lab_path}, treating as non_vocal")
                label = "non_vocal"

            segments.append((start_sec, end_sec, label))

    return segments


def convert_to_wav(audio_path: str, wav_dir: str) -> str | None:
    """Convert audio to 44.1kHz mono WAV using ffmpeg. Returns WAV path."""
    stem = os.path.splitext(os.path.basename(audio_path))[0]
    wav_path = os.path.join(wav_dir, f"{stem}.wav")

    if os.path.exists(wav_path):
        return wav_path

    try:
        subprocess.run(
            [
                "ffmpeg", "-y",
                "-i", audio_path,
                "-ar", "44100",
                "-ac", "1",
                "-sample_fmt", "s16",
                wav_path,
            ],
            check=True,
            capture_output=True,
            timeout=120,
        )
        return wav_path
    except subprocess.CalledProcessError as e:
        stderr = e.stderr.decode("utf-8", errors="replace") if e.stderr else ""
        print(f"  ERROR converting {audio_path}: {stderr[:200]}")
        return None
    except subprocess.TimeoutExpired:
        print(f"  ERROR: Timeout converting {audio_path}")
        return None


def segments_to_manifest_label(segments: list[tuple[float, float, str]]) -> str:
    """Convert segments to manifest format: "start-end:label,start-end:label,..."."""
    parts: list[str] = []
    for start_sec, end_sec, label in segments:
        parts.append(f"{start_sec:.1f}-{end_sec:.1f}:{label}")
    return ",".join(parts)


def detect_splits(jamendo_dir: str) -> dict[str, list[str]]:
    """
    Detect train/validation/test splits from directory structure or filelists.

    Jamendo typically has filelists or subdirectories for each split.
    Returns mapping of split_name -> list of audio stems.
    """
    splits: dict[str, list[str]] = {}

    # Look for filelist files (e.g., train_filelist.txt, test_filelist.txt)
    for split_name in ("train", "valid", "validation", "test"):
        for pattern in (f"{split_name}_filelist.txt", f"{split_name}.txt", f"{split_name}_list.txt"):
            candidates = glob.glob(os.path.join(jamendo_dir, "**", pattern), recursive=True)
            if candidates:
                with open(candidates[0], encoding="utf-8") as f:
                    stems = [line.strip() for line in f if line.strip()]
                    # Normalize split name
                    key = "val" if "valid" in split_name else split_name
                    splits[key] = stems
                break

    return splits


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Prepare Jamendo Corpus manifest for vocal detector training"
    )
    parser.add_argument(
        "--output-dir",
        default=os.path.join(DATA_ROOT, "jamendo"),
        help="Directory to download/extract Jamendo and store WAV files",
    )
    parser.add_argument(
        "--output",
        default=os.path.join(DATA_ROOT, "jamendo_manifest.tsv"),
        help="Output TSV manifest path",
    )
    parser.add_argument(
        "--split",
        choices=["all", "train", "val", "test"],
        default="all",
        help="Which split to include (default: all)",
    )
    parser.add_argument(
        "--skip-download",
        action="store_true",
        help="Skip download, assume data already in --output-dir/jamendo",
    )
    args = parser.parse_args()

    # Validate dependencies
    if shutil.which("ffmpeg") is None:
        print("ERROR: ffmpeg not found in PATH.")
        sys.exit(1)

    # Download and extract
    if args.skip_download:
        jamendo_dir = os.path.join(args.output_dir, "jamendo")
        if not os.path.exists(jamendo_dir):
            print(f"ERROR: {jamendo_dir} not found. Remove --skip-download to auto-download.")
            sys.exit(1)
    else:
        jamendo_dir = download_and_extract(args.output_dir)

    print(f"\nJamendo directory: {jamendo_dir}")

    # Find audio + label pairs
    pairs = find_audio_and_labels(jamendo_dir)
    print(f"Found {len(pairs)} audio files with annotations")

    if not pairs:
        sys.exit(1)

    # Detect splits if filtering
    splits = detect_splits(jamendo_dir)
    if splits:
        print(f"Detected splits: {', '.join(f'{k}({len(v)})' for k, v in splits.items())}")

    # Filter by split if requested
    if args.split != "all" and splits:
        allowed_stems = set(splits.get(args.split, []))
        if not allowed_stems:
            print(f"WARNING: Split '{args.split}' not found in filelists, using all files")
        else:
            pairs = [
                (audio, lab) for audio, lab in pairs
                if os.path.splitext(os.path.basename(audio))[0] in allowed_stems
                or os.path.basename(audio) in allowed_stems
            ]
            print(f"Filtered to {len(pairs)} files for split '{args.split}'")

    # Convert and build manifest
    wav_dir = os.path.join(args.output_dir, "wavs")
    os.makedirs(wav_dir, exist_ok=True)

    manifest_entries: list[tuple[str, str, str]] = []
    converted = 0
    failed = 0

    print(f"\nConverting and processing {len(pairs)} files...")
    for i, (audio_path, lab_path) in enumerate(pairs):
        stem = os.path.splitext(os.path.basename(audio_path))[0]
        print(f"  [{i+1}/{len(pairs)}] {stem}...", end=" ", flush=True)

        # Parse annotations
        segments = parse_lab_file(lab_path)
        if not segments:
            print("SKIPPED (no segments)")
            failed += 1
            continue

        # Convert to WAV
        wav_path = convert_to_wav(audio_path, wav_dir)
        if wav_path is None:
            failed += 1
            print("FAILED")
            continue

        segment_label = segments_to_manifest_label(segments)
        manifest_entries.append((wav_path, "segments", segment_label))
        converted += 1

        vocal_segs = sum(1 for _, _, l in segments if l == "vocal")
        print(f"OK ({len(segments)} segments, {vocal_segs} vocal)")

    # Write manifest
    with open(args.output, "w", encoding="utf-8") as f:
        for wav_path, label_type, label_value in manifest_entries:
            f.write(f"{wav_path}\t{label_type}\t{label_value}\n")

    # Summary
    print(f"\nManifest written to: {args.output}")
    print(f"  Files converted: {converted}")
    print(f"  Files failed:    {failed}")
    print(f"  Manifest entries: {len(manifest_entries)}")


if __name__ == "__main__":
    main()
