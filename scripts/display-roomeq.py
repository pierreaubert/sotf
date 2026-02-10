#!/usr/bin/env python3
"""
Display roomeq optimization results using Plotly.

Reads a roomeq-generated JSON file and creates an HTML file with interactive
plots comparing initial (without EQ) and final (with EQ) frequency response
curves for each channel.

Usage:
    python scripts/display-roomeq.py <output.json> [output.html] [--input <input.json>]

If no output file is specified, it defaults to <input_basename>_plots.html

When --input is provided with the original roomeq config JSON containing
measurement file paths (CSV or WAV), the IR graph shows corrected room
impulse responses (original measurement convolved with correction filters).
Missing initial_curve data in the output is populated from the input CSVs.
"""

import argparse
import sys
from pathlib import Path

from src.loaders import load_roomeq_json, load_csv_measurement
from src.input_parser import extract_channel_measurement_paths
from src.data_extract import get_all_ir_wav_paths
from src.report import create_html_report


def _inject_curves_from_input(data: dict, input_data: dict, input_json_path: Path | None) -> int:
    """Populate missing initial_curve fields from input measurement CSVs.

    Args:
        data: Output JSON data (modified in place)
        input_data: Input JSON data with speaker measurement references
        input_json_path: Path to input JSON for resolving relative paths

    Returns:
        Number of curves injected
    """
    input_dir = input_json_path.parent if input_json_path else Path(".")
    channel_paths = extract_channel_measurement_paths(input_data, input_dir)
    channels = data.get("channels", {})
    injected = 0

    for channel_name, channel_data in channels.items():
        if channel_data.get("initial_curve"):
            continue

        csv_path = channel_paths.get(channel_name)
        if csv_path is None:
            continue

        if not csv_path.exists():
            print(f"  Warning: CSV not found for {channel_name}: {csv_path}")
            continue

        result = load_csv_measurement(csv_path)
        if result is None:
            continue

        freq, spl, _ = result
        channel_data["initial_curve"] = {"freq": freq, "spl": spl}
        injected += 1
        print(f"  Loaded initial curve for {channel_name} from {csv_path.name}")

    return injected


def main():
    parser = argparse.ArgumentParser(
        description="Display roomeq optimization results using Plotly.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    python scripts/display-roomeq.py output.json
    python scripts/display-roomeq.py output.json result.html
    python scripts/display-roomeq.py output.json --input input.json
    python scripts/display-roomeq.py output.json result.html --input input.json

When --input is provided with the original roomeq config JSON containing
measurement file paths (CSV or WAV), the IR graph shows corrected room
impulse responses (original measurement convolved with correction filters).
""",
    )
    parser.add_argument(
        "output_json",
        type=Path,
        help="Path to roomeq output JSON file (contains correction filters)",
    )
    parser.add_argument(
        "html_output",
        type=Path,
        nargs="?",
        help="Path for HTML output (default: <input>_plots.html)",
    )
    parser.add_argument(
        "-i", "--input",
        type=Path,
        dest="input_json",
        help="Path to roomeq input JSON file (contains measurement paths for corrected IR display)",
    )

    args = parser.parse_args()

    output_json_path = args.output_json
    if not output_json_path.exists():
        print(f"Error: Output JSON file not found: {output_json_path}")
        sys.exit(1)

    # Determine HTML output path
    if args.html_output:
        html_output_path = args.html_output
    else:
        html_output_path = output_json_path.with_name(f"{output_json_path.stem}_plots.html")

    # Load output JSON (roomeq result)
    print(f"Loading output JSON: {output_json_path}")
    data = load_roomeq_json(output_json_path)

    # Load input JSON if provided
    input_data = None
    input_json_path = args.input_json
    if input_json_path:
        if not input_json_path.exists():
            print(f"Warning: Input JSON file not found: {input_json_path}")
            input_json_path = None
        else:
            print(f"Loading input JSON: {input_json_path}")
            input_data = load_roomeq_json(input_json_path)

    # Check for channels
    channels = data.get("channels", {})
    if not channels:
        print("Error: No channels found in the output JSON file")
        sys.exit(1)

    print(f"Found {len(channels)} channel(s): {', '.join(channels.keys())}")

    # Inject missing initial curves from input measurement CSVs
    if input_data:
        missing = sum(1 for ch in channels.values() if not ch.get("initial_curve"))
        if missing:
            print(f"Injecting {missing} missing initial curve(s) from input measurements:")
            injected = _inject_curves_from_input(data, input_data, input_json_path)
            if injected:
                print(f"  Injected {injected} curve(s)")

    # Check for curves
    has_curves = False
    for name, ch in channels.items():
        initial = ch.get("initial_curve")
        final = ch.get("final_curve")
        if initial or final:
            has_curves = True
            print(
                f"  {name}: initial={'yes' if initial else 'no'}, final={'yes' if final else 'no'}"
            )

    if not has_curves:
        print("Warning: No curve data found. The JSON may not contain frequency response data.")

    # Check for FIR correction IR WAV files in output
    ir_paths = get_all_ir_wav_paths(data, output_json_path.parent)
    if ir_paths:
        total_irs = sum(len(paths) for paths in ir_paths.values())
        print(f"Found {total_irs} FIR correction IR file(s)")
        for ch_name, paths in ir_paths.items():
            for ir_name, path in paths:
                exists = "exists" if path.exists() else "MISSING"
                print(f"  {ch_name}/{ir_name}: {path} ({exists})")

    # Check for measurement files in input
    if input_data:
        input_dir = input_json_path.parent if input_json_path else Path(".")
        measurement_paths = extract_channel_measurement_paths(input_data, input_dir)
        if measurement_paths:
            print(f"Found {len(measurement_paths)} measurement file(s) for corrected IR:")
            for ch_name, path in measurement_paths.items():
                exists = "exists" if path.exists() else "MISSING"
                print(f"  {ch_name}: {path} ({exists})")

    # Generate HTML report
    create_html_report(data, html_output_path, output_json_path, input_data, input_json_path)


if __name__ == "__main__":
    main()
