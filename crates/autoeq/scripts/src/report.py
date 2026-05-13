"""HTML report generation for roomeq visualization."""

from html import escape
from pathlib import Path

from .figures import (
    create_channel_figure,
    create_zoomed_figure,
    create_eq_figure,
    create_multipass_eq_figure,
    create_ir_figure,
    create_combined_figure,
    create_bass_management_routing_figure,
    create_bass_management_headroom_figure,
    create_comparison_overlay_figure,
    create_comparison_zoomed_figure,
    create_comparison_eq_overlay_figure,
    create_comparison_phase_figure,
    create_comparison_group_delay_figure,
    create_comparison_ir_figure,
    create_mode_subplots_figure,
    create_score_comparison_figure,
    _mode_label,
    _mode_color,
)
from .data_extract import extract_eq_passes, get_channel_sort_key
from .dsp import synthesize_lr_channel

# Synthetic channel name used for the complex L+R sum tab in the
# comparison report. Picked so it cannot collide with a real recording
# channel id (those are short upper-case codes like L / R / LFE / Cs).
LR_SUM_CHANNEL = "L+R"


# Ordered EPA fields with their display labels and formatting rules.
# Used by the pre/post tables in both single-mode and comparison reports.
_EPA_FIELDS: list[tuple[str, str, str]] = [
    ("preference", "Preference", "{:+.2f}"),
    ("evaluation", "Evaluation", "{:+.2f}"),
    ("potency", "Potency", "{:+.2f}"),
    ("activity", "Activity", "{:+.2f}"),
    ("sharpness_acum", "Sharpness (acum)", "{:+.2f}"),
    ("roughness", "Roughness", "{:+.3f}"),
    ("total_loudness_sone", "Total loudness (sone)", "{:+.2f}"),
    ("loudness_balance", "Loudness balance", "{:+.3f}"),
]

# Fields where a larger value is the desired outcome (pre → post).
# Used to colour the delta column green/red.
_EPA_HIGHER_IS_BETTER: set[str] = {
    "preference",
    "evaluation",
    "total_loudness_sone",
    "loudness_balance",
}


def _format_delta(field: str, pre: float | None, post: float | None) -> str:
    """Return an HTML-colored span with the post-pre delta for an EPA field."""
    if pre is None or post is None:
        return '<span style="color:#999">-</span>'
    delta = post - pre
    if abs(delta) < 1e-9:
        return '<span style="color:#999">=</span>'
    higher_is_better = field in _EPA_HIGHER_IS_BETTER
    improved = (delta > 0) == higher_is_better
    color = "#2ecc71" if improved else "#e74c3c"
    return f'<span style="color:{color};font-weight:600">{delta:+.3f}</span>'


def _epa_channel_table_html(channel_epa: dict | None) -> str:
    """Render the pre/post EPA comparison table for a single channel.

    `channel_epa` is the `epa_per_channel[<channel>]` dict holding `pre` and
    `post` EpaScore objects. Returns an empty string if the data is missing
    or malformed so the caller can just append it unconditionally.
    """
    if not channel_epa:
        return ""
    pre = channel_epa.get("pre") or {}
    post = channel_epa.get("post") or {}
    if not pre and not post:
        return ""

    rows: list[str] = []
    for field, label, fmt in _EPA_FIELDS:
        pre_val = pre.get(field)
        post_val = post.get(field)
        pre_str = fmt.format(pre_val) if isinstance(pre_val, (int, float)) else "-"
        post_str = fmt.format(post_val) if isinstance(post_val, (int, float)) else "-"
        delta_html = _format_delta(field, pre_val, post_val)
        rows.append(
            f"                        <tr><td>{label}</td>"
            f"<td>{pre_str}</td><td>{post_str}</td>"
            f"<td>{delta_html}</td></tr>\n"
        )

    return (
        '                <div class="epa-section">\n'
        '                    <h3>EPA Psychoacoustic Scores</h3>\n'
        '                    <table class="epa-table">\n'
        "                        <thead><tr>"
        "<th>Metric</th><th>Before EQ</th><th>After EQ</th><th>Δ</th>"
        "</tr></thead>\n"
        "                        <tbody>\n"
        + "".join(rows)
        + "                        </tbody>\n"
        "                    </table>\n"
        '                    <p class="epa-footer">Higher is better for '
        "Preference / Evaluation / Total loudness / Loudness balance; "
        "lower is better for Activity / Sharpness deviation / Roughness. "
        "Δ cells are green when the change improves the metric.</p>\n"
        "                </div>\n"
    )


