"""Plotly figure creation functions for roomeq visualization."""

import math

import plotly.graph_objects as go
from plotly.subplots import make_subplots

from . import SMOOTHING_OPTIONS, DEFAULT_SMOOTHING
from .dsp import (
    smooth_octave,
    compute_eq_response,
    generate_freq_points,
)
from .data_extract import (
    compute_y_range,
    compute_average_spl_in_range,
    get_all_crossover_frequencies,
)


def get_freq_axis_config() -> dict:
    """Get standardized frequency axis configuration with k notation."""
    return dict(
        title=dict(text="Frequency (Hz)", font=dict(size=11)),
        type="log",
        tickvals=[20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000],
        ticktext=["20", "50", "100", "200", "500", "1k", "2k", "5k", "10k", "20k"],
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
    )


def get_spl_axis_config(y_range: tuple[float, float]) -> dict:
    """Get standardized SPL axis configuration."""
    return dict(
        title=dict(text="SPL (dB)", font=dict(size=11)),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=list(y_range),
    )


def create_smoothing_buttons(
    n_traces: int, freq_data: list, spl_data_list: list[list]
) -> list[dict]:
    """
    Create dropdown buttons for smoothing selection.

    Args:
        n_traces: Number of traces to update
        freq_data: Frequency points (same for all traces)
        spl_data_list: List of raw SPL data for each trace

    Returns:
        List of button configurations for updatemenus
    """
    buttons = []

    for label, octave_frac in SMOOTHING_OPTIONS:
        # Compute smoothed data for each trace
        new_y_data = []
        for spl in spl_data_list:
            if spl is not None:
                smoothed = smooth_octave(freq_data, spl, octave_frac)
                new_y_data.append(smoothed)
            else:
                new_y_data.append(None)

        buttons.append(dict(label=label, method="update", args=[{"y": new_y_data}]))

    return buttons


def create_channel_figure(
    channel_name: str,
    initial_curve: dict | None,
    final_curve: dict | None,
    title_suffix: str = "",
) -> go.Figure:
    """Create a Plotly figure for a single channel with dynamic y-axis."""
    fig = go.Figure()

    freq_data = None
    spl_data_list = []

    # Add initial curve (before EQ)
    if initial_curve:
        freq_data = initial_curve["freq"]
        spl_raw = initial_curve["spl"]
        spl_smoothed = smooth_octave(freq_data, spl_raw, DEFAULT_SMOOTHING)
        fig.add_trace(
            go.Scatter(
                x=freq_data,
                y=spl_smoothed,
                mode="lines",
                name="Before EQ",
                line=dict(color="rgba(255, 100, 100, 0.8)", width=2),
            )
        )
        spl_data_list.append(spl_raw)
    else:
        spl_data_list.append(None)

    # Add final curve (after EQ)
    if final_curve:
        if freq_data is None:
            freq_data = final_curve["freq"]
        spl_raw = final_curve["spl"]
        spl_smoothed = smooth_octave(freq_data, spl_raw, DEFAULT_SMOOTHING)
        fig.add_trace(
            go.Scatter(
                x=final_curve["freq"],
                y=spl_smoothed,
                mode="lines",
                name="After EQ",
                line=dict(color="rgba(100, 200, 100, 0.9)", width=2),
            )
        )
        spl_data_list.append(spl_raw)
    else:
        spl_data_list.append(None)

    # Add target (flat at 0 dB)
    if initial_curve:
        freq = initial_curve["freq"]
        fig.add_trace(
            go.Scatter(
                x=[freq[0], freq[-1]],
                y=[0, 0],
                mode="lines",
                name="Target (0 dB)",
                line=dict(color="rgba(150, 150, 150, 0.5)", width=1, dash="dash"),
            )
        )

    # Compute dynamic y-range
    y_min, y_max = compute_y_range([initial_curve, final_curve])

    # Create smoothing buttons
    updatemenus = []
    if freq_data and any(s is not None for s in spl_data_list):
        buttons = create_smoothing_buttons(2, freq_data, spl_data_list)
        updatemenus = [
            dict(
                type="dropdown",
                direction="down",
                active=0,
                x=0.0,
                xanchor="left",
                y=1.15,
                yanchor="top",
                buttons=buttons,
                showactive=True,
                font=dict(size=10),
            )
        ]

    freq_axis = get_freq_axis_config()
    freq_axis["range"] = [1.3, 4.3]  # 20 Hz to 20 kHz in log scale

    fig.update_layout(
        title=dict(text=f"Channel: {channel_name}{title_suffix}", font=dict(size=14)),
        xaxis=freq_axis,
        yaxis=get_spl_axis_config((y_min, y_max)),
        legend=dict(yanchor="top", y=0.99, xanchor="right", x=0.99, font=dict(size=10)),
        plot_bgcolor="white",
        paper_bgcolor="white",
        margin=dict(l=60, r=40, t=80, b=60),
        height=400,
        updatemenus=updatemenus,
    )

    return fig


