#!/usr/bin/env python3
"""
Prepare MUSAN dataset (OpenSLR-17) for vocal detector training.

Walks the MUSAN directory structure:
  musan/speech/**/*.wav  -> whole_file:vocal
  musan/noise/**/*.wav   -> whole_file:non_vocal
  musan/music/**/*.wav   -> whole_file:vocal or non_vocal (from ANNOTATIONS)

Outputs a TSV manifest compatible with train_vocal_detector.py --data-dirs.

Usage:
    python3 scripts/prepare_musan.py --musan-dir /path/to/musan --output musan_manifest.tsv

Download MUSAN:
    wget https://openslr.org/resources/17/musan.tar.gz
    tar xzf musan.tar.gz
"""

import argparse
import os
import sys
from pathlib import Path


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


def process_speech_dir(speech_dir: str) -> list[tuple[str, str, str]]:
    """All speech files are vocal."""
    entries: list[tuple[str, str, str]] = []
    wav_files = find_wav_files(speech_dir)
    for wav_path in wav_files:
        entries.append((wav_path, "whole_file", "vocal"))
    return entries


def process_noise_dir(noise_dir: str) -> list[tuple[str, str, str]]:
    """All noise files are non-vocal."""
    entries: list[tuple[str, str, str]] = []
    wav_files = find_wav_files(noise_dir)
    for wav_path in wav_files:
        entries.append((wav_path, "whole_file", "non_vocal"))
    return entries


def process_music_dir(music_dir: str) -> list[tuple[str, str, str]]:
    """
    Music files use ANNOTATIONS for vocal_activity label.

    MUSAN music structure: musan/music/<group>/{ANNOTATIONS, *.wav}
    Each group directory has an ANNOTATIONS file mapping file IDs to vocal_activity.
    """
    entries: list[tuple[str, str, str]] = []
    skipped = 0

    for group_name in sorted(os.listdir(music_dir)):
        group_path = os.path.join(music_dir, group_name)
        if not os.path.isdir(group_path):
            continue

        annotations_path = os.path.join(group_path, "ANNOTATIONS")
        if not os.path.exists(annotations_path):
            # No annotations — skip this group
            wav_count = len(find_wav_files(group_path))
            if wav_count > 0:
                print(f"  WARNING: No ANNOTATIONS in {group_name}, skipping {wav_count} files")
                skipped += wav_count
            continue

        vocal_map = parse_annotations(annotations_path)

        for wav_path in find_wav_files(group_path):
            # Extract file_id: the stem without extension
            file_id = Path(wav_path).stem

            if file_id in vocal_map:
                label = "vocal" if vocal_map[file_id] else "non_vocal"
                entries.append((wav_path, "whole_file", label))
            else:
                print(f"  WARNING: No annotation for {file_id} in {group_name}")
                skipped += 1

    if skipped > 0:
        print(f"  Skipped {skipped} music files without annotations")

    return entries


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Prepare MUSAN dataset manifest for vocal detector training"
    )
    parser.add_argument(
        "--musan-dir",
        required=True,
        help="Path to extracted MUSAN root directory (contains speech/, music/, noise/)",
    )
    parser.add_argument(
        "--output",
        default="musan_manifest.tsv",
        help="Output TSV manifest path (default: musan_manifest.tsv)",
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

    print("Preparing MUSAN manifest...")
    all_entries: list[tuple[str, str, str]] = []

    # Process each subdirectory
    print("\n[1/3] Processing speech/ (all vocal)...")
    speech_entries = process_speech_dir(speech_dir)
    print(f"  Found {len(speech_entries)} speech files")
    all_entries.extend(speech_entries)

    print("\n[2/3] Processing noise/ (all non-vocal)...")
    noise_entries = process_noise_dir(noise_dir)
    print(f"  Found {len(noise_entries)} noise files")
    all_entries.extend(noise_entries)

    print("\n[3/3] Processing music/ (from ANNOTATIONS)...")
    music_entries = process_music_dir(music_dir)
    vocal_music = sum(1 for _, _, label in music_entries if label == "vocal")
    non_vocal_music = sum(1 for _, _, label in music_entries if label == "non_vocal")
    print(f"  Found {len(music_entries)} music files ({vocal_music} vocal, {non_vocal_music} non-vocal)")
    all_entries.extend(music_entries)

    # Write manifest
    with open(output_path, "w", encoding="utf-8") as f:
        for wav_path, label_type, label_value in all_entries:
            f.write(f"{wav_path}\t{label_type}\t{label_value}\n")

    # Summary
    total_vocal = sum(1 for _, _, label in all_entries if label == "vocal")
    total_non_vocal = sum(1 for _, _, label in all_entries if label == "non_vocal")
    print(f"\nManifest written to: {output_path}")
    print(f"  Total: {len(all_entries)} files ({total_vocal} vocal, {total_non_vocal} non-vocal)")


if __name__ == "__main__":
    main()
