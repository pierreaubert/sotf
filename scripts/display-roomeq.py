#!/usr/bin/env python3
"""
Display roomeq optimization results using Plotly.

Reads a roomeq-generated JSON file and creates an HTML file with interactive
plots comparing initial (without EQ) and final (with EQ) frequency response
curves for each channel.

Usage:
    python scripts/display-roomeq.py <roomeq_output.json> [output.html]

If no output file is specified, it defaults to <input_basename>_plots.html
"""

import json
import math
import sys
from pathlib import Path

try:
    import numpy as np
except ImportError:
    print("Error: numpy is required. Install with: pip install numpy")
    sys.exit(1)

try:
    import plotly.graph_objects as go
    from plotly.subplots import make_subplots
except ImportError:
    print("Error: plotly is required. Install with: pip install plotly")
    sys.exit(1)


# Smoothing options in octave fractions
# Index 0 is the default
SMOOTHING_OPTIONS = [
    ("1/1 oct", 1.0),
    ("1/3 oct", 1/3),
    ("1/6 oct", 1/6),
    ("1/12 oct", 1/12),
    ("1/24 oct", 1/24),
    ("Raw", None),
]

DEFAULT_SMOOTHING = 1.0  # 1/1 octave


def load_roomeq_json(filepath: Path) -> dict:
    """Load and parse roomeq JSON output file."""
    with open(filepath, "r") as f:
        return json.load(f)


def smooth_octave(freq: list[float], spl: list[float], octave_fraction: float) -> list[float]:
    """
    Apply octave smoothing to frequency response data.

    Args:
        freq: Frequency points in Hz
        spl: SPL values in dB
        octave_fraction: Smoothing width in octaves (e.g., 1/3 for 1/3 octave smoothing)

    Returns:
        Smoothed SPL values
    """
    if not freq or not spl or octave_fraction is None:
        return spl

    n = len(freq)
    smoothed = []

    for i in range(n):
        f_center = freq[i]
        if f_center <= 0:
            smoothed.append(spl[i])
            continue

        # Calculate frequency range for this octave fraction
        # For 1/N octave, the bandwidth is 2^(1/N) ratio
        ratio = 2 ** (octave_fraction / 2)
        f_low = f_center / ratio
        f_high = f_center * ratio

        # Find all points within the smoothing window
        values = []
        weights = []

        for j in range(n):
            if f_low <= freq[j] <= f_high:
                # Use triangular weighting (closer to center = more weight)
                log_dist = abs(math.log10(freq[j]) - math.log10(f_center))
                log_half_width = math.log10(ratio)
                weight = 1.0 - (log_dist / log_half_width) if log_half_width > 0 else 1.0
                values.append(spl[j])
                weights.append(max(0, weight))

        if values and sum(weights) > 0:
            # Weighted average
            smoothed.append(sum(v * w for v, w in zip(values, weights)) / sum(weights))
        else:
            smoothed.append(spl[i])

    return smoothed


def compute_y_range(curves: list[dict | None], padding: float = 2.0) -> tuple[float, float]:
    """Compute dynamic y-axis range from curve data."""
    all_spl = []
    for curve in curves:
        if curve and "spl" in curve:
            all_spl.extend(curve["spl"])

    if not all_spl:
        return (-20, 20)

    min_spl = min(all_spl)
    max_spl = max(all_spl)
    return (min_spl - padding, max_spl + padding)


def compute_average_spl_in_range(
    curve: dict, min_freq: float = 20.0, max_freq: float = 1200.0
) -> float:
    """Compute average SPL in a frequency range."""
    if not curve or "freq" not in curve or "spl" not in curve:
        return 0.0

    freq = curve["freq"]
    spl = curve["spl"]

    values_in_range = [
        s for f, s in zip(freq, spl) if min_freq <= f <= max_freq
    ]

    if not values_in_range:
        return 0.0

    return sum(values_in_range) / len(values_in_range)