def create_zoomed_figure(
    channel_name: str,
    initial_curve: dict | None,
    final_curve: dict | None,
    min_freq: float = 20.0,
    max_freq: float = 1200.0,
    y_range: float = 10.0,
) -> go.Figure:
    """Create a zoomed Plotly figure for a single channel (20-1200Hz, centered y-axis)."""
    fig = go.Figure()

    # Compute average SPL for centering (use final curve if available, else initial)
    ref_curve = final_curve if final_curve else initial_curve
    avg_spl = (
        compute_average_spl_in_range(ref_curve, min_freq, max_freq)
        if ref_curve
        else 0.0
    )

    freq_data = None
    spl_data_list = []

    # Add initial curve (before EQ)
    if initial_curve:
        freq_data = initial_curve["freq"]
        spl_raw = initial_curve["spl"]
        spl_smoothed = smooth_octave(freq_data, spl_raw, DEFAULT_SMOOTHING)
        fig.add_trace(
            go.Scatter(
                x=freq_data,
                y=spl_smoothed,
                mode="lines",
                name="Before EQ",
                line=dict(color="rgba(255, 100, 100, 0.8)", width=2),
            )
        )
        spl_data_list.append(spl_raw)
    else:
        spl_data_list.append(None)

    # Add final curve (after EQ)
    if final_curve:
        if freq_data is None:
            freq_data = final_curve["freq"]
        spl_raw = final_curve["spl"]
        spl_smoothed = smooth_octave(freq_data, spl_raw, DEFAULT_SMOOTHING)
        fig.add_trace(
            go.Scatter(
                x=final_curve["freq"],
                y=spl_smoothed,
                mode="lines",
                name="After EQ",
                line=dict(color="rgba(100, 200, 100, 0.9)", width=2),
            )
        )
        spl_data_list.append(spl_raw)
    else:
        spl_data_list.append(None)

    # Add target line at average
    fig.add_trace(
        go.Scatter(
            x=[min_freq, max_freq],
            y=[avg_spl, avg_spl],
            mode="lines",
            name=f"Average ({avg_spl:.1f} dB)",
            line=dict(color="rgba(150, 150, 150, 0.5)", width=1, dash="dash"),
        )
    )

    # Log scale range for 20-1200 Hz
    log_min = math.log10(min_freq)
    log_max = math.log10(max_freq)

    # Create smoothing buttons
    updatemenus = []
    if freq_data and any(s is not None for s in spl_data_list):
        buttons = create_smoothing_buttons(2, freq_data, spl_data_list)
        updatemenus = [
            dict(
                type="dropdown",
                direction="down",
                active=0,
                x=0.0,
                xanchor="left",
                y=1.15,
                yanchor="top",
                buttons=buttons,
                showactive=True,
                font=dict(size=10),
            )
        ]

    freq_axis = get_freq_axis_config()
    freq_axis["range"] = [log_min, log_max]
    freq_axis["tickvals"] = [20, 50, 100, 200, 500, 1000]
    freq_axis["ticktext"] = ["20", "50", "100", "200", "500", "1k"]

    fig.update_layout(
        title=dict(
            text=f"Channel: {channel_name} (Zoom {int(min_freq)}-{int(max_freq)} Hz)",
            font=dict(size=14),
        ),
        xaxis=freq_axis,
        yaxis=dict(
            title=dict(text="SPL (dB)", font=dict(size=11)),
            tickfont=dict(size=10),
            gridcolor="rgba(128, 128, 128, 0.2)",
            range=[avg_spl - y_range, avg_spl + y_range],
            dtick=5,
        ),
        legend=dict(yanchor="top", y=0.99, xanchor="right", x=0.99, font=dict(size=10)),
        plot_bgcolor="white",
        paper_bgcolor="white",
        margin=dict(l=60, r=40, t=80, b=60),
        height=400,
        updatemenus=updatemenus,
    )

    return fig


