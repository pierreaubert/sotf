#!/usr/bin/env python3
"""
Display roomeq optimization results using Plotly.

Reads a roomeq-generated JSON file and creates an HTML file with interactive
plots comparing initial (without EQ) and final (with EQ) frequency response
curves for each channel.

Usage:
    # Single result
    python scripts/display-roomeq.py <output.json> [output.html]

    # Compare multiple modes (IIR vs FIR vs Hybrid vs MixedPhase)
    python scripts/display-roomeq.py --compare iir.json fir.json hybrid.json [output.html]
"""

import argparse
import sys
from pathlib import Path

from src.loaders import load_roomeq_json
from src.report import create_html_report, create_comparison_html_report


def infer_mode_name(filepath: Path) -> str:
    """Infer processing mode name from filename or parent directory.

    The order matters: longer / more specific names must come first
    so that e.g. `iir_epa.json` resolves to `iir_epa` rather than
    being collapsed to `iir` by an earlier substring match.
    """
    known_modes = (
        "mixed_phase_auto_all",
        "iir_auto_filters",
        "iir_auto_bounds",
        "iir_auto_all",
        "iir_gd_safety_gate",
        "iir_gd_adaptive_allpass",
        "iir_gd_fixed_allpass",
        "iir_gd_delay_only",
        "fir_gd_phase_linear",
        "mixed_phase_gd",
        "mixed_phase_epa",
        "mixed_phase",
        "hybrid_epa",
        "hybrid",
        "fir_epa",
        "fir",
        "iir_epa",
        "iir",
    )
    stem = filepath.stem.lower()
    for mode in known_modes:
        if mode in stem:
            return mode
    # Try parent directory name
    parent = filepath.parent.name.lower()
    for mode in known_modes:
        if mode in parent:
            return mode
    return stem


def main():
    parser = argparse.ArgumentParser(
        description="Display roomeq optimization results using Plotly.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    python scripts/display-roomeq.py output.json
    python scripts/display-roomeq.py output.json result.html
    python scripts/display-roomeq.py --compare iir.json fir.json hybrid.json
    python scripts/display-roomeq.py --compare iir.json fir.json -o comparison.html
""",
    )
    parser.add_argument(
        "output_json",
        type=Path,
        nargs="?",
        help="Path to roomeq output JSON file (single-file mode)",
    )
    parser.add_argument(
        "html_output",
        type=Path,
        nargs="?",
        help="Path for HTML output (default: <input>_plots.html)",
    )
    parser.add_argument(
        "--compare",
        type=Path,
        nargs="+",
        metavar="JSON",
        help="Compare 2 or more roomeq output JSONs (e.g., --compare iir.json fir.json hybrid.json). The report layout adapts to the number of modes.",
    )
    parser.add_argument(
        "-o", "--output",
        type=Path,
        help="Output HTML path (alternative to positional arg)",
    )

    args = parser.parse_args()

    # --- Comparison mode ---
    if args.compare:
        if len(args.compare) < 2:
            print("Error: --compare requires at least 2 JSON files")
            sys.exit(1)

        mode_datasets: list[tuple[str, dict]] = []
        for json_path in args.compare:
            if not json_path.exists():
                print(f"Error: File not found: {json_path}")
                sys.exit(1)
            mode_name = infer_mode_name(json_path)
            data = load_roomeq_json(json_path)
            mode_datasets.append((mode_name, data))
            channels = data.get("channels", {})
            print(f"  Loaded {mode_name}: {len(channels)} channel(s)")

        html_output = args.output or args.html_output
        if html_output is None:
            html_output = args.compare[0].with_name("comparison_plots.html")

        create_comparison_html_report(mode_datasets, html_output)
        return

    # --- Single-file mode ---
    if args.output_json is None:
        parser.print_help()
        sys.exit(1)

    output_json_path = args.output_json
    if not output_json_path.exists():
        print(f"Error: Output JSON file not found: {output_json_path}")
        sys.exit(1)

    html_output_path = args.output or args.html_output
    if html_output_path is None:
        html_output_path = output_json_path.with_name(f"{output_json_path.stem}_plots.html")

    print(f"Loading output JSON: {output_json_path}")
    data = load_roomeq_json(output_json_path)

    channels = data.get("channels", {})
    if not channels:
        print("Error: No channels found in the output JSON file")
        sys.exit(1)

    print(f"Found {len(channels)} channel(s): {', '.join(channels.keys())}")

    bass_management = (data.get("metadata") or {}).get("bass_management") or {}
    routing_graph = bass_management.get("routing_graph") or {}
    routes = routing_graph.get("routes") or []
    if routes:
        outputs = sorted(
            {
                route.get("destination")
                for route in routes
                if route.get("route_kind")
                in {"redirected_bass_lowpass_to_sub", "lfe_lowpass_to_sub"}
                and route.get("destination")
            }
        )
        print(
            "Bass management routing: "
            f"{len(routes)} route(s), physical bass outputs={', '.join(outputs) or 'none'}"
        )

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

    create_html_report(data, html_output_path, output_json_path)


if __name__ == "__main__":
    main()