def compute_eq_response(
    filters: list[dict], freq_points: list[float], sample_rate: float = 48000.0
) -> list[float]:
    """Compute the combined EQ frequency response from biquad filters."""
    if not filters or not freq_points:
        return []

    # Compute combined magnitude response
    combined_db = [0.0] * len(freq_points)

    for filt in filters:
        filter_type = filt.get("filter_type", "peak").lower()
        fc = filt.get("freq", 1000.0)
        q = filt.get("q", 1.0)
        gain_db = filt.get("db_gain", 0.0)

        # Compute biquad coefficients based on filter type
        for i, f in enumerate(freq_points):
            if f <= 0:
                continue

            # Normalized frequency
            w0 = 2 * math.pi * fc / sample_rate
            w = 2 * math.pi * f / sample_rate

            # For peak/parametric EQ filter
            if filter_type in ("peak", "peaking", "pk"):
                A = 10 ** (gain_db / 40.0)
                alpha = math.sin(w0) / (2 * q)

                b0 = 1 + alpha * A
                b1 = -2 * math.cos(w0)
                b2 = 1 - alpha * A
                a0 = 1 + alpha / A
                a1 = -2 * math.cos(w0)
                a2 = 1 - alpha / A

            elif filter_type in ("lowshelf", "ls"):
                A = 10 ** (gain_db / 40.0)
                alpha = math.sin(w0) / (2 * q)
                cos_w0 = math.cos(w0)
                sqrt_A = math.sqrt(A)

                b0 = A * ((A + 1) - (A - 1) * cos_w0 + 2 * sqrt_A * alpha)
                b1 = 2 * A * ((A - 1) - (A + 1) * cos_w0)
                b2 = A * ((A + 1) - (A - 1) * cos_w0 - 2 * sqrt_A * alpha)
                a0 = (A + 1) + (A - 1) * cos_w0 + 2 * sqrt_A * alpha
                a1 = -2 * ((A - 1) + (A + 1) * cos_w0)
                a2 = (A + 1) + (A - 1) * cos_w0 - 2 * sqrt_A * alpha

            elif filter_type in ("highshelf", "hs"):
                A = 10 ** (gain_db / 40.0)
                alpha = math.sin(w0) / (2 * q)
                cos_w0 = math.cos(w0)
                sqrt_A = math.sqrt(A)

                b0 = A * ((A + 1) + (A - 1) * cos_w0 + 2 * sqrt_A * alpha)
                b1 = -2 * A * ((A - 1) + (A + 1) * cos_w0)
                b2 = A * ((A + 1) + (A - 1) * cos_w0 - 2 * sqrt_A * alpha)
                a0 = (A + 1) - (A - 1) * cos_w0 + 2 * sqrt_A * alpha
                a1 = 2 * ((A - 1) - (A + 1) * cos_w0)
                a2 = (A + 1) - (A - 1) * cos_w0 - 2 * sqrt_A * alpha

            else:
                # Default to peak filter for unknown types
                A = 10 ** (gain_db / 40.0)
                alpha = math.sin(w0) / (2 * q)

                b0 = 1 + alpha * A
                b1 = -2 * math.cos(w0)
                b2 = 1 - alpha * A
                a0 = 1 + alpha / A
                a1 = -2 * math.cos(w0)
                a2 = 1 - alpha / A

            # Normalize coefficients
            b0 /= a0
            b1 /= a0
            b2 /= a0
            a1 /= a0
            a2 /= a0

            # Compute frequency response using z-transform
            # H(e^jw) = (b0 + b1*e^-jw + b2*e^-2jw) / (1 + a1*e^-jw + a2*e^-2jw)
            cos_w = math.cos(w)
            cos_2w = math.cos(2 * w)
            sin_w = math.sin(w)
            sin_2w = math.sin(2 * w)

            num_real = b0 + b1 * cos_w + b2 * cos_2w
            num_imag = -b1 * sin_w - b2 * sin_2w
            den_real = 1 + a1 * cos_w + a2 * cos_2w
            den_imag = -a1 * sin_w - a2 * sin_2w

            num_mag = math.sqrt(num_real**2 + num_imag**2)
            den_mag = math.sqrt(den_real**2 + den_imag**2)

            if den_mag > 1e-10:
                mag = num_mag / den_mag
                if mag > 1e-10:
                    combined_db[i] += 20 * math.log10(mag)

    return combined_db