def _epa_comparison_table_html(
    ch_name: str,
    mode_datasets: list[tuple[str, dict]],
) -> str:
    """Render a multi-mode EPA comparison table for a single channel.

    Shows one column per processing mode with its post-EQ EPA values, plus
    a single "Before EQ" column taken from the first mode that has pre
    data (the input measurement is the same across modes in a typical
    comparison run).
    """
    # Collect (mode_name, pre_dict, post_dict) triples; skip modes without data.
    entries: list[tuple[str, dict, dict]] = []
    for mode_name, data in mode_datasets:
        ch_epa = (data.get("metadata") or {}).get("epa_per_channel", {}).get(ch_name)
        if not ch_epa:
            continue
        pre = ch_epa.get("pre") or {}
        post = ch_epa.get("post") or {}
        if not pre and not post:
            continue
        entries.append((mode_name, pre, post))

    if not entries:
        return ""

    # Use the first mode's pre-EQ values as the "Before EQ" baseline.
    _, baseline_pre, _ = entries[0]

    mode_header_cells = "".join(
        f"<th>{_mode_label(name)} (post)</th>" for name, _, _ in entries
    )
    header_row = f"<tr><th>Metric</th><th>Before EQ</th>{mode_header_cells}</tr>"

    body_rows: list[str] = []
    for field, label, fmt in _EPA_FIELDS:
        pre_val = baseline_pre.get(field)
        pre_str = fmt.format(pre_val) if isinstance(pre_val, (int, float)) else "-"
        mode_cells: list[str] = []
        for _, _, post in entries:
            post_val = post.get(field)
            if isinstance(post_val, (int, float)):
                post_str = fmt.format(post_val)
                if isinstance(pre_val, (int, float)):
                    delta_span = _format_delta(field, pre_val, post_val)
                    cell = f'<td>{post_str} <span style="font-size:0.85em">{delta_span}</span></td>'
                else:
                    cell = f"<td>{post_str}</td>"
            else:
                cell = '<td style="color:#999">-</td>'
            mode_cells.append(cell)
        body_rows.append(
            f"<tr><td>{label}</td><td>{pre_str}</td>{''.join(mode_cells)}</tr>"
        )

    return (
        '<div class="epa-section">\n'
        '    <h3>EPA Psychoacoustic Scores (by mode)</h3>\n'
        '    <table class="epa-table">\n'
        f"        <thead>{header_row}</thead>\n"
        "        <tbody>\n            "
        + "\n            ".join(body_rows)
        + "\n        </tbody>\n"
        "    </table>\n"
        '    <p class="epa-footer">Higher is better for Preference / '
        "Evaluation / Total loudness / Loudness balance; lower is better "
        "for Activity / Sharpness deviation / Roughness. Δ colours show "
        "whether each mode improved the metric versus the pre-EQ baseline.</p>\n"
        "</div>\n"
    )


def _epa_summary_pref(metadata: dict) -> tuple[float | None, float | None]:
    """Return the (pre, post) preference averaged across all channels.

    Returns `(None, None)` if `metadata.epa_per_channel` is absent or empty.
    """
    per_channel = metadata.get("epa_per_channel") or {}
    if not per_channel:
        return (None, None)
    pre_vals: list[float] = []
    post_vals: list[float] = []
    for entry in per_channel.values():
        pre = (entry or {}).get("pre") or {}
        post = (entry or {}).get("post") or {}
        if isinstance(pre.get("preference"), (int, float)):
            pre_vals.append(pre["preference"])
        if isinstance(post.get("preference"), (int, float)):
            post_vals.append(post["preference"])
    pre_avg = sum(pre_vals) / len(pre_vals) if pre_vals else None
    post_avg = sum(post_vals) / len(post_vals) if post_vals else None
    return (pre_avg, post_avg)


def _fmt_db(value: object, suffix: str = " dB") -> str:
    return f"{value:+.2f}{suffix}" if isinstance(value, (int, float)) else "-"


def _fmt_hz(value: object) -> str:
    return f"{value:.1f} Hz" if isinstance(value, (int, float)) else "-"


def _fmt_ms(value: object) -> str:
    return f"{value:.3f} ms" if isinstance(value, (int, float)) else "-"


def _eq_filter_counts(data: dict) -> list[int]:
    channels = data.get("channels") or {}
    if isinstance(channels, dict):
        channel_values = channels.values()
    elif isinstance(channels, list):
        channel_values = channels
    else:
        return []

    counts: list[int] = []
    for channel in channel_values:
        if not isinstance(channel, dict):
            continue
        total = 0
        for plugin in channel.get("plugins") or []:
            if not isinstance(plugin, dict) or plugin.get("plugin_type") != "eq":
                continue
            params = plugin.get("parameters") or {}
            filters = params.get("filters") or plugin.get("filters") or []
            if isinstance(filters, list):
                total += len(filters)
        counts.append(total)
    return counts


def _eq_filter_summary_html(data: dict) -> str:
    counts = _eq_filter_counts(data)
    if not counts:
        return '<td style="color:#999">-</td>'
    if min(counts) == max(counts):
        return f"<td>{counts[0]}</td>"
    avg = sum(counts) / len(counts)
    return f"<td>{min(counts)}-{max(counts)} (avg {avg:.1f})</td>"


