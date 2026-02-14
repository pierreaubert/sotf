#!/usr/bin/env python3
"""
Display roomeq optimization results using Plotly.

Reads a roomeq-generated JSON file and creates an HTML file with interactive
plots comparing initial (without EQ) and final (with EQ) frequency response
curves for each channel.

Usage:
    python scripts/display-roomeq.py <output.json> [output.html]

If no output file is specified, it defaults to <input_basename>_plots.html
"""

import argparse
import sys
from pathlib import Path

from src.loaders import load_roomeq_json
from src.report import create_html_report


def main():
    parser = argparse.ArgumentParser(
        description="Display roomeq optimization results using Plotly.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    python scripts/display-roomeq.py output.json
    python scripts/display-roomeq.py output.json result.html
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

    # Check for channels
    channels = data.get("channels", {})
    if not channels:
        print("Error: No channels found in the output JSON file")
        sys.exit(1)

    print(f"Found {len(channels)} channel(s): {', '.join(channels.keys())}")

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

    # Generate HTML report
    create_html_report(data, html_output_path, output_json_path)


if __name__ == "__main__":
    main()
