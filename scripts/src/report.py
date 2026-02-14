"""HTML report generation for roomeq visualization."""

from pathlib import Path

from .figures import (
    create_channel_figure,
    create_zoomed_figure,
    create_eq_figure,
    create_combined_figure,
)


def create_html_report(
    data: dict,
    output_path: Path,
    output_json_path: Path | None = None,
) -> None:
    """Create an HTML report with all channel plots.

    Args:
        data: Output JSON data (roomeq result)
        output_path: Path to write HTML report
        output_json_path: Path to output JSON (for resolving relative paths)
    """
    channels = data.get("channels", {})
    metadata = data.get("metadata", {})
    version = data.get("version", "unknown")

    # Short name for title: parent_dir/filename
    if output_json_path:
        short_name = f"{output_json_path.parent.name}/{output_json_path.name}"
    else:
        short_name = ""
    page_title = f"RoomEQ Results - {short_name}" if short_name else "RoomEQ Results"

    # Build HTML content
    html_parts = [
        "<!DOCTYPE html>\n"
        "<html>\n"
        "<head>\n"
        '    <meta charset="utf-8">\n'
        f"    <title>{page_title}</title>\n"
        '    <script src="https://cdn.plot.ly/plotly-2.27.0.min.js"></script>\n'
        """    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background: #f5f5f5;
        }
        .container {
            max-width: 1400px;
            margin: 0 auto;
        }
        h1 {
            color: #333;
            border-bottom: 2px solid #4a90d9;
            padding-bottom: 10px;
        }
        h2 {
            color: #444;
            margin-top: 30px;
        }
        .metadata {
            background: white;
            padding: 15px 20px;
            border-radius: 8px;
            margin-bottom: 20px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .metadata h2 {
            margin-top: 0;
            color: #555;
            font-size: 1.1em;
        }
        .metadata-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 10px;
        }
        .metadata-item {
            padding: 5px 0;
        }
        .metadata-label {
            font-weight: 600;
            color: #666;
        }
        .metadata-value {
            color: #333;
        }
        .improvement {
            color: #2ecc71;
            font-weight: bold;
        }
        .plot-container {
            background: white;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 20px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .plot-row {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 20px;
            margin-bottom: 20px;
        }
        @media (max-width: 1000px) {
            .plot-row {
                grid-template-columns: 1fr;
            }
        }
        .filters-section {
            background: white;
            padding: 15px 20px;
            border-radius: 8px;
            margin-bottom: 20px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .filters-section h3 {
            margin-top: 0;
            color: #555;
        }
        .filter-list {
            font-family: monospace;
            font-size: 0.9em;
            background: #f8f8f8;
            padding: 10px;
            border-radius: 4px;
            overflow-x: auto;
        }
        .channel-section {
            border-left: 4px solid #4a90d9;
            padding-left: 15px;
            margin-bottom: 30px;
        }
        .channel-section h2 {
            margin-top: 0;
            color: #4a90d9;
        }
    </style>
</head>
<body>
    <div class="container">
"""
        f"        <h1>{page_title}</h1>\n"
    ]

    # Metadata section
    if metadata:
        pre_score = metadata.get("pre_score", 0)
        post_score = metadata.get("post_score", 0)
        improvement = pre_score - post_score if pre_score and post_score else 0

        html_parts.append(
            f"""
        <div class="metadata">
            <h2>Optimization Summary</h2>
            <div class="metadata-grid">
                <div class="metadata-item">
                    <span class="metadata-label">Version:</span>
                    <span class="metadata-value">{version}</span>
                </div>
                <div class="metadata-item">
                    <span class="metadata-label">Algorithm:</span>
                    <span class="metadata-value">{metadata.get('algorithm', 'N/A')}</span>
                </div>
                <div class="metadata-item">
                    <span class="metadata-label">Iterations:</span>
                    <span class="metadata-value">{metadata.get('iterations', 'N/A')}</span>
                </div>
                <div class="metadata-item">
                    <span class="metadata-label">Score Before:</span>
                    <span class="metadata-value">{pre_score:.2f}</span>
                </div>
                <div class="metadata-item">
                    <span class="metadata-label">Score After:</span>
                    <span class="metadata-value">{post_score:.2f}</span>
                </div>
                <div class="metadata-item">
                    <span class="metadata-label">Improvement:</span>
                    <span class="metadata-value improvement">{improvement:.2f}</span>
                </div>
                <div class="metadata-item">
                    <span class="metadata-label">Timestamp:</span>
                    <span class="metadata-value">{metadata.get('timestamp', 'N/A')}</span>
                </div>
            </div>
        </div>
"""
        )

    # Combined plot
    combined_fig = create_combined_figure(data, output_json_path)
    combined_html = combined_fig.to_html(full_html=False, include_plotlyjs=False)
    html_parts.append(
        f"""
        <div class="plot-container">
            <h2>All Channels Overview</h2>
            {combined_html}
        </div>
"""
    )

    # Individual channel sections
    for channel_name, channel_data in channels.items():
        initial_curve = channel_data.get("initial_curve")
        final_curve = channel_data.get("final_curve")

        # Extract EQ filters
        plugins = channel_data.get("plugins", [])
        eq_filters = []
        for plugin in plugins:
            if plugin.get("plugin_type") == "eq":
                filters = plugin.get("parameters", {}).get("filters", [])
                eq_filters.extend(filters)

        html_parts.append(
            f"""
        <div class="channel-section">
            <h2>Channel: {channel_name}</h2>
"""
        )

        # Full range plot
        fig_full = create_channel_figure(channel_name, initial_curve, final_curve, " (Full Range)")
        full_html = fig_full.to_html(full_html=False, include_plotlyjs=False)

        # Zoomed plot (20-1200 Hz)
        fig_zoom = create_zoomed_figure(channel_name, initial_curve, final_curve)
        zoom_html = fig_zoom.to_html(full_html=False, include_plotlyjs=False)

        html_parts.append(
            f"""
            <div class="plot-row">
                <div class="plot-container">
                    {full_html}
                </div>
                <div class="plot-container">
                    {zoom_html}
                </div>
            </div>
"""
        )

        # EQ response plot
        fig_eq = create_eq_figure(channel_name, eq_filters, channel_data.get("eq_response"))
        if fig_eq:
            eq_html = fig_eq.to_html(full_html=False, include_plotlyjs=False)
            html_parts.append(
                f"""
            <div class="plot-container">
                {eq_html}
            </div>
"""
            )

        # Filter details
        if eq_filters:
            html_parts.append(
                f"""
            <div class="filters-section">
                <h3>EQ Filters</h3>
                <div class="filter-list">
"""
            )
            for i, f in enumerate(eq_filters, 1):
                filter_type = f.get("filter_type", "peak")
                freq = f.get("freq", 0)
                q = f.get("q", 1)
                gain = f.get("db_gain", 0)
                html_parts.append(
                    f"Filter {i}: {filter_type.upper()} @ {freq:.1f} Hz, Q={q:.2f}, Gain={gain:+.1f} dB<br>\n"
                )
            html_parts.append(
                """
                </div>
            </div>
"""
            )

        html_parts.append(
            """
        </div>
"""
        )

    # Close HTML
    html_parts.append(
        """
    </div>
</body>
</html>
"""
    )

    # Write output
    with open(output_path, "w") as f:
        f.write("".join(html_parts))

    print(f"HTML report written to: {output_path}")