def _gd_summary_cells_html(metadata: dict) -> str:
    """Render comparison-table cells for metadata.group_delay."""
    gd = metadata.get("group_delay") or {}
    if not gd:
        return (
            '<td style="color:#999">-</td>'
            '<td style="color:#999">-</td>'
            '<td style="color:#999">-</td>'
            '<td style="color:#999">-</td>'
            '<td style="color:#999">-</td>'
        )

    advisory = str(gd.get("advisory", "-"))
    advisory_color = "#2ecc71" if advisory == "success" else "#b9770e"
    applied = gd.get("applied")
    applied_str = "yes" if applied is True else "no" if applied is False else "-"
    applied_color = "#2ecc71" if applied is True else "#999"
    pre = gd.get("sum_gd_pre_rms_ms")
    post = gd.get("sum_gd_post_rms_ms")
    rms_str = f"{_fmt_ms(pre)} → {_fmt_ms(post)}"
    improvement = gd.get("improvement_db")
    improvement_str = f"{improvement:+.2f} dB" if isinstance(improvement, (int, float)) else "-"
    improvement_color = (
        "#2ecc71" if isinstance(improvement, (int, float)) and improvement >= 0.0 else "#e74c3c"
    )
    ap_counts = gd.get("per_channel_ap_count") or []
    ap_total = sum(v for v in ap_counts if isinstance(v, int))
    mean_coh = gd.get("mean_coherence")
    coh_str = f", coh={mean_coh:.2f}" if isinstance(mean_coh, (int, float)) else ""
    ap_str = f"{ap_total}{coh_str}"

    return (
        f'<td style="color:{advisory_color};font-weight:600">{escape(advisory)}</td>'
        f'<td style="color:{applied_color};font-weight:600">{applied_str}</td>'
        f"<td>{rms_str}</td>"
        f'<td style="color:{improvement_color};font-weight:600">{improvement_str}</td>'
        f"<td>{ap_str}</td>"
    )


def _bass_management_summary_html(report: dict) -> str:
    if not report:
        return ""

    routing = report.get("routing_graph") or {}
    routes = routing.get("routes") or []
    route_count = len(routes)
    physical_outputs = sorted(
        {
            str(route.get("destination"))
            for route in routes
            if route.get("route_kind")
            in {"redirected_bass_lowpass_to_sub", "lfe_lowpass_to_sub"}
            and route.get("destination")
        }
    )
    advisories: list[str] = []
    advisory = report.get("advisory")
    if advisory:
        advisories.append(str(advisory))
    advisories.extend(str(item) for item in routing.get("advisories") or [])
    advisories = sorted({item for item in advisories if item and item != "ok"})
    advisory_html = (
        f"<br><span style=\"color:#b36b00\">{escape('; '.join(advisories))}</span>"
        if advisories
        else ""
    )

    items = [
        ("Enabled", "yes" if report.get("enabled") else "no"),
        ("Crossover", f"{escape(str(report.get('crossover_type', '-')))} @ {_fmt_hz(report.get('crossover_frequency_hz'))}"),
        ("LFE gain", _fmt_db(report.get("lfe_playback_gain_db"))),
        ("Shared sub gain", _fmt_db(report.get("applied_sub_gain_db"))),
        ("Physical bass outputs", ", ".join(escape(name) for name in physical_outputs) or escape(str(report.get("physical_sub_output", "-")))),
        ("Route count", str(route_count)),
        ("Graph mode", "route branches" if route_count else "linear / none"),
    ]
    cells = "".join(
        f'<div class="metadata-item"><span class="metadata-label">{label}:</span> '
        f'<span class="metadata-value">{value}</span></div>\n'
        for label, value in items
    )
    return (
        '<div class="metadata bass-management-section">\n'
        "<h2>Bass Management</h2>\n"
        f'<div class="metadata-grid">{cells}</div>\n'
        f"{advisory_html}\n"
        "</div>\n"
    )


def _bass_management_groups_table_html(report: dict) -> str:
    groups = report.get("groups") or []
    if not groups:
        groups = ((report.get("optimization") or {}).get("group_results") or [])
    if not groups:
        return ""

    rows = []
    for group in groups:
        advisories = ", ".join(
            str(item) for item in group.get("advisories", []) if item != "ok"
        )
        rows.append(
            "<tr>"
            f"<td>{escape(str(group.get('group_id', '-')))}</td>"
            f"<td>{escape(', '.join(str(role) for role in group.get('roles', [])))}</td>"
            f"<td>{escape(str(group.get('crossover_type', '-')))}</td>"
            f"<td>{_fmt_hz(group.get('selected_crossover_hz'))}</td>"
            f"<td>{_fmt_ms(group.get('main_delay_ms'))}</td>"
            f"<td>{_fmt_ms(group.get('bass_route_delay_ms'))}</td>"
            f"<td>{'yes' if group.get('polarity_inverted') else 'no'}</td>"
            f"<td>{_fmt_db(group.get('trim_db'))}</td>"
            f"<td>{escape(advisories) if advisories else '-'}</td>"
            "</tr>\n"
        )

    return (
        '<div class="filters-section bass-management-section">\n'
        "<h3>Per-Speaker-Group Crossovers</h3>\n"
        '<table class="bm-table">\n'
        "<thead><tr><th>Group</th><th>Roles</th><th>Type</th><th>Selected XO</th>"
        "<th>Main delay</th><th>Bass delay</th><th>Invert bass</th><th>Trim</th><th>Advisories</th></tr></thead>\n"
        f"<tbody>{''.join(rows)}</tbody>\n"
        "</table>\n"
        "</div>\n"
    )


