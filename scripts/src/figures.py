"""Plotly figure creation functions for roomeq visualization."""

import math
from pathlib import Path

import numpy as np
import plotly.graph_objects as go
from plotly.subplots import make_subplots

from . import SMOOTHING_OPTIONS, DEFAULT_SMOOTHING
from .dsp import (
    smooth_octave,
    compute_eq_response,
    generate_freq_points,
    compute_minimum_phase_from_spl,
    compute_corrected_ir,
    compute_driver_correction_db,
    compute_plugin_chain_response,
)
from .data_extract import (
    compute_y_range,
    compute_average_spl_in_range,
    get_all_ir_wav_paths,
    get_driver_initial_curves,
    get_all_crossover_frequencies,
    get_summing_groups,
)
from .input_parser import extract_channel_measurement_paths
from .loaders import load_wav_file, load_measurement_ir


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


def create_smoothing_buttons(n_traces: int, freq_data: list, spl_data_list: list[list]) -> list[dict]:
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

        buttons.append(dict(
            label=label,
            method="update",
            args=[{"y": new_y_data}]
        ))

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
        updatemenus = [dict(
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
        )]

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
    avg_spl = compute_average_spl_in_range(ref_curve, min_freq, max_freq)

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
        updatemenus = [dict(
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
        )]

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
    if eq_response:
        max_abs = max(abs(min(eq_response)), abs(max(eq_response)))
        y_limit = max(15, math.ceil(max_abs / 5) * 5 + 5)
    else:
        y_limit = 15

    freq_axis = get_freq_axis_config()
    freq_axis["range"] = [1.3, 4.3]

    fig.update_layout(
        title=dict(text=f"EQ Response: {channel_name}", font=dict(size=14)),
        xaxis=freq_axis,
        yaxis=dict(
            title=dict(text="Gain (dB)", font=dict(size=11)),
            tickfont=dict(size=10),
            gridcolor="rgba(128, 128, 128, 0.2)",
            range=[-y_limit, y_limit],
            dtick=5,
        ),
        legend=dict(yanchor="top", y=0.99, xanchor="right", x=0.99, font=dict(size=10)),
        plot_bgcolor="white",
        paper_bgcolor="white",
        margin=dict(l=60, r=40, t=60, b=60),
        height=400,
    )

    return fig