def create_eq_figure(
    channel_name: str,
    eq_filters: list[dict],
    eq_response_data: dict | None = None,
) -> go.Figure | None:
    """Create a Plotly figure showing the EQ frequency response.

    Args:
        channel_name: Name of the channel.
        eq_filters: List of EQ filter dicts (for individual filter decomposition).
        eq_response_data: Optional pre-computed EQ response from JSON output
            (with 'freq' and 'spl' keys). When provided, used for the combined
            EQ curve instead of recomputing from biquad filters.
    """
    if not eq_filters and not eq_response_data:
        return None

    # Use pre-computed EQ response from JSON if available, otherwise compute from filters
    if eq_response_data and "freq" in eq_response_data and "spl" in eq_response_data:
        freq_points = eq_response_data["freq"]
        eq_response = eq_response_data["spl"]
    else:
        freq_points = generate_freq_points(20.0, 20000.0, 500)
        eq_response = compute_eq_response(eq_filters, freq_points)

    if not eq_response:
        return None

    fig = go.Figure()

    # Add combined EQ response
    fig.add_trace(
        go.Scatter(
            x=freq_points,
            y=eq_response,
            mode="lines",
            name="Combined EQ",
            line=dict(color="rgba(100, 100, 255, 0.9)", width=2),
        )
    )

    # Add individual filter responses
    colors = [
        "rgba(255, 150, 150, 0.6)",
        "rgba(150, 255, 150, 0.6)",
        "rgba(150, 150, 255, 0.6)",
        "rgba(255, 255, 150, 0.6)",
        "rgba(255, 150, 255, 0.6)",
        "rgba(150, 255, 255, 0.6)",
        "rgba(200, 200, 200, 0.6)",
    ]

    for i, filt in enumerate(eq_filters):
        single_response = compute_eq_response([filt], freq_points)
        freq = filt.get("freq", 0)
        gain = filt.get("db_gain", 0)
        filter_type = filt.get("filter_type", "peak")

        fig.add_trace(
            go.Scatter(
                x=freq_points,
                y=single_response,
                mode="lines",
                name=f"{filter_type.upper()} {freq:.0f}Hz {gain:+.1f}dB",
                line=dict(color=colors[i % len(colors)], width=1, dash="dot"),
            )
        )

    # Add 0 dB reference line
    fig.add_trace(
        go.Scatter(
            x=[freq_points[0], freq_points[-1]],
            y=[0, 0],
            mode="lines",
            name="0 dB",
            line=dict(color="rgba(150, 150, 150, 0.5)", width=1, dash="dash"),
        )
    )

    # Compute y-range from EQ response
    y_limit: int | None = None
    y_min = -15
    y_max = 15

    if eq_response:
        is_lfe = "lfe" in channel_name.lower()
        if is_lfe:
            lfe_spl = []
            for f, s in zip(freq_points, eq_response):
                if 20 <= f <= 200:
                    lfe_spl.append(s)
            if lfe_spl:
                max_abs = max(abs(min(lfe_spl)), abs(max(lfe_spl)))
                y_limit = max(10, math.ceil(max_abs / 5) * 5 + 5)
            else:
                y_limit = 15
        else:
            eq_max = max(eq_response)
            eq_min = min(eq_response)
            y_max = math.ceil(eq_max / 5) * 5
            y_min = math.floor(eq_min / 5) * 5
            if y_max - y_min < 50:
                y_min = y_max - 50
            y_limit = None
    else:
        y_limit = 15

    # Compute y_range for plot
    if y_limit is not None:
        y_range = [-y_limit, y_limit]
    else:
        y_range = [y_min, y_max]

    freq_axis = get_freq_axis_config()
    freq_axis["range"] = [1.3, 4.3]

    fig.update_layout(
        title=dict(text=f"EQ Response: {channel_name}", font=dict(size=14)),
        xaxis=freq_axis,
        yaxis=dict(
            title=dict(text="Gain (dB)", font=dict(size=11)),
            tickfont=dict(size=10),
            gridcolor="rgba(128, 128, 128, 0.2)",
            range=y_range,
            dtick=5,
        ),
        legend=dict(yanchor="top", y=0.99, xanchor="right", x=0.99, font=dict(size=10)),
        plot_bgcolor="white",
        paper_bgcolor="white",
        margin=dict(l=60, r=40, t=60, b=60),
        height=400,
    )

    return fig