def _bass_management_sub_outputs_table_html(report: dict) -> str:
    outputs = report.get("sub_outputs") or []
    if not outputs:
        outputs = ((report.get("optimization") or {}).get("sub_output_results") or [])
    if not outputs:
        return ""

    rows = []
    for output in outputs:
        rows.append(
            "<tr>"
            f"<td>{escape(str(output.get('output_role', '-')))}</td>"
            f"<td>{escape(str(output.get('strategy_source', '-')))}</td>"
            f"<td>{_fmt_db(output.get('gain_db'))}</td>"
            f"<td>{_fmt_ms(output.get('delay_ms'))}</td>"
            f"<td>{'yes' if output.get('polarity_inverted') else 'no'}</td>"
            f"<td>{_fmt_db(output.get('headroom_contribution_db'))}</td>"
            "</tr>\n"
        )

    return (
        '<div class="filters-section bass-management-section">\n'
        "<h3>Physical Bass Outputs</h3>\n"
        '<table class="bm-table">\n'
        "<thead><tr><th>Output</th><th>Strategy</th><th>Gain</th><th>Delay</th>"
        "<th>Invert</th><th>Headroom contribution</th></tr></thead>\n"
        f"<tbody>{''.join(rows)}</tbody>\n"
        "</table>\n"
        "</div>\n"
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
    channels_dict = data.get("channels", {})
    metadata = data.get("metadata", {})
    version = data.get("version", "unknown")

    # Sort channels by classical order
    sorted_channel_names = sorted(channels_dict.keys(), key=get_channel_sort_key)
    channels = [(name, channels_dict[name]) for name in sorted_channel_names]

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
            background: #fdfdfd;
            padding: 15px 20px;
            border-radius: 8px;
            margin-top: 20px;
            border: 1px solid #eee;
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
            padding: 10px 0;
        }
        .epa-section {
            background: #fafafa;
            padding: 15px 20px;
            border-radius: 8px;
            margin-top: 20px;
            border: 1px solid #eee;
        }
        .epa-section h3 {
            margin-top: 0;
            color: #555;
        }
        .epa-table {
            width: 100%;
            border-collapse: collapse;
            font-size: 0.95em;
        }
        .epa-table th,
        .epa-table td {
            padding: 6px 10px;
            border: 1px solid #e4e4e4;
            text-align: right;
        }
        .epa-table th:first-child,
        .epa-table td:first-child {
            text-align: left;
        }
        .epa-table th {
            background: #f1f1f1;
            font-weight: 600;
            color: #555;
        }
        .epa-table tbody tr:nth-child(odd) {
            background: #fdfdfd;
        }
        .epa-footer {
            margin: 10px 0 0;
            color: #888;
            font-size: 0.8em;
            font-style: italic;
        }
        .bass-management-section {
            border-left: 4px solid #2ecc71;
        }
        .bm-table {
            width: 100%;
            border-collapse: collapse;
            font-size: 0.92em;
        }
        .bm-table th,
        .bm-table td {
            padding: 7px 9px;
            border: 1px solid #e4e4e4;
            text-align: left;
            vertical-align: top;
        }
        .bm-table th {
            background: #f1f1f1;
            font-weight: 600;
            color: #555;
        }
        .bm-table tbody tr:nth-child(odd) {
            background: #fdfdfd;
        }

        /* Tabs styles */
        .tabs-container {
            margin-top: 30px;
        }
        .tab-header {
            display: flex;
            flex-wrap: wrap;
            background: #e0e0e0;
            padding: 10px 10px 0;
            border-radius: 8px 8px 0 0;
            gap: 2px;
        }
        .tab-btn {
            padding: 10px 20px;
            border: none;
            background: #d0d0d0;
            cursor: pointer;
            border-radius: 5px 5px 0 0;
            font-weight: 600;
            color: #666;
            transition: all 0.2s;
        }
        .tab-btn:hover {
            background: #c0c0c0;
        }
        .tab-btn.active {
            background: white;
            color: #4a90d9;
            border-top: 3px solid #4a90d9;
        }
        .tab-content {
            display: none;
            background: white;
            padding: 20px;
            border-radius: 0 0 8px 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .tab-content.active {
            display: block;
        }
    </style>
    <script>
        function openChannel(evt, channelId) {
            var i, tabcontent, tablinks;
            tabcontent = document.getElementsByClassName("tab-content");
            for (i = 0; i < tabcontent.length; i++) {
                tabcontent[i].classList.remove("active");
            }
            tablinks = document.getElementsByClassName("tab-btn");
            for (i = 0; i < tablinks.length; i++) {
                tablinks[i].classList.remove("active");
            }
            document.getElementById(channelId).classList.add("active");
            evt.currentTarget.classList.add("active");
            
            // Trigger resize to fix Plotly plots in the newly visible tab
            window.dispatchEvent(new Event('resize'));
        }
    </script>
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

        epa_pre_avg, epa_post_avg = _epa_summary_pref(metadata)
        if epa_pre_avg is not None and epa_post_avg is not None:
            epa_delta = epa_post_avg - epa_pre_avg
            epa_color = "#2ecc71" if epa_delta >= 0 else "#e74c3c"
            epa_summary_html = (
                '                <div class="metadata-item">\n'
                '                    <span class="metadata-label">EPA Preference (avg):</span>\n'
                f'                    <span class="metadata-value">{epa_pre_avg:.2f} → {epa_post_avg:.2f} '
                f'<span style="color:{epa_color};font-weight:600">({epa_delta:+.2f})</span></span>\n'
                "                </div>\n"
            )
        else:
            epa_summary_html = ""

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
                    <span class="metadata-label">Loss function:</span>
                    <span class="metadata-value">{metadata.get('loss_type', 'N/A')}</span>
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
{epa_summary_html}                <div class="metadata-item">
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

    # Bass-management routing/headroom section. This is driven by the
    # route-level #14 schema, not the deprecated single matrix summary.
    bass_management = metadata.get("bass_management") or {}
    if bass_management:
        html_parts.append(_bass_management_summary_html(bass_management))
        routing_fig = create_bass_management_routing_figure(data)
        headroom_fig = create_bass_management_headroom_figure(data)
        if routing_fig or headroom_fig:
            html_parts.append('<div class="plot-row">\n')
            if routing_fig:
                html_parts.append(
                    f'<div class="plot-container">{routing_fig.to_html(full_html=False, include_plotlyjs=False)}</div>\n'
                )
            if headroom_fig:
                html_parts.append(
                    f'<div class="plot-container">{headroom_fig.to_html(full_html=False, include_plotlyjs=False)}</div>\n'
                )
            html_parts.append("</div>\n")
        html_parts.append(_bass_management_groups_table_html(bass_management))
        html_parts.append(_bass_management_sub_outputs_table_html(bass_management))

    # Individual channel sections in tabs
    html_parts.append('<div class="tabs-container">\n')
    html_parts.append('    <div class="tab-header">\n')
    for i, (channel_name, _) in enumerate(channels):
        active_class = " active" if i == 0 else ""
        safe_id = f"channel_{i}"
        html_parts.append(f'        <button class="tab-btn{active_class}" onclick="openChannel(event, \'{safe_id}\')">{channel_name}</button>\n')
    html_parts.append('    </div>\n')

    for i, (channel_name, channel_data) in enumerate(channels):
        active_class = " active" if i == 0 else ""
        safe_id = f"channel_{i}"
        initial_curve = channel_data.get("initial_curve")
        final_curve = channel_data.get("final_curve")

        # Extract EQ filters (grouped by pass for 3-pass pipeline)
        passes = extract_eq_passes(channel_data)
        eq_filters = []
        for p in passes:
            eq_filters.extend(p["filters"])

        html_parts.append(
            f"""
        <div id="{safe_id}" class="tab-content{active_class}">
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

        # EQ response plot (uses per-pass breakdown when 3-pass labels are present)
        fig_eq = create_multipass_eq_figure(
            channel_name, channel_data, channel_data.get("eq_response")
        )
        if fig_eq is None:
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

        # IR waveform plot
        fig_ir = create_ir_figure(
            channel_name,
            channel_data.get("pre_ir"),
            channel_data.get("post_ir"),
        )
        if fig_ir:
            ir_html = fig_ir.to_html(full_html=False, include_plotlyjs=False)
            html_parts.append(
                f"""
                <div class="plot-container">
                    {ir_html}
                </div>
"""
            )

        # EPA psychoacoustic scores (pre/post) for this channel
        epa_per_channel = metadata.get("epa_per_channel") or {}
        epa_html = _epa_channel_table_html(epa_per_channel.get(channel_name))
        if epa_html:
            html_parts.append(epa_html)

        # Filter details (grouped by pass when 3-pass labels are present)
        has_labeled = any(p["label"] for p in passes)
        if has_labeled and passes:
            html_parts.append(
                """
                <div class="filters-section">
                    <h3>EQ Filters (3-Pass Pipeline)</h3>
"""
            )
            for p in passes:
                html_parts.append(
                    f"""
                    <h4 style="color: {p['color']}; margin-bottom: 5px;">{p['display_name']}</h4>
                    <div class="filter-list" style="margin-bottom: 10px;">
"""
                )
                for j, f in enumerate(p["filters"], 1):
                    filter_type = f.get("filter_type", "peak")
                    freq = f.get("freq", 0)
                    q = f.get("q", 1)
                    gain = f.get("db_gain", 0)
                    html_parts.append(
                        f"Filter {j}: {filter_type.upper()} @ {freq:.1f} Hz, Q={q:.2f}, Gain={gain:+.1f} dB<br>\n"
                    )
                html_parts.append("                    </div>\n")
            html_parts.append("                </div>\n")
        elif eq_filters:
            html_parts.append(
                """
                <div class="filters-section">
                    <h3>EQ Filters</h3>
                    <div class="filter-list">
"""
            )
            for j, f in enumerate(eq_filters, 1):
                filter_type = f.get("filter_type", "peak")
                freq = f.get("freq", 0)
                q = f.get("q", 1)
                gain = f.get("db_gain", 0)
                html_parts.append(
                    f"Filter {j}: {filter_type.upper()} @ {freq:.1f} Hz, Q={q:.2f}, Gain={gain:+.1f} dB<br>\n"
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
        </div>
"""
        )

    html_parts.append('</div><!-- tabs-container -->\n')

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


def create_comparison_html_report(
    mode_datasets: list[tuple[str, dict]],
    output_path: Path,
) -> None:
    """Create an HTML report comparing multiple processing modes.

    Args:
        mode_datasets: List of (mode_name, roomeq_output_data) tuples.
        output_path: Path to write HTML report.
    """
    # Collect all channel names across modes (union)
    all_channel_names: set[str] = set()
    for _, data in mode_datasets:
        all_channel_names.update(data.get("channels", {}).keys())
    sorted_channels = sorted(all_channel_names, key=get_channel_sort_key)

    # If both L and R are present in at least one mode, append a
    # synthetic "L+R" tab whose curves are the complex (coherent) sum
    # of the per-mode L and R curves. The tab is only emitted when at
    # least one mode actually has both channels — otherwise the sum
    # would be undefined and the tab would be empty.
    has_lr_pair = any(
        "L" in data.get("channels", {}) and "R" in data.get("channels", {})
        for _, data in mode_datasets
    )
    if has_lr_pair:
        sorted_channels.append(LR_SUM_CHANNEL)

    mode_names = [name for name, _ in mode_datasets]
    page_title = f"RoomEQ Mode Comparison: {', '.join(_mode_label(n) for n in mode_names)}"

    html_parts = [
        "<!DOCTYPE html>\n<html>\n<head>\n"
        '    <meta charset="utf-8">\n'
        f"    <title>{page_title}</title>\n"
        '    <script src="https://cdn.plot.ly/plotly-2.27.0.min.js"></script>\n'
        """    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
               margin: 0; padding: 20px; background: #f5f5f5; }
        .container { max-width: 1400px; margin: 0 auto; }
        h1 { color: #333; border-bottom: 2px solid #4a90d9; padding-bottom: 10px; }
        h2 { color: #444; margin-top: 30px; }
        .summary-table { width: 100%; border-collapse: collapse; margin: 15px 0; }
        .summary-table th, .summary-table td { padding: 8px 12px; border: 1px solid #ddd; text-align: center; }
        .summary-table th { background: #f0f0f0; font-weight: 600; color: #555; }
        .improvement { color: #2ecc71; font-weight: bold; }
        .plot-container { background: white; padding: 15px; border-radius: 8px;
                         margin-bottom: 20px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .plot-row { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; margin-bottom: 20px; }
        @media (max-width: 1000px) { .plot-row { grid-template-columns: 1fr; } }
        .tabs-container { margin-top: 30px; }
        .tab-header { display: flex; flex-wrap: wrap; background: #e0e0e0;
                     padding: 10px 10px 0; border-radius: 8px 8px 0 0; gap: 2px; }
        .tab-btn { padding: 10px 20px; border: none; background: #d0d0d0; cursor: pointer;
                  border-radius: 5px 5px 0 0; font-weight: 600; color: #666; transition: all 0.2s; }
        .tab-btn:hover { background: #c0c0c0; }
        .tab-btn.active { background: white; color: #4a90d9; border-top: 3px solid #4a90d9; }
        .tab-content { display: none; background: white; padding: 20px;
                      border-radius: 0 0 8px 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .tab-content.active { display: block; }
        .epa-section { background: #fafafa; padding: 15px 20px; border-radius: 8px;
                      margin-top: 20px; border: 1px solid #eee; }
        .epa-section h3 { margin-top: 0; color: #555; }
        .epa-table { width: 100%; border-collapse: collapse; font-size: 0.95em; }
        .epa-table th, .epa-table td { padding: 6px 10px; border: 1px solid #e4e4e4; text-align: right; }
        .epa-table th:first-child, .epa-table td:first-child { text-align: left; }
        .epa-table th { background: #f1f1f1; font-weight: 600; color: #555; }
        .epa-table tbody tr:nth-child(odd) { background: #fdfdfd; }
        .epa-footer { margin: 10px 0 0; color: #888; font-size: 0.8em; font-style: italic; }
    </style>
    <script>
        function openChannel(evt, channelId) {
            var tabcontent = document.getElementsByClassName("tab-content");
            for (var i = 0; i < tabcontent.length; i++) tabcontent[i].classList.remove("active");
            var tablinks = document.getElementsByClassName("tab-btn");
            for (var i = 0; i < tablinks.length; i++) tablinks[i].classList.remove("active");
            document.getElementById(channelId).classList.add("active");
            evt.currentTarget.classList.add("active");
            window.dispatchEvent(new Event('resize'));
        }
    </script>
</head>
<body>
    <div class="container">
"""
        f"        <h1>{page_title}</h1>\n"
    ]

    # --- Summary table ---
    html_parts.append('<div class="plot-container">\n<h2>Summary</h2>\n')
    html_parts.append('<table class="summary-table">\n<thead><tr>')
    has_gd_summary = any(
        (data.get("metadata") or {}).get("group_delay") for _, data in mode_datasets
    )
    has_auto_summary = any("_auto" in mode_name for mode_name, _ in mode_datasets)
    html_parts.append(
        "<th>Mode</th><th>Loss</th>"
        '<th title="Flat loss before EQ (always computed via compute_flat_loss, regardless of which objective was minimized)">Pre flat-loss</th>'
        '<th title="Flat loss after EQ (always computed via compute_flat_loss, regardless of which objective was minimized)">Post flat-loss</th>'
        "<th>Improvement</th>"
        "<th>EPA Pref (pre)</th><th>EPA Pref (post)</th><th>EPA Δ</th>"
    )
    if has_auto_summary:
        html_parts.append(
            '<th title="Per-channel count of emitted EQ filters; ranges show min-max across channels">EQ filters</th>'
        )
    if has_gd_summary:
        html_parts.append(
            '<th title="Group-delay optimization advisory">GD advisory</th>'
            '<th title="Whether GD controls were inserted into exported DSP">GD applied</th>'
            '<th title="Summed in-band GD RMS before and after GD optimization">GD RMS</th>'
            '<th title="GD RMS improvement, 20*log10(pre/post)">GD Δ</th>'
            '<th title="Total emitted all-pass filters; coh is mean in-band coherence">GD AP/coh</th>'
        )
    html_parts.append("</tr></thead>\n<tbody>\n")

    mode_scores: list[tuple[str, float, float]] = []
    loss_types: list[str | None] = []
    for mode_name, data in mode_datasets:
        meta = data.get("metadata", {})
        pre = meta.get("pre_score", 0)
        post = meta.get("post_score", 0)
        improv = ((pre - post) / pre * 100) if pre > 0 else 0
        mode_scores.append((mode_name, pre, post))
        color = _mode_color(mode_name)

        loss_type = meta.get("loss_type")
        loss_types.append(loss_type)
        loss_cell = f"<td>{loss_type}</td>" if loss_type else '<td style="color:#999">-</td>'

        epa_pre_avg, epa_post_avg = _epa_summary_pref(meta)
        if epa_pre_avg is not None and epa_post_avg is not None:
            epa_delta = epa_post_avg - epa_pre_avg
            epa_color = "#2ecc71" if epa_delta >= 0 else "#e74c3c"
            epa_cells = (
                f"<td>{epa_pre_avg:.2f}</td><td>{epa_post_avg:.2f}</td>"
                f'<td style="color:{epa_color};font-weight:600">{epa_delta:+.2f}</td>'
            )
        else:
            epa_cells = '<td style="color:#999">-</td><td style="color:#999">-</td><td style="color:#999">-</td>'

        html_parts.append(
            f'<tr><td style="color:{color};font-weight:600">{_mode_label(mode_name)}</td>'
            f"{loss_cell}"
            f"<td>{pre:.4f}</td><td>{post:.4f}</td>"
            f'<td class="improvement">{improv:.1f}%</td>'
            f"{epa_cells}"
            f"{_eq_filter_summary_html(data) if has_auto_summary else ''}"
            f"{_gd_summary_cells_html(meta) if has_gd_summary else ''}</tr>\n"
        )
    html_parts.append("</tbody></table>\n")

    # When the report mixes loss functions, clarify what the Pre/Post
    # flat-loss columns actually measure — readers might otherwise
    # assume an EPA-loss run's "score" reflects the EPA composite.
    distinct_losses = sorted({lt for lt in loss_types if lt})
    if len(distinct_losses) >= 2:
        html_parts.append(
            '<p style="color:#555;font-size:0.9em;margin-top:10px;">'
            "ℹ This report mixes runs that minimized different loss "
            f"functions ({', '.join(distinct_losses)}). The "
            "<strong>Pre flat-loss</strong> and <strong>Post flat-loss</strong> "
            "columns are <em>always</em> computed via the flat-loss helper "
            "(<code>compute_flat_loss</code> over <code>[min_freq, max_freq]</code>), "
            "regardless of which objective each mode actually minimized — "
            "this keeps every mode on the same scale so the columns answer "
            "<em>“how flat is the response?”</em> for all modes. For "
            "<em>perceptual</em> outcomes across loss types, the "
            "<strong>EPA Pref</strong> columns are the right place to look."
            "</p>\n"
        )
    if has_gd_summary:
        html_parts.append(
            '<p style="color:#555;font-size:0.9em;margin-top:10px;">'
            "GD columns come from <code>metadata.group_delay</code>. "
            "A non-success advisory is expected for recordings that do not "
            "carry coherence or independent sweep realisations; it means the "
            "production safety gates downgraded or skipped the requested GD path."
            "</p>\n"
        )
    if has_auto_summary:
        html_parts.append(
            '<p style="color:#555;font-size:0.9em;margin-top:10px;">'
            "Auto columns show emitted EQ filter counts from the output JSON. "
            "Resolved automatic Q and gain bounds are logged by roomeq during the run "
            "but are not currently persisted in comparison JSON."
            "</p>\n"
        )

    # Score bar chart (passes loss_types so the chart can label/warn correctly)
    score_fig = create_score_comparison_figure(mode_scores, loss_types)
    html_parts.append(score_fig.to_html(full_html=False, include_plotlyjs=False))
    html_parts.append("</div>\n")

    # --- Per-channel tabs ---
    html_parts.append('<div class="tabs-container">\n<div class="tab-header">\n')
    for i, ch_name in enumerate(sorted_channels):
        active = " active" if i == 0 else ""
        html_parts.append(
            f'    <button class="tab-btn{active}" '
            f"""onclick="openChannel(event, 'ch_{i}')">{ch_name}</button>\n"""
        )
    html_parts.append("</div>\n")

    for i, ch_name in enumerate(sorted_channels):
        active = " active" if i == 0 else ""
        html_parts.append(f'<div id="ch_{i}" class="tab-content{active}">\n')
        is_lr = (ch_name == LR_SUM_CHANNEL)
        if is_lr:
            html_parts.append(
                "<h2>Channel: L+R (complex sum)</h2>\n"
                '<p style="color:#666;font-size:0.9em;margin:-10px 0 15px 0;">'
                "Coherent (phase-aware) sum of the L and R frequency "
                "responses, computed per mode from the channel curves "
                "in the JSON. Useful for spotting room-mode coupling and "
                "centre-channel coloration that is invisible when L and R "
                "are inspected separately."
                "</p>\n"
            )
        else:
            html_parts.append(f"<h2>Channel: {ch_name}</h2>\n")

        # Build mode_data for this channel. The synthetic L+R channel
        # is computed on the fly from each mode's L and R curves; all
        # other channels are read straight out of the JSON.
        mode_data: list[tuple[str, dict]] = []
        for mode_name, data in mode_datasets:
            channels = data.get("channels", {})
            if is_lr:
                lr = synthesize_lr_channel(channels.get("L"), channels.get("R"))
                if lr:
                    mode_data.append((mode_name, lr))
            else:
                ch_data = channels.get(ch_name, {})
                if ch_data:
                    mode_data.append((mode_name, ch_data))

        if not mode_data:
            html_parts.append("<p>No data for this channel.</p>\n</div>\n")
            continue

        # 1. Overlay plot (full range) + Zoomed (bass)
        fig_overlay = create_comparison_overlay_figure(ch_name, mode_data)
        fig_zoom = create_comparison_zoomed_figure(ch_name, mode_data)
        html_parts.append('<div class="plot-row">\n')
        html_parts.append(f'<div class="plot-container">{fig_overlay.to_html(full_html=False, include_plotlyjs=False)}</div>\n')
        html_parts.append(f'<div class="plot-container">{fig_zoom.to_html(full_html=False, include_plotlyjs=False)}</div>\n')
        html_parts.append("</div>\n")

        # 2. Phase before/after per mode
        fig_phase = create_comparison_phase_figure(ch_name, mode_data)
        if fig_phase:
            html_parts.append(f'<div class="plot-container">{fig_phase.to_html(full_html=False, include_plotlyjs=False)}</div>\n')

        # 3. Group delay before/after per mode
        fig_gd = create_comparison_group_delay_figure(ch_name, mode_data)
        if fig_gd:
            html_parts.append(f'<div class="plot-container">{fig_gd.to_html(full_html=False, include_plotlyjs=False)}</div>\n')

        # 4. Impulse response before/after per mode
        fig_ir = create_comparison_ir_figure(ch_name, mode_data)
        if fig_ir:
            html_parts.append(f'<div class="plot-container">{fig_ir.to_html(full_html=False, include_plotlyjs=False)}</div>\n')

        # 5. Per-mode subplots
        fig_subplots = create_mode_subplots_figure(ch_name, mode_data)
        html_parts.append(f'<div class="plot-container">{fig_subplots.to_html(full_html=False, include_plotlyjs=False)}</div>\n')

        # 6. EQ response overlay
        fig_eq = create_comparison_eq_overlay_figure(ch_name, mode_data)
        if fig_eq:
            html_parts.append(f'<div class="plot-container">{fig_eq.to_html(full_html=False, include_plotlyjs=False)}</div>\n')

        # 7. EPA psychoacoustic scores per mode
        epa_html = _epa_comparison_table_html(ch_name, mode_datasets)
        if epa_html:
            html_parts.append(epa_html)

        html_parts.append("</div>\n")

    html_parts.append("</div>\n</div>\n</body>\n</html>\n")

    with open(output_path, "w") as f:
        f.write("".join(html_parts))

    print(f"Comparison report written to: {output_path}")