def create_combined_figure(
    data: dict, json_path: Path | None = None, input_data: dict | None = None, input_path: Path | None = None
) -> go.Figure:
    """Create a combined figure with subplots: EQs, corrected curves, and impulse responses.

    Args:
        data: Output JSON data (roomeq result with correction filters)
        json_path: Path to output JSON (for resolving relative paths)
        input_data: Optional input JSON data (roomeq config with measurement file paths)
        input_path: Optional path to input JSON (for resolving relative paths)

    When input_data is provided, shows corrected room IRs (original measurement
    convolved with corrections). Supports both WAV and CSV measurement files.
    Otherwise, shows FIR correction filter IRs if available.
    """
    channels = data.get("channels", {})

    if not channels:
        print("Warning: No channels found in the JSON file")
        return go.Figure()

    channel_names = list(channels.keys())
    n_channels = len(channel_names)

    # Determine directories for path resolution
    json_dir = json_path.parent if json_path else Path(".")
    input_dir = input_path.parent if input_path else json_dir

    # Check for corrected IR mode (when input_data provided)
    measurement_paths = {}
    if input_data:
        measurement_paths = extract_channel_measurement_paths(input_data, input_dir)
        if measurement_paths:
            print(f"Found {len(measurement_paths)} measurement file(s) for corrected IR computation")
        else:
            print("No measurement paths found in input JSON")

    # Check if we have data for impulse responses
    # Priority: corrected IRs (from input) > FIR correction filters (from output)
    ir_wav_paths = get_all_ir_wav_paths(data, json_dir)
    has_corrected_ir = bool(measurement_paths)
    has_fir_ir = bool(ir_wav_paths)
    has_ir_data = has_corrected_ir or has_fir_ir

    # Determine summing groups (e.g., L+LFE, R+LFE for 2.1)
    summing_groups = get_summing_groups(input_data, channel_names)
    has_summing = bool(summing_groups)
    sum_row = 4  # row for summed curves (always after corrected)
    ir_row = sum_row + (1 if has_summing else 0)  # IR row shifts if summing row present

    # Create subplot layout based on available data
    titles = ["All Original Curves", "All EQ Responses", "All Corrected Curves"]
    specs: list[list[dict]] = [[{"secondary_y": True}], [{}], [{"secondary_y": True}]]
    if has_summing:
        titles.append("Summed Curves (Listening Position)")
        specs.append([{"secondary_y": True}])
    if has_ir_data:
        ir_title = "Corrected Room Impulse Responses" if has_corrected_ir else "FIR Correction Filters"
        titles.append(ir_title)
        specs.append([{}])
    fig = make_subplots(
        rows=len(titles),
        cols=1,
        subplot_titles=titles,
        vertical_spacing=0.05,
        specs=specs,
    )

    # Color palette for channels
    channel_colors = [
        "rgba(31, 119, 180, 0.9)",   # blue
        "rgba(255, 127, 14, 0.9)",   # orange
        "rgba(44, 160, 44, 0.9)",    # green
        "rgba(214, 39, 40, 0.9)",    # red
        "rgba(148, 103, 189, 0.9)",  # purple
        "rgba(140, 86, 75, 0.9)",    # brown
        "rgba(227, 119, 194, 0.9)",  # pink
        "rgba(127, 127, 127, 0.9)",  # gray
    ]

    # Generate frequency points for EQ response
    freq_points = generate_freq_points(20.0, 20000.0, 500)

    # Collect all curves and pre-compute full-range corrected curves
    all_initial_curves = []
    all_eq_responses = []
    all_chain_responses = []
    corrected_curves = {}  # channel_name -> {"freq": [...], "spl": [...]}
    # Per-driver data for channels that are speaker groups
    # channel_name -> list of (driver_name, initial_curve_data)
    per_driver_initial: dict[str, list[tuple[str, dict]]] = {}
    # channel_name -> list of (driver_name, corrected_curve_data)
    per_driver_corrected: dict[str, list[tuple[str, dict]]] = {}

    for channel_name, channel_data in channels.items():
        # Check for per-driver initial curves
        driver_curves = get_driver_initial_curves(channel_data)
        if driver_curves:
            per_driver_initial[channel_name] = driver_curves
            # Use per-driver curves for y-range (these are what gets plotted)
            for _, dcurve in driver_curves:
                all_initial_curves.append(dcurve)
        else:
            # Only include combined curve when it's the one being plotted
            all_initial_curves.append(channel_data.get("initial_curve"))

        # Get EQ filters for this channel
        plugins = channel_data.get("plugins", [])
        eq_filters = []
        for plugin in plugins:
            if plugin.get("plugin_type") == "eq":
                filters = plugin.get("parameters", {}).get("filters", [])
                eq_filters.extend(filters)

        eq_response_data = channel_data.get("eq_response")
        if eq_response_data and "freq" in eq_response_data and "spl" in eq_response_data:
            all_eq_responses.append(eq_response_data["spl"])
        elif eq_filters:
            eq_response = compute_eq_response(eq_filters, freq_points)
            all_eq_responses.append(eq_response)

        # Pre-compute full chain response for EQ subplot y-range
        chain_response = compute_plugin_chain_response(
            plugins, freq_points, json_dir=json_dir
        )
        all_chain_responses.append(chain_response)

        # Use final_curve from the output JSON (pre-computed by the optimizer)
        final_curve = channel_data.get("final_curve")
        if final_curve and "freq" in final_curve and "spl" in final_curve:
            corrected_curves[channel_name] = final_curve

        # Compute per-driver corrected curves
        if driver_curves:
            drivers = channel_data.get("drivers", [])
            driver_corrected_list = []
            for driver_idx, (driver_name, dcurve) in enumerate(driver_curves):
                dfreq = dcurve["freq"]
                dspl = dcurve["spl"]
                # Find the matching driver dict by index
                driver_dict = drivers[driver_idx] if driver_idx < len(drivers) else {}
                corr_db = compute_driver_correction_db(driver_dict, plugins, dfreq)
                corrected_dspl = [s + c for s, c in zip(dspl, corr_db)]
                driver_corrected_list.append(
                    (driver_name, {"freq": dfreq, "spl": corrected_dspl})
                )
            per_driver_corrected[channel_name] = driver_corrected_list

    # Compute y-ranges: max SPL rounded up to next multiple of 5, range of 50 dB
    def _compute_fixed_range(curves):
        all_spl = []
        for curve in curves:
            if curve and "spl" in curve:
                all_spl.extend(curve["spl"])
        if not all_spl:
            return (-20, 30)
        upper = math.ceil(max(all_spl) / 5) * 5
        return (upper - 50, upper)

    initial_y_min, initial_y_max = _compute_fixed_range(all_initial_curves)
    all_corrected_for_range = list(corrected_curves.values())
    for dc_list in per_driver_corrected.values():
        for _, dc in dc_list:
            all_corrected_for_range.append(dc)
    corrected_y_min, corrected_y_max = _compute_fixed_range(all_corrected_for_range)

    # Compute EQ y-range (include full chain responses for proper scaling)
    # Upper bound clamped to 20 dB; lower bound can extend further
    all_row2_values = [v for resp in all_eq_responses for v in resp]
    all_row2_values.extend(v for resp in all_chain_responses for v in resp)
    if all_row2_values:
        eq_y_upper = min(20, math.ceil(max(all_row2_values) / 5) * 5 + 5)
        eq_y_lower = max(-20, math.floor(min(all_row2_values) / 5) * 5 - 5)
    else:
        eq_y_upper = 15
        eq_y_lower = -15

    # Track trace indices and raw data for smoothing
    trace_y_data = []  # Will hold y-data for each trace (None for non-smoothable)
    original_freq_data = None  # Frequency data for original curves
    original_raw_spl = []  # Raw SPL data for each original curve
    original_trace_indices = []  # Indices of original curve traces
    original_phase_trace_indices = []  # Indices of original phase traces
    original_raw_phase = []  # Raw phase data for each original curve
    original_phase_freq = []  # Frequency data for each original phase curve
    corrected_freq_data = None  # Frequency data for corrected curves
    corrected_raw_spl = []  # Raw SPL data for each corrected curve
    corrected_trace_indices = []  # Indices of corrected curve traces
    corrected_phase_trace_indices = []  # Indices of corrected phase traces
    corrected_raw_phase = []  # Raw phase data for each corrected curve
    corrected_phase_freq = []  # Frequency data for each corrected phase curve

    # Driver line dash patterns for distinguishing drivers within a channel
    driver_dashes = ["solid", "dash", "dot", "dashdot"]

    # Plot original curves (row 1)
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]

        if channel_name in per_driver_initial:
            # Plot individual driver curves instead of combined
            for d_idx, (driver_name, dcurve) in enumerate(per_driver_initial[channel_name]):
                if original_freq_data is None:
                    original_freq_data = dcurve["freq"]

                spl_raw = dcurve["spl"]
                spl_smoothed = smooth_octave(dcurve["freq"], spl_raw, DEFAULT_SMOOTHING)

                original_trace_indices.append(len(trace_y_data))
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
            # Single-driver channel: plot combined curve
            initial_curve = channel_data.get("initial_curve")
            if initial_curve:
                if original_freq_data is None:
                    original_freq_data = initial_curve["freq"]

                spl_raw = initial_curve["spl"]
                spl_smoothed = smooth_octave(initial_curve["freq"], spl_raw, DEFAULT_SMOOTHING)

                original_trace_indices.append(len(trace_y_data))
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

    # Add target line to original curves plot
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

    # Add phase traces for original curves (row 1, secondary y-axis)
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]
        phase_color = color.replace("0.9)", "0.5)")
        initial_curve = channel_data.get("initial_curve")
        if initial_curve:
            phase_raw = compute_minimum_phase_from_spl(initial_curve["freq"], initial_curve["spl"])
            if phase_raw is not None:
                phase_smoothed = smooth_octave(initial_curve["freq"], phase_raw, DEFAULT_SMOOTHING)
                original_phase_trace_indices.append(len(trace_y_data))
                original_raw_phase.append(phase_raw)
                original_phase_freq.append(initial_curve["freq"])
                fig.add_trace(
                    go.Scatter(
                        x=initial_curve["freq"],
                        y=phase_smoothed,
                        mode="lines",
                        name=f"Phase: {channel_name}",
                        line=dict(color=phase_color, width=1, dash="dot"),
                        legendgroup=f"ch_{channel_name}",
                        showlegend=False,
                    ),
                    row=1, col=1, secondary_y=True,
                )
                trace_y_data.append(phase_smoothed)

    # Plot EQ responses and full chain responses (row 2)
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]

        plugins = channel_data.get("plugins", [])

        eq_response_data = channel_data.get("eq_response")
        if eq_response_data and "freq" in eq_response_data and "spl" in eq_response_data:
            eq_freq = eq_response_data["freq"]
            eq_spl = eq_response_data["spl"]
        else:
            eq_filters = []
            for plugin in plugins:
                if plugin.get("plugin_type") == "eq":
                    filters = plugin.get("parameters", {}).get("filters", [])
                    eq_filters.extend(filters)
            eq_freq = freq_points if eq_filters else None
            eq_spl = compute_eq_response(eq_filters, freq_points) if eq_filters else None

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

        # Full chain response (gain + crossover + EQ + convolution)
        chain_response = compute_plugin_chain_response(
            plugins, freq_points, json_dir=json_dir
        )
        has_non_eq = any(
            p.get("plugin_type") in ("gain", "crossover", "convolution")
            for p in plugins
        )
        if has_non_eq:
            # Use semi-transparent version of channel color for the chain trace
            chain_color = color.replace("0.9)", "0.5)")
            fig.add_trace(
                go.Scatter(
                    x=freq_points,
                    y=chain_response,
                    mode="lines",
                    name=f"Full Chain: {channel_name}",
                    line=dict(color=chain_color, width=1.5, dash="dash"),
                    legendgroup=f"ch_{channel_name}",
                    showlegend=True,
                ),
                row=2,
                col=1,
            )
            trace_y_data.append(chain_response)

    # Add 0 dB reference line to EQ plot
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

    # Plot corrected curves (row 3) - full 20-20k Hz range using initial + EQ
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]

        if channel_name in per_driver_corrected:
            # Plot individual driver corrected curves
            for d_idx, (driver_name, dcurve) in enumerate(per_driver_corrected[channel_name]):
                freq = dcurve["freq"]
                spl_raw = dcurve["spl"]

                if corrected_freq_data is None:
                    corrected_freq_data = freq

                spl_smoothed = smooth_octave(freq, spl_raw, DEFAULT_SMOOTHING)

                corrected_trace_indices.append(len(trace_y_data))
                corrected_raw_spl.append(spl_raw)

                fig.add_trace(
                    go.Scatter(
                        x=freq,
                        y=spl_smoothed,
                        mode="lines",
                        name=f"Corrected: {channel_name}/{driver_name}",
                        line=dict(
                            color=color,
                            width=2,
                            dash=driver_dashes[d_idx % len(driver_dashes)],
                        ),
                        legendgroup=f"ch_{channel_name}",
                        showlegend=False,
                    ),
                    row=3,
                    col=1,
                )
                trace_y_data.append(spl_smoothed)
        elif channel_name in corrected_curves:
            # Single-driver channel: plot combined corrected curve
            curve_data = corrected_curves[channel_name]
            freq = curve_data["freq"]
            spl_raw = curve_data["spl"]

            if corrected_freq_data is None:
                corrected_freq_data = freq

            spl_smoothed = smooth_octave(freq, spl_raw, DEFAULT_SMOOTHING)

            corrected_trace_indices.append(len(trace_y_data))
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

    # Add target line to corrected curves plot
    if corrected_curves:
        first_corrected = next(iter(corrected_curves.values()))
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

    # Add phase traces for corrected curves (row 3, secondary y-axis)
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]
        phase_color = color.replace("0.9)", "0.5)")
        if channel_name in corrected_curves:
            curve_data = corrected_curves[channel_name]
            phase_raw = compute_minimum_phase_from_spl(curve_data["freq"], curve_data["spl"])
            if phase_raw is not None:
                phase_smoothed = smooth_octave(curve_data["freq"], phase_raw, DEFAULT_SMOOTHING)
                corrected_phase_trace_indices.append(len(trace_y_data))
                corrected_raw_phase.append(phase_raw)
                corrected_phase_freq.append(curve_data["freq"])
                fig.add_trace(
                    go.Scatter(
                        x=curve_data["freq"],
                        y=phase_smoothed,
                        mode="lines",
                        name=f"Phase: {channel_name}",
                        line=dict(color=phase_color, width=1, dash="dot"),
                        legendgroup=f"ch_{channel_name}",
                        showlegend=False,
                    ),
                    row=3, col=1, secondary_y=True,
                )
                trace_y_data.append(phase_smoothed)

    # Add crossover frequency vertical lines to all frequency plots
    crossover_freqs = get_all_crossover_frequencies(data)
    for xover_freq in crossover_freqs:
        # Format frequency label
        if xover_freq >= 1000:
            freq_label = f"{xover_freq/1000:.1f}k"
        else:
            freq_label = f"{xover_freq:.0f}"

        # Add to original curves plot (row 1)
        fig.add_trace(
            go.Scatter(
                x=[xover_freq, xover_freq],
                y=[initial_y_min, initial_y_max],
                mode="lines",
                name=f"Xover {freq_label} Hz",
                line=dict(color="rgba(180, 80, 180, 0.7)", width=1.5, dash="dashdot"),
                showlegend=True,
                legendgroup="crossover",
            ),
            row=1,
            col=1,
        )
        trace_y_data.append([initial_y_min, initial_y_max])

        # Add to EQ plot (row 2)
        fig.add_trace(
            go.Scatter(
                x=[xover_freq, xover_freq],
                y=[eq_y_lower, eq_y_upper],
                mode="lines",
                name=f"Xover {freq_label} Hz",
                line=dict(color="rgba(180, 80, 180, 0.7)", width=1.5, dash="dashdot"),
                showlegend=False,
                legendgroup="crossover",
            ),
            row=2,
            col=1,
        )
        trace_y_data.append([eq_y_lower, eq_y_upper])

        # Add to corrected curves plot (row 3)
        fig.add_trace(
            go.Scatter(
                x=[xover_freq, xover_freq],
                y=[corrected_y_min, corrected_y_max],
                mode="lines",
                name=f"Xover {freq_label} Hz",
                line=dict(color="rgba(180, 80, 180, 0.7)", width=1.5, dash="dashdot"),
                showlegend=False,
                legendgroup="crossover",
            ),
            row=3,
            col=1,
        )
        trace_y_data.append([corrected_y_min, corrected_y_max])

    # Plot summed curves (row 4) — listening position sums (e.g., L+LFE, R+LFE)
    summed_curves: dict[str, dict] = {}  # label -> {"freq": [...], "spl": [...]}
    summed_y_min, summed_y_max = -20.0, 30.0
    if has_summing:
        for label, group_channels in summing_groups:
            # Collect curves to sum (use corrected/final curves)
            group_curve_list = []
            for ch in group_channels:
                if ch in corrected_curves:
                    group_curve_list.append(corrected_curves[ch])

            if len(group_curve_list) < 2:
                continue

            # Use the frequency grid of the first curve; interpolate others onto it
            ref_freq = group_curve_list[0]["freq"]
            # Complex sum: magnitude + phase → phasor, sum, back to dB
            summed_complex = np.zeros(len(ref_freq), dtype=complex)
            for curve in group_curve_list:
                spl_interp = np.interp(ref_freq, curve["freq"], curve["spl"])
                phase_deg = curve.get("phase")
                if phase_deg is not None:
                    phase_interp = np.interp(ref_freq, curve["freq"], phase_deg)
                    phase_rad = np.deg2rad(phase_interp)
                else:
                    phase_rad = np.zeros(len(ref_freq))
                magnitude = 10.0 ** (np.array(spl_interp) / 20.0)
                summed_complex += magnitude * np.exp(1j * phase_rad)
            summed_spl = (20.0 * np.log10(np.maximum(np.abs(summed_complex), 1e-10))).tolist()
            summed_curves[label] = {"freq": ref_freq, "spl": summed_spl}

        # Compute y-range for summed curves
        summed_y_min, summed_y_max = _compute_fixed_range(list(summed_curves.values()))

        # Plot each summed group
        group_colors = [
            "rgba(31, 119, 180, 0.9)",   # blue
            "rgba(255, 127, 14, 0.9)",   # orange
            "rgba(44, 160, 44, 0.9)",    # green
            "rgba(214, 39, 40, 0.9)",    # red
        ]
        for g_idx, (label, curve_data) in enumerate(summed_curves.items()):
            color = group_colors[g_idx % len(group_colors)]
            freq = curve_data["freq"]
            spl_raw = curve_data["spl"]
            spl_smoothed = smooth_octave(freq, spl_raw, DEFAULT_SMOOTHING)

            fig.add_trace(
                go.Scatter(
                    x=freq,
                    y=spl_smoothed,
                    mode="lines",
                    name=f"Sum: {label}",
                    line=dict(color=color, width=2),
                ),
                row=sum_row,
                col=1,
            )
            trace_y_data.append(spl_smoothed)

        # Target line
        if summed_curves:
            first_summed = next(iter(summed_curves.values()))
            fig.add_trace(
                go.Scatter(
                    x=[first_summed["freq"][0], first_summed["freq"][-1]],
                    y=[0, 0],
                    mode="lines",
                    name="Target (0 dB)",
                    line=dict(color="rgba(150, 150, 150, 0.5)", width=1, dash="dash"),
                    showlegend=False,
                ),
                row=sum_row,
                col=1,
            )
            trace_y_data.append([0, 0])

        # Crossover vertical lines on summed plot
        for xover_freq in crossover_freqs:
            if xover_freq >= 1000:
                freq_label = f"{xover_freq/1000:.1f}k"
            else:
                freq_label = f"{xover_freq:.0f}"
            fig.add_trace(
                go.Scatter(
                    x=[xover_freq, xover_freq],
                    y=[summed_y_min, summed_y_max],
                    mode="lines",
                    name=f"Xover {freq_label} Hz",
                    line=dict(color="rgba(180, 80, 180, 0.7)", width=1.5, dash="dashdot"),
                    showlegend=False,
                    legendgroup="crossover",
                ),
                row=sum_row,
                col=1,
            )
            trace_y_data.append([summed_y_min, summed_y_max])

    # Plot impulse responses - shifted vertically
    ir_shift = 1.5  # Vertical shift between impulse responses
    ir_time_limit = 50.0  # Show first 50ms of impulse response
    ir_count = 0

    # Determine total IRs for vertical positioning
    if has_corrected_ir:
        total_irs = len(measurement_paths)
    else:
        total_irs = sum(len(paths) for paths in ir_wav_paths.values())

    if has_ir_data:
        if has_corrected_ir:
            # Plot corrected room impulse responses
            for i, (channel_name, meas_path) in enumerate(measurement_paths.items()):
                color = channel_colors[i % len(channel_colors)]

                # Load original measurement (WAV or CSV)
                result = load_measurement_ir(meas_path)
                if result is None:
                    print(f"Warning: Could not load measurement for {channel_name}: {meas_path}")
                    continue

                time_ms, original_ir, sample_rate = result

                # Get channel data for correction filters
                channel_data = channels.get(channel_name, {})

                # Compute corrected IR
                corrected_ir = compute_corrected_ir(original_ir, sample_rate, channel_data, json_dir)

                if corrected_ir is not None:
                    ir_to_plot = corrected_ir
                    label_suffix = " (corrected)"
                else:
                    ir_to_plot = original_ir
                    label_suffix = " (original - no correction applied)"

                if len(time_ms) > 0 and len(ir_to_plot) > 0:
                    # Limit to first ir_time_limit ms
                    mask = time_ms <= ir_time_limit
                    time_ms_limited = time_ms[mask]
                    ir_limited = ir_to_plot[:len(time_ms_limited)]

                    # Shift vertically (bottom to top)
                    vertical_offset = (total_irs - 1 - ir_count) * ir_shift
                    ir_shifted = ir_limited + vertical_offset

                    fig.add_trace(
                        go.Scatter(
                            x=time_ms_limited,
                            y=ir_shifted,
                            mode="lines",
                            name=f"IR: {channel_name}{label_suffix}",
                            line=dict(color=color, width=1.5),
                            legendgroup=f"ch_{channel_name}",
                            showlegend=False,
                        ),
                        row=ir_row,
                        col=1,
                    )
                    trace_y_data.append(ir_shifted.tolist())
                    ir_count += 1
        else:
            # Plot FIR correction filter impulse responses (original behavior)
            for i, (channel_name, ir_paths) in enumerate(ir_wav_paths.items()):
                color = channel_colors[i % len(channel_colors)]

                for ir_name, wav_path in ir_paths:
                    result = load_wav_file(wav_path)
                    if result is None:
                        continue

                    time_ms, ir, sample_rate = result

                    if len(time_ms) > 0 and len(ir) > 0:
                        # Limit to first ir_time_limit ms
                        mask = time_ms <= ir_time_limit
                        time_ms = time_ms[mask]
                        ir = ir[mask]

                        # Shift vertically (bottom to top)
                        vertical_offset = (total_irs - 1 - ir_count) * ir_shift
                        ir_shifted = ir + vertical_offset

                        # Create label
                        if len(ir_paths) > 1:
                            label = f"IR: {channel_name}/{ir_name}"
                        else:
                            label = f"IR: {channel_name}"

                        fig.add_trace(
                            go.Scatter(
                                x=time_ms,
                                y=ir_shifted,
                                mode="lines",
                                name=label,
                                line=dict(color=color, width=1.5),
                                legendgroup=f"ch_{channel_name}",
                                showlegend=False,
                            ),
                            row=ir_row,
                            col=1,
                        )
                        trace_y_data.append(ir_shifted.tolist())
                        ir_count += 1

    # Create smoothing buttons for original and corrected curves
    updatemenus = []
    has_smoothable = (corrected_freq_data and corrected_raw_spl) or (original_freq_data and original_raw_spl)
    if has_smoothable:
        buttons = []
        for label, octave_frac in SMOOTHING_OPTIONS:
            # Build new y-data for all traces
            new_y_data = []
            original_idx = 0
            corrected_idx = 0
            orig_phase_idx = 0
            corr_phase_idx = 0
            for trace_idx, y_data in enumerate(trace_y_data):
                if trace_idx in original_trace_indices:
                    smoothed = smooth_octave(
                        original_freq_data, original_raw_spl[original_idx], octave_frac
                    )
                    new_y_data.append(smoothed)
                    original_idx += 1
                elif trace_idx in original_phase_trace_indices:
                    smoothed = smooth_octave(
                        original_phase_freq[orig_phase_idx], original_raw_phase[orig_phase_idx], octave_frac
                    )
                    new_y_data.append(smoothed)
                    orig_phase_idx += 1
                elif trace_idx in corrected_trace_indices:
                    smoothed = smooth_octave(
                        corrected_freq_data, corrected_raw_spl[corrected_idx], octave_frac
                    )
                    new_y_data.append(smoothed)
                    corrected_idx += 1
                elif trace_idx in corrected_phase_trace_indices:
                    smoothed = smooth_octave(
                        corrected_phase_freq[corr_phase_idx], corrected_raw_phase[corr_phase_idx], octave_frac
                    )
                    new_y_data.append(smoothed)
                    corr_phase_idx += 1
                else:
                    # Keep unchanged (EQ traces, reference lines, IR traces)
                    new_y_data.append(y_data)

            buttons.append(dict(
                label=label,
                method="update",
                args=[{"y": new_y_data}]
            ))

        updatemenus = [dict(
            type="dropdown",
            direction="down",
            active=0,
            x=0.0,
            xanchor="left",
            y=0.62,
            yanchor="top",
            buttons=buttons,
            showactive=True,
            font=dict(size=10),
        )]

    # Update axes for frequency plots (rows 1, 2, 3, and summed if present)
    freq_rows = [1, 2, 3]
    if has_summing:
        freq_rows.append(sum_row)
    for row in freq_rows:
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

    # Original curves y-axis
    fig.update_yaxes(
        title_text="SPL (dB)",
        title_font=dict(size=11),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=[initial_y_min, initial_y_max],
        row=1,
        col=1,
    )

    # EQ response y-axis (symmetric around 0)
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

    # Corrected curves y-axis
    fig.update_yaxes(
        title_text="SPL (dB)",
        title_font=dict(size=11),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=[corrected_y_min, corrected_y_max],
        row=3,
        col=1,
    )

    # Summed curves y-axis
    if has_summing:
        fig.update_yaxes(
            title_text="SPL (dB)",
            title_font=dict(size=11),
            tickfont=dict(size=10),
            gridcolor="rgba(128, 128, 128, 0.2)",
            range=[summed_y_min, summed_y_max],
            row=sum_row,
            col=1,
        )

    # Phase secondary y-axes (rows 1 and 3)
    all_phase_values = []
    for phase_data in original_raw_phase + corrected_raw_phase:
        all_phase_values.extend(phase_data)
    if all_phase_values:
        phase_abs_max = max(abs(min(all_phase_values)), abs(max(all_phase_values)))
        phase_limit = max(90, math.ceil(phase_abs_max / 45) * 45)
    else:
        phase_limit = 180
    for row in [1, 3]:
        fig.update_yaxes(
            title_text="Phase (\u00b0)",
            title_font=dict(size=11),
            tickfont=dict(size=10),
            gridcolor="rgba(128, 128, 128, 0.0)",
            range=[-phase_limit, phase_limit],
            secondary_y=True,
            row=row,
            col=1,
        )

    # Configure impulse response axes if we have IR data
    if has_ir_data:
        # Impulse response x-axis (linear time in ms)
        fig.update_xaxes(
            title_text="Time (ms)",
            title_font=dict(size=11),
            tickfont=dict(size=10),
            gridcolor="rgba(128, 128, 128, 0.2)",
            range=[0, ir_time_limit],
            row=ir_row,
            col=1,
        )

        # Impulse response y-axis
        ir_y_max = max(1, ir_count) * ir_shift
        fig.update_yaxes(
            title_text="Amplitude (shifted)",
            title_font=dict(size=11),
            tickfont=dict(size=10),
            gridcolor="rgba(128, 128, 128, 0.2)",
            range=[-0.5, ir_y_max + 0.5],
            showticklabels=False,  # Hide tick labels since values are shifted
            row=ir_row,
            col=1,
        )
        fig_height = 1250 + (300 if has_summing else 0)
        dropdown_y = 0.70
    else:
        fig_height = 950 + (300 if has_summing else 0)
        dropdown_y = 0.60

    # Update smoothing dropdown position
    if updatemenus:
        updatemenus[0]["y"] = dropdown_y

    fig.update_layout(
        height=fig_height,
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