def _get_driver_initial_curves(channel_data: dict) -> list[tuple[str, dict]] | None:
    """Extract per-driver initial curves from a channel's driver chains.

    Returns:
        List of (driver_name, curve_data) tuples, or None if no per-driver curves exist.
        Each curve_data has "freq" and "spl" keys.
    """
    drivers = channel_data.get("drivers", [])
    if not drivers:
        return None

    result = []
    for driver in drivers:
        initial_curve = driver.get("initial_curve")
        if initial_curve and "freq" in initial_curve and "spl" in initial_curve:
            name = driver.get("name", f"driver_{driver.get('index', '?')}")
            result.append((name, initial_curve))

    return result if result else None


def create_combined_figure(data: dict, json_path: "None | object" = None) -> go.Figure:
    """Create a combined figure with 3-row subplots: Original, EQ, Corrected.

    Args:
        data: Output JSON data (roomeq result with correction filters)
        json_path: Path to output JSON (unused, kept for API compatibility)
    """
    channels = data.get("channels", {})

    if not channels:
        print("Warning: No channels found in the JSON file")
        return go.Figure()

    # Create 3-row subplot layout
    fig = make_subplots(
        rows=3,
        cols=1,
        subplot_titles=[
            "All Original Curves",
            "All EQ Responses",
            "All Corrected Curves",
        ],
        vertical_spacing=0.08,
        specs=[[{}], [{}], [{}]],
    )

    # Color palette for channels
    channel_colors = [
        "rgba(31, 119, 180, 0.9)",  # blue
        "rgba(255, 127, 14, 0.9)",  # orange
        "rgba(44, 160, 44, 0.9)",  # green
        "rgba(214, 39, 40, 0.9)",  # red
        "rgba(148, 103, 189, 0.9)",  # purple
        "rgba(140, 86, 75, 0.9)",  # brown
        "rgba(227, 119, 194, 0.9)",  # pink
        "rgba(127, 127, 127, 0.9)",  # gray
    ]

    # Generate frequency points for EQ response
    freq_points = generate_freq_points(20.0, 20000.0, 500)

    # Driver line dash patterns for distinguishing drivers within a channel
    driver_dashes = ["solid", "dash", "dot", "dashdot"]

    # Collect all curves for y-range computation
    all_initial_curves: list[dict | None] = []
    all_corrected_curves: list[dict | None] = []

    # Per-driver data for channels that are speaker groups
    per_driver_initial: dict[str, list[tuple[str, dict]]] = {}

    for channel_name, channel_data in channels.items():
        driver_curves = _get_driver_initial_curves(channel_data)
        if driver_curves:
            per_driver_initial[channel_name] = driver_curves
            for _, dcurve in driver_curves:
                all_initial_curves.append(dcurve)
        else:
            all_initial_curves.append(channel_data.get("initial_curve"))

        final_curve = channel_data.get("final_curve")
        if final_curve and "freq" in final_curve and "spl" in final_curve:
            all_corrected_curves.append(final_curve)

    # Compute y-ranges
    initial_y_min, initial_y_max = compute_y_range(all_initial_curves)
    corrected_y_min, corrected_y_max = compute_y_range(all_corrected_curves)

    # Compute EQ y-range
    all_eq_values: list[float] = []
    for channel_data in channels.values():
        eq_response_data = channel_data.get("eq_response")
        if eq_response_data and "spl" in eq_response_data:
            all_eq_values.extend(eq_response_data["spl"])
        else:
            plugins = channel_data.get("plugins", [])
            eq_filters = []
            for plugin in plugins:
                if plugin.get("plugin_type") == "eq":
                    filters = plugin.get("parameters", {}).get("filters", [])
                    eq_filters.extend(filters)
            if eq_filters:
                eq_resp = compute_eq_response(eq_filters, freq_points)
                all_eq_values.extend(eq_resp)

    if all_eq_values:
        eq_y_upper = min(20, math.ceil(max(all_eq_values) / 5) * 5 + 5)
        eq_y_lower = max(-20, math.floor(min(all_eq_values) / 5) * 5 - 5)
    else:
        eq_y_upper = 15
        eq_y_lower = -15

    # Track trace indices and raw data for smoothing
    trace_y_data: list = []
    original_freq_list: list[list[float]] = []
    original_raw_spl: list[list[float]] = []
    original_trace_indices: list[int] = []
    corrected_freq_list: list[list[float]] = []
    corrected_raw_spl: list[list[float]] = []
    corrected_trace_indices: list[int] = []

    # --- Row 1: Original curves ---
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]

        if channel_name in per_driver_initial:
            for d_idx, (driver_name, dcurve) in enumerate(
                per_driver_initial[channel_name]
            ):
                spl_raw = dcurve["spl"]
                spl_smoothed = smooth_octave(dcurve["freq"], spl_raw, DEFAULT_SMOOTHING)

                original_trace_indices.append(len(trace_y_data))
                original_freq_list.append(dcurve["freq"])
                original_raw_spl.append(spl_raw)

                fig.add_trace(
                    go.Scatter(
                        x=dcurve["freq"],
                        y=spl_smoothed,
                        mode="lines",
                        name=f"Original: {channel_name}/{driver_name}",
                        line=dict(
                            color=color,
                            width=2,
                            dash=driver_dashes[d_idx % len(driver_dashes)],
                        ),
                        legendgroup=f"ch_{channel_name}",
                    ),
                    row=1,
                    col=1,
                )
                trace_y_data.append(spl_smoothed)
        else:
            initial_curve = channel_data.get("initial_curve")
            if initial_curve:
                spl_raw = initial_curve["spl"]
                spl_smoothed = smooth_octave(
                    initial_curve["freq"], spl_raw, DEFAULT_SMOOTHING
                )

                original_trace_indices.append(len(trace_y_data))
                original_freq_list.append(initial_curve["freq"])
                original_raw_spl.append(spl_raw)

                fig.add_trace(
                    go.Scatter(
                        x=initial_curve["freq"],
                        y=spl_smoothed,
                        mode="lines",
                        name=f"Original: {channel_name}",
                        line=dict(color=color, width=2),
                        legendgroup=f"ch_{channel_name}",
                    ),
                    row=1,
                    col=1,
                )
                trace_y_data.append(spl_smoothed)

    # Target line on original curves
    if channels:
        first_channel = next(iter(channels.values()))
        ref_curve = first_channel.get("initial_curve")
        if ref_curve:
            freq = ref_curve["freq"]
            fig.add_trace(
                go.Scatter(
                    x=[freq[0], freq[-1]],
                    y=[0, 0],
                    mode="lines",
                    name="Target (0 dB)",
                    line=dict(color="rgba(150, 150, 150, 0.5)", width=1, dash="dash"),
                    showlegend=False,
                ),
                row=1,
                col=1,
            )
            trace_y_data.append([0, 0])

    # --- Row 2: EQ responses ---
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]

        eq_response_data = channel_data.get("eq_response")
        if (
            eq_response_data
            and "freq" in eq_response_data
            and "spl" in eq_response_data
        ):
            eq_freq = eq_response_data["freq"]
            eq_spl = eq_response_data["spl"]
        else:
            plugins = channel_data.get("plugins", [])
            eq_filters = []
            for plugin in plugins:
                if plugin.get("plugin_type") == "eq":
                    filters = plugin.get("parameters", {}).get("filters", [])
                    eq_filters.extend(filters)
            eq_freq = freq_points if eq_filters else None
            eq_spl = (
                compute_eq_response(eq_filters, freq_points) if eq_filters else None
            )

        if eq_spl:
            fig.add_trace(
                go.Scatter(
                    x=eq_freq,
                    y=eq_spl,
                    mode="lines",
                    name=f"EQ: {channel_name}",
                    line=dict(color=color, width=2),
                    legendgroup=f"ch_{channel_name}",
                    showlegend=False,
                ),
                row=2,
                col=1,
            )
            trace_y_data.append(eq_spl)

    # 0 dB reference on EQ plot
    fig.add_trace(
        go.Scatter(
            x=[freq_points[0], freq_points[-1]],
            y=[0, 0],
            mode="lines",
            name="0 dB",
            line=dict(color="rgba(150, 150, 150, 0.5)", width=1, dash="dash"),
            showlegend=False,
        ),
        row=2,
        col=1,
    )
    trace_y_data.append([0, 0])

    # --- Row 3: Corrected curves (final_curve from JSON) ---
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]
        final_curve = channel_data.get("final_curve")

        if final_curve and "freq" in final_curve and "spl" in final_curve:
            freq = final_curve["freq"]
            spl_raw = final_curve["spl"]

            spl_smoothed = smooth_octave(freq, spl_raw, DEFAULT_SMOOTHING)

            corrected_trace_indices.append(len(trace_y_data))
            corrected_freq_list.append(freq)
            corrected_raw_spl.append(spl_raw)

            fig.add_trace(
                go.Scatter(
                    x=freq,
                    y=spl_smoothed,
                    mode="lines",
                    name=f"Corrected: {channel_name}",
                    line=dict(color=color, width=2),
                    legendgroup=f"ch_{channel_name}",
                    showlegend=False,
                ),
                row=3,
                col=1,
            )
            trace_y_data.append(spl_smoothed)

    # Target line on corrected curves
    if all_corrected_curves:
        first_corrected = all_corrected_curves[0]
        if first_corrected:
            freq = first_corrected["freq"]
            fig.add_trace(
                go.Scatter(
                    x=[freq[0], freq[-1]],
                    y=[0, 0],
                    mode="lines",
                    name="Target (0 dB)",
                    line=dict(color="rgba(150, 150, 150, 0.5)", width=1, dash="dash"),
                    showlegend=False,
                ),
                row=3,
                col=1,
            )
            trace_y_data.append([0, 0])

    # --- Crossover vertical lines on all 3 rows ---
    crossover_freqs = get_all_crossover_frequencies(data)
    row_ranges = [
        (1, initial_y_min, initial_y_max),
        (2, eq_y_lower, eq_y_upper),
        (3, corrected_y_min, corrected_y_max),
    ]
    for xover_freq in crossover_freqs:
        freq_label = (
            f"{xover_freq / 1000:.1f}k" if xover_freq >= 1000 else f"{xover_freq:.0f}"
        )

        for row_idx, (row, y_lo, y_hi) in enumerate(row_ranges):
            fig.add_trace(
                go.Scatter(
                    x=[xover_freq, xover_freq],
                    y=[y_lo, y_hi],
                    mode="lines",
                    name=f"Xover {freq_label} Hz",
                    line=dict(
                        color="rgba(180, 80, 180, 0.7)", width=1.5, dash="dashdot"
                    ),
                    showlegend=(row_idx == 0),
                    legendgroup="crossover",
                ),
                row=row,
                col=1,
            )
            trace_y_data.append([y_lo, y_hi])

    # --- Smoothing dropdown ---
    updatemenus = []
    has_smoothable = (corrected_freq_list and corrected_raw_spl) or (
        original_freq_list and original_raw_spl
    )
    if has_smoothable:
        buttons = []
        for label, octave_frac in SMOOTHING_OPTIONS:
            new_y_data = []
            original_idx = 0
            corrected_idx = 0
            for trace_idx, y_data in enumerate(trace_y_data):
                if (
                    trace_idx in original_trace_indices
                    and original_freq_list
                ):
                    smoothed = smooth_octave(
                        original_freq_list[original_idx], original_raw_spl[original_idx], octave_frac
                    )
                    new_y_data.append(smoothed)
                    original_idx += 1
                elif (
                    trace_idx in corrected_trace_indices
                    and corrected_freq_list
                ):
                    smoothed = smooth_octave(
                        corrected_freq_list[corrected_idx],
                        corrected_raw_spl[corrected_idx],
                        octave_frac,
                    )
                    new_y_data.append(smoothed)
                    corrected_idx += 1
                else:
                    new_y_data.append(y_data)

            buttons.append(dict(label=label, method="update", args=[{"y": new_y_data}]))

        updatemenus = [
            dict(
                type="dropdown",
                direction="down",
                active=0,
                x=0.0,
                xanchor="left",
                y=0.60,
                yanchor="top",
                buttons=buttons,
                showactive=True,
                font=dict(size=10),
            )
        ]

    # --- Axis configuration ---
    for row in [1, 2, 3]:
        fig.update_xaxes(
            type="log",
            tickvals=[20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000],
            ticktext=["20", "50", "100", "200", "500", "1k", "2k", "5k", "10k", "20k"],
            tickfont=dict(size=10),
            gridcolor="rgba(128, 128, 128, 0.2)",
            range=[1.3, 4.3],
            row=row,
            col=1,
        )

    fig.update_yaxes(
        title_text="SPL (dB)",
        title_font=dict(size=11),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=[initial_y_min, initial_y_max],
        row=1,
        col=1,
    )
    fig.update_yaxes(
        title_text="Gain (dB)",
        title_font=dict(size=11),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=[eq_y_lower, eq_y_upper],
        dtick=5,
        row=2,
        col=1,
    )
    fig.update_yaxes(
        title_text="SPL (dB)",
        title_font=dict(size=11),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=[corrected_y_min, corrected_y_max],
        row=3,
        col=1,
    )

    fig.update_layout(
        height=950,
        plot_bgcolor="white",
        paper_bgcolor="white",
        legend=dict(
            orientation="h",
            yanchor="bottom",
            y=1.0,
            xanchor="left",
            x=0.0,
            font=dict(size=10),
        ),
        margin=dict(l=60, r=60, t=110, b=60),
        updatemenus=updatemenus,
    )

    return fig