def generate_freq_points(min_freq: float = 20.0, max_freq: float = 20000.0, n_points: int = 200) -> list[float]:
    """Generate logarithmically spaced frequency points."""
    log_min = math.log10(min_freq)
    log_max = math.log10(max_freq)
    return [10 ** (log_min + (log_max - log_min) * i / (n_points - 1)) for i in range(n_points)]


def compute_impulse_response(
    freq: list[float],
    spl: list[float],
    sample_rate: float = 48000.0,
    n_fft: int = 4096,
) -> tuple[np.ndarray, np.ndarray]:
    """
    Compute minimum-phase impulse response from frequency response magnitude data.

    Args:
        freq: Frequency points in Hz
        spl: SPL values in dB
        sample_rate: Sample rate for the impulse response
        n_fft: FFT size

    Returns:
        Tuple of (time_ms, impulse_response) arrays
    """
    if not freq or not spl:
        return np.array([]), np.array([])

    freq = np.array(freq)
    spl = np.array(spl)

    # Create linearly spaced frequency bins for FFT
    fft_freqs = np.fft.rfftfreq(n_fft, 1.0 / sample_rate)

    # Interpolate magnitude response to FFT frequency grid
    # Use log-frequency interpolation for better accuracy
    log_freq = np.log10(np.maximum(freq, 1e-10))
    log_fft_freqs = np.log10(np.maximum(fft_freqs, 1e-10))

    # Interpolate, extrapolating flat at edges
    mag_db = np.interp(log_fft_freqs, log_freq, spl, left=spl[0], right=spl[-1])

    # Convert dB to linear magnitude
    mag_linear = 10 ** (mag_db / 20.0)

    # Compute minimum phase using Hilbert transform of log magnitude
    # This ensures a causal impulse response
    log_mag = np.log(np.maximum(mag_linear, 1e-10))

    # For minimum phase: phase = -hilbert(log_magnitude)
    # We use the discrete Hilbert transform via FFT
    n_rfft = len(log_mag)
    full_log_mag = np.concatenate([log_mag, log_mag[-2:0:-1]])  # Make symmetric
    analytic = np.fft.fft(full_log_mag)

    # Apply Hilbert transform (multiply by -j*sign(f))
    n_full = len(analytic)
    h = np.zeros(n_full)
    h[0] = 1
    h[1:(n_full + 1) // 2] = 2
    if n_full % 2 == 0:
        h[n_full // 2] = 1

    phase = -np.imag(np.fft.ifft(analytic * h))[:n_rfft]

    # Construct complex spectrum
    spectrum = mag_linear * np.exp(1j * phase)

    # Inverse FFT to get impulse response
    ir = np.fft.irfft(spectrum, n_fft)

    # Normalize
    ir = ir / np.max(np.abs(ir)) if np.max(np.abs(ir)) > 0 else ir

    # Create time axis in milliseconds
    time_ms = np.arange(n_fft) / sample_rate * 1000.0

    return time_ms, ir


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
) -> go.Figure | None:
    """Create a Plotly figure showing the EQ frequency response."""
    if not eq_filters:
        return None

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


def create_combined_figure(data: dict) -> go.Figure:
    """Create a combined figure with three subplots: EQs, corrected curves, and impulse responses."""
    channels = data.get("channels", {})

    if not channels:
        print("Warning: No channels found in the JSON file")
        return go.Figure()

    channel_names = list(channels.keys())
    n_channels = len(channel_names)

    # Create 3-row subplot: EQ responses, corrected curves, impulse responses
    fig = make_subplots(
        rows=3,
        cols=1,
        subplot_titles=["All EQ Responses", "All Corrected Curves", "Impulse Responses (shifted)"],
        vertical_spacing=0.08,
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

    # Collect all final curves to compute y-range for corrected curves plot
    all_final_curves = []
    all_eq_responses = []

    for channel_data in channels.values():
        all_final_curves.append(channel_data.get("final_curve"))

    # First pass: compute EQ responses and collect for y-range
    for channel_name, channel_data in channels.items():
        plugins = channel_data.get("plugins", [])
        eq_filters = []
        for plugin in plugins:
            if plugin.get("plugin_type") == "eq":
                filters = plugin.get("parameters", {}).get("filters", [])
                eq_filters.extend(filters)

        if eq_filters:
            eq_response = compute_eq_response(eq_filters, freq_points)
            all_eq_responses.append(eq_response)

    # Compute y-ranges
    final_y_min, final_y_max = compute_y_range(all_final_curves)

    # Compute EQ y-range
    if all_eq_responses:
        all_eq_values = [v for resp in all_eq_responses for v in resp]
        if all_eq_values:
            max_abs = max(abs(min(all_eq_values)), abs(max(all_eq_values)))
            eq_y_limit = max(15, math.ceil(max_abs / 5) * 5 + 5)
        else:
            eq_y_limit = 15
    else:
        eq_y_limit = 15

    # Track trace indices and raw data for smoothing
    trace_y_data = []  # Will hold y-data for each trace (None for non-smoothable)
    corrected_freq_data = None  # Frequency data for corrected curves
    corrected_raw_spl = []  # Raw SPL data for each corrected curve
    corrected_trace_indices = []  # Indices of corrected curve traces

    # Plot EQ responses (row 1)
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]

        plugins = channel_data.get("plugins", [])
        eq_filters = []
        for plugin in plugins:
            if plugin.get("plugin_type") == "eq":
                filters = plugin.get("parameters", {}).get("filters", [])
                eq_filters.extend(filters)

        if eq_filters:
            eq_response = compute_eq_response(eq_filters, freq_points)
            fig.add_trace(
                go.Scatter(
                    x=freq_points,
                    y=eq_response,
                    mode="lines",
                    name=f"EQ: {channel_name}",
                    line=dict(color=color, width=2),
                    legendgroup=f"ch_{channel_name}",
                ),
                row=1,
                col=1,
            )
            trace_y_data.append(eq_response)  # EQ traces don't get smoothed

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
        row=1,
        col=1,
    )
    trace_y_data.append([0, 0])  # Reference line

    # Plot corrected curves (row 2) with default smoothing
    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]
        final_curve = channel_data.get("final_curve")

        if final_curve:
            if corrected_freq_data is None:
                corrected_freq_data = final_curve["freq"]

            spl_raw = final_curve["spl"]
            spl_smoothed = smooth_octave(final_curve["freq"], spl_raw, DEFAULT_SMOOTHING)

            corrected_trace_indices.append(len(trace_y_data))
            corrected_raw_spl.append(spl_raw)

            fig.add_trace(
                go.Scatter(
                    x=final_curve["freq"],
                    y=spl_smoothed,
                    mode="lines",
                    name=f"Corrected: {channel_name}",
                    line=dict(color=color, width=2),
                    legendgroup=f"ch_{channel_name}",
                    showlegend=False,
                ),
                row=2,
                col=1,
            )
            trace_y_data.append(spl_smoothed)

    # Add target line to corrected curves plot
    if channels:
        first_channel = next(iter(channels.values()))
        ref_curve = first_channel.get("final_curve") or first_channel.get("initial_curve")
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
                row=2,
                col=1,
            )
            trace_y_data.append([0, 0])  # Target line

    # Plot impulse responses (row 3) - shifted vertically
    ir_shift = 1.5  # Vertical shift between impulse responses
    ir_time_limit = 50.0  # Show first 50ms of impulse response
    ir_count = 0

    for i, (channel_name, channel_data) in enumerate(channels.items()):
        color = channel_colors[i % len(channel_colors)]
        final_curve = channel_data.get("final_curve")

        if final_curve:
            time_ms, ir = compute_impulse_response(
                final_curve["freq"],
                final_curve["spl"],
                sample_rate=48000.0,
                n_fft=4096,
            )

            if len(time_ms) > 0 and len(ir) > 0:
                # Limit to first ir_time_limit ms
                mask = time_ms <= ir_time_limit
                time_ms = time_ms[mask]
                ir = ir[mask]

                # Shift vertically (bottom to top)
                vertical_offset = (n_channels - 1 - ir_count) * ir_shift
                ir_shifted = ir + vertical_offset

                fig.add_trace(
                    go.Scatter(
                        x=time_ms,
                        y=ir_shifted,
                        mode="lines",
                        name=f"IR: {channel_name}",
                        line=dict(color=color, width=1.5),
                        legendgroup=f"ch_{channel_name}",
                        showlegend=False,
                    ),
                    row=3,
                    col=1,
                )
                trace_y_data.append(ir_shifted.tolist())
                ir_count += 1

    # Create smoothing buttons for corrected curves
    updatemenus = []
    if corrected_freq_data and corrected_raw_spl:
        buttons = []
        for label, octave_frac in SMOOTHING_OPTIONS:
            # Build new y-data for all traces
            new_y_data = []
            corrected_idx = 0
            for trace_idx, y_data in enumerate(trace_y_data):
                if trace_idx in corrected_trace_indices:
                    # Apply smoothing to this corrected curve
                    smoothed = smooth_octave(
                        corrected_freq_data, corrected_raw_spl[corrected_idx], octave_frac
                    )
                    new_y_data.append(smoothed)
                    corrected_idx += 1
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
            y=0.62,  # Position near the second subplot
            yanchor="top",
            buttons=buttons,
            showactive=True,
            font=dict(size=10),
        )]

    # Update axes for frequency plots (rows 1 and 2)
    for row in [1, 2]:
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

    # EQ response y-axis (symmetric around 0)
    fig.update_yaxes(
        title_text="Gain (dB)",
        title_font=dict(size=11),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=[-eq_y_limit, eq_y_limit],
        dtick=5,
        row=1,
        col=1,
    )

    # Corrected curves y-axis
    fig.update_yaxes(
        title_text="SPL (dB)",
        title_font=dict(size=11),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=[final_y_min, final_y_max],
        row=2,
        col=1,
    )

    # Impulse response x-axis (linear time in ms)
    fig.update_xaxes(
        title_text="Time (ms)",
        title_font=dict(size=11),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=[0, ir_time_limit],
        row=3,
        col=1,
    )

    # Impulse response y-axis
    ir_y_max = n_channels * ir_shift
    fig.update_yaxes(
        title_text="Amplitude (shifted)",
        title_font=dict(size=11),
        tickfont=dict(size=10),
        gridcolor="rgba(128, 128, 128, 0.2)",
        range=[-0.5, ir_y_max + 0.5],
        showticklabels=False,  # Hide tick labels since values are shifted
        row=3,
        col=1,
    )

    fig.update_layout(
        height=950,
        plot_bgcolor="white",
        paper_bgcolor="white",
        legend=dict(yanchor="top", y=0.99, xanchor="right", x=0.99, font=dict(size=10)),
        margin=dict(l=60, r=40, t=80, b=60),
        updatemenus=updatemenus,
    )

    return fig


def create_html_report(data: dict, output_path: Path) -> None:
    """Create an HTML report with all channel plots."""
    channels = data.get("channels", {})
    metadata = data.get("metadata", {})
    version = data.get("version", "unknown")

    # Build HTML content
    html_parts = [
        """<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>RoomEQ Results</title>
    <script src="https://cdn.plot.ly/plotly-2.27.0.min.js"></script>
    <style>
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
        <h1>RoomEQ Optimization Results</h1>
"""
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
    combined_fig = create_combined_figure(data)
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
        fig_eq = create_eq_figure(channel_name, eq_filters)
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


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    input_path = Path(sys.argv[1])
    if not input_path.exists():
        print(f"Error: File not found: {input_path}")
        sys.exit(1)

    # Determine output path
    if len(sys.argv) >= 3:
        output_path = Path(sys.argv[2])
    else:
        output_path = input_path.with_name(f"{input_path.stem}_plots.html")

    # Load data
    print(f"Loading: {input_path}")
    data = load_roomeq_json(input_path)

    # Check for channels
    channels = data.get("channels", {})
    if not channels:
        print("Error: No channels found in the JSON file")
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
    create_html_report(data, output_path)


if __name__ == "__main__":
    main()
