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
measurement WAV paths, the IR graph shows corrected room impulse responses
(original room IR convolved with correction filters).
"""

import argparse
import json
import math
import sys
import wave
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

try:
    from scipy import signal as scipy_signal
    HAS_SCIPY = True
except ImportError:
    HAS_SCIPY = False


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
    if not filepath.exists():
        print(f"Error: File not found: {filepath}")
        sys.exit(1)
    with open(filepath, "r") as f:
        return json.load(f)


def extract_measurement_wav_paths(input_data: dict, json_dir: Path) -> dict[str, Path]:
    """
    Extract original measurement WAV paths from input JSON.

    Handles MeasurementSource structure:
    - InlineMeasurement with wav_path
    - Single(MeasurementRef) -> InlineMeasurement.wav_path
    - Multiple measurements -> first wav_path found
    - SpeakerGroup -> measurements[].wav_path

    Args:
        input_data: The roomeq input JSON data
        json_dir: Directory containing the JSON file (for resolving relative paths)

    Returns:
        Dict mapping channel name to resolved WAV Path
    """
    wav_paths = {}
    speakers = input_data.get("speakers", {})

    for channel_name, speaker_data in speakers.items():
        wav_path = _extract_wav_path_from_speaker(speaker_data, json_dir)
        if wav_path:
            wav_paths[channel_name] = wav_path

    return wav_paths


def _extract_wav_path_from_speaker(speaker_data, json_dir: Path) -> Path | None:
    """Extract WAV path from a speaker configuration."""
    if speaker_data is None:
        return None

    # Check if it's a string (simple CSV path reference)
    if isinstance(speaker_data, str):
        return None

    # Check for SpeakerGroup structure
    if isinstance(speaker_data, dict):
        # SpeakerGroup has 'name' and 'measurements'
        if "measurements" in speaker_data and "name" in speaker_data:
            for measurement in speaker_data.get("measurements", []):
                wav_path = _extract_wav_path_from_measurement_source(measurement, json_dir)
                if wav_path:
                    return wav_path
            return None

        # InlineMeasurement structure
        wav_path = _extract_wav_from_inline(speaker_data, json_dir)
        if wav_path:
            return wav_path

        # Check for 'path' field (Named reference)
        if "path" in speaker_data:
            return None

    # Check if it's a list (Multiple measurements)
    if isinstance(speaker_data, list):
        for item in speaker_data:
            wav_path = _extract_wav_path_from_measurement_source(item, json_dir)
            if wav_path:
                return wav_path

    return None


def _extract_wav_path_from_measurement_source(source, json_dir: Path) -> Path | None:
    """Extract WAV path from a MeasurementSource (Single, Multiple, or InlineMeasurement)."""
    if source is None:
        return None

    # String path - no WAV
    if isinstance(source, str):
        return None

    # List - Multiple measurements
    if isinstance(source, list):
        for item in source:
            wav_path = _extract_wav_path_from_measurement_source(item, json_dir)
            if wav_path:
                return wav_path
        return None

    # Dict - could be InlineMeasurement, Named, or nested source
    if isinstance(source, dict):
        return _extract_wav_from_inline(source, json_dir)

    return None


def _extract_wav_from_inline(data: dict, json_dir: Path) -> Path | None:
    """Extract WAV path from an InlineMeasurement dict."""
    wav_path_str = data.get("wav_path")
    if wav_path_str:
        wav_path = Path(wav_path_str)
        if not wav_path.is_absolute():
            wav_path = json_dir / wav_path
        return wav_path
    return None


def compute_iir_impulse_response(
    eq_filters: list[dict], sample_rate: float, n_samples: int
) -> np.ndarray | None:
    """
    Compute impulse response of cascaded biquad filters.

    Args:
        eq_filters: List of filter configs with filter_type, freq, q, db_gain
        sample_rate: Sample rate in Hz
        n_samples: Number of samples for impulse response

    Returns:
        Impulse response as numpy array, or None if scipy not available
    """
    if not HAS_SCIPY:
        print("Warning: scipy not available, skipping IIR IR computation")
        return None

    if not eq_filters:
        # No filters - return unit impulse
        ir = np.zeros(n_samples)
        ir[0] = 1.0
        return ir

    # Build cascaded second-order sections (SOS)
    sos_list = []

    for filt in eq_filters:
        filter_type = filt.get("filter_type", "peak").lower()
        fc = filt.get("freq", 1000.0)
        q = filt.get("q", 1.0)
        gain_db = filt.get("db_gain", 0.0)

        # Compute biquad coefficients
        w0 = 2 * math.pi * fc / sample_rate
        alpha = math.sin(w0) / (2 * q)
        cos_w0 = math.cos(w0)

        if filter_type in ("peak", "peaking", "pk"):
            A = 10 ** (gain_db / 40.0)
            b0 = 1 + alpha * A
            b1 = -2 * cos_w0
            b2 = 1 - alpha * A
            a0 = 1 + alpha / A
            a1 = -2 * cos_w0
            a2 = 1 - alpha / A

        elif filter_type in ("lowshelf", "ls"):
            A = 10 ** (gain_db / 40.0)
            sqrt_A = math.sqrt(A)
            b0 = A * ((A + 1) - (A - 1) * cos_w0 + 2 * sqrt_A * alpha)
            b1 = 2 * A * ((A - 1) - (A + 1) * cos_w0)
            b2 = A * ((A + 1) - (A - 1) * cos_w0 - 2 * sqrt_A * alpha)
            a0 = (A + 1) + (A - 1) * cos_w0 + 2 * sqrt_A * alpha
            a1 = -2 * ((A - 1) + (A + 1) * cos_w0)
            a2 = (A + 1) + (A - 1) * cos_w0 - 2 * sqrt_A * alpha

        elif filter_type in ("highshelf", "hs"):
            A = 10 ** (gain_db / 40.0)
            sqrt_A = math.sqrt(A)
            b0 = A * ((A + 1) + (A - 1) * cos_w0 + 2 * sqrt_A * alpha)
            b1 = -2 * A * ((A - 1) + (A + 1) * cos_w0)
            b2 = A * ((A + 1) + (A - 1) * cos_w0 - 2 * sqrt_A * alpha)
            a0 = (A + 1) - (A - 1) * cos_w0 + 2 * sqrt_A * alpha
            a1 = 2 * ((A - 1) - (A + 1) * cos_w0)
            a2 = (A + 1) - (A - 1) * cos_w0 - 2 * sqrt_A * alpha

        else:
            # Default to peak
            A = 10 ** (gain_db / 40.0)
            b0 = 1 + alpha * A
            b1 = -2 * cos_w0
            b2 = 1 - alpha * A
            a0 = 1 + alpha / A
            a1 = -2 * cos_w0
            a2 = 1 - alpha / A

        # Normalize
        b0 /= a0
        b1 /= a0
        b2 /= a0
        a1 /= a0
        a2 /= a0

        # SOS format: [b0, b1, b2, 1, a1, a2]
        sos_list.append([b0, b1, b2, 1.0, a1, a2])

    if not sos_list:
        ir = np.zeros(n_samples)
        ir[0] = 1.0
        return ir

    # Create unit impulse
    impulse = np.zeros(n_samples)
    impulse[0] = 1.0

    # Apply cascaded SOS filter
    sos = np.array(sos_list)
    ir = scipy_signal.sosfilt(sos, impulse)

    return ir


def compute_corrected_ir(
    original_ir: np.ndarray,
    sample_rate: int,
    channel_data: dict,
    json_dir: Path,
) -> np.ndarray | None:
    """
    Compute corrected room IR based on correction mode.

    Detects mode from plugins:
    - IIR only: EQ plugins present, no convolution
    - FIR only: convolution plugin present, no EQ
    - Mixed: band_split + convolution + EQ

    Args:
        original_ir: Original room impulse response
        sample_rate: Sample rate in Hz
        channel_data: Channel data from output JSON
        json_dir: Directory containing the JSON file (for resolving relative paths)

    Returns:
        Corrected impulse response, or None if unable to compute
    """
    plugins = channel_data.get("plugins", [])

    # Detect correction mode
    has_eq = any(p.get("plugin_type") == "eq" for p in plugins)
    has_convolution = any(p.get("plugin_type") == "convolution" for p in plugins)
    has_band_split = any(p.get("plugin_type") == "band_split" for p in plugins)

    n_samples = len(original_ir)

    if has_band_split:
        # Mixed mode - complex, not fully implemented
        # For now, fall back to IIR-only if EQ present
        if has_eq:
            return _compute_iir_corrected_ir(original_ir, sample_rate, plugins)
        return None

    if has_eq and not has_convolution:
        # IIR only mode
        return _compute_iir_corrected_ir(original_ir, sample_rate, plugins)

    if has_convolution and not has_eq:
        # FIR only mode
        return _compute_fir_corrected_ir(original_ir, plugins, json_dir)

    if has_convolution and has_eq:
        # Both FIR and IIR - apply both
        # First apply FIR, then IIR
        fir_corrected = _compute_fir_corrected_ir(original_ir, plugins, json_dir)
        if fir_corrected is not None:
            return _compute_iir_corrected_ir(fir_corrected, sample_rate, plugins)
        return None

    # No correction plugins
    return original_ir.copy()


def _compute_iir_corrected_ir(
    original_ir: np.ndarray, sample_rate: int, plugins: list[dict]
) -> np.ndarray | None:
    """Apply IIR correction to original IR."""
    # Collect all EQ filters
    eq_filters = []
    for plugin in plugins:
        if plugin.get("plugin_type") == "eq":
            filters = plugin.get("parameters", {}).get("filters", [])
            eq_filters.extend(filters)

    if not eq_filters:
        return original_ir.copy()

    # Compute IIR impulse response
    iir_ir = compute_iir_impulse_response(eq_filters, sample_rate, len(original_ir) * 2)
    if iir_ir is None:
        return None

    # Convolve original with IIR correction
    corrected = np.convolve(original_ir, iir_ir, mode='same')

    # Normalize
    max_val = np.max(np.abs(corrected))
    if max_val > 0:
        corrected = corrected / max_val

    return corrected


def _compute_fir_corrected_ir(
    original_ir: np.ndarray, plugins: list[dict], json_dir: Path
) -> np.ndarray | None:
    """Apply FIR correction to original IR."""
    # Find convolution plugin
    fir_ir = None
    for plugin in plugins:
        if plugin.get("plugin_type") == "convolution":
            ir_file = plugin.get("parameters", {}).get("ir_file")
            if ir_file:
                ir_path = Path(ir_file)
                if not ir_path.is_absolute():
                    ir_path = json_dir / ir_path
                result = load_wav_file(ir_path)
                if result is not None:
                    _, fir_ir, _ = result
                    break

    if fir_ir is None:
        return None

    # Convolve original with FIR correction
    corrected = np.convolve(original_ir, fir_ir, mode='same')

    # Normalize
    max_val = np.max(np.abs(corrected))
    if max_val > 0:
        corrected = corrected / max_val

    return corrected


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


def compute_biquad_response_db(
    b0: float, b1: float, b2: float, a1: float, a2: float, freq_points: list[float], sample_rate: float
) -> list[float]:
    """Compute magnitude response in dB for a single biquad section (already normalized by a0)."""
    result = []
    for f in freq_points:
        if f <= 0:
            result.append(0.0)
            continue
        w = 2 * math.pi * f / sample_rate
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

        if den_mag > 1e-10 and num_mag > 1e-10:
            result.append(20 * math.log10(num_mag / den_mag))
        else:
            result.append(-200.0)
    return result


def compute_crossover_response(
    crossover_type: str,
    frequency: float,
    output: str,
    freq_points: list[float],
    sample_rate: float = 48000.0,
) -> list[float]:
    """Compute magnitude response of a crossover filter.

    Args:
        crossover_type: "LinkwitzRiley4", "LinkwitzRiley2", or "Butterworth2"
        frequency: Crossover frequency in Hz
        output: "lowpass" or "highpass"
        freq_points: Frequency points to evaluate at
        sample_rate: Sample rate in Hz

    Returns:
        Magnitude response in dB at the given frequency points
    """
    w0 = 2 * math.pi * frequency / sample_rate
    sin_w0 = math.sin(w0)
    cos_w0 = math.cos(w0)

    # Butterworth Q for 2nd-order
    q = math.sqrt(2) / 2  # 0.7071

    alpha = sin_w0 / (2 * q)

    if output == "lowpass":
        # 2nd-order Butterworth lowpass
        b0 = (1 - cos_w0) / 2
        b1 = 1 - cos_w0
        b2 = (1 - cos_w0) / 2
    else:
        # 2nd-order Butterworth highpass
        b0 = (1 + cos_w0) / 2
        b1 = -(1 + cos_w0)
        b2 = (1 + cos_w0) / 2

    a0 = 1 + alpha
    a1_coeff = -2 * cos_w0
    a2_coeff = 1 - alpha

    # Normalize
    b0 /= a0
    b1 /= a0
    b2 /= a0
    a1_coeff /= a0
    a2_coeff /= a0

    # Single 2nd-order section response
    section_db = compute_biquad_response_db(b0, b1, b2, a1_coeff, a2_coeff, freq_points, sample_rate)

    ct = crossover_type.lower()
    if ct in ("linkwitzriley4", "lr4", "lr24"):
        # LR4 = two cascaded 2nd-order Butterworth => double the dB
        return [2 * d for d in section_db]
    elif ct in ("linkwitzriley2", "lr2", "lr12"):
        # LR2 = single 2nd-order Butterworth (Q=0.5 ideally, but standard BW Q is used)
        # For LR2, use Q=0.5
        alpha_lr2 = sin_w0 / (2 * 0.5)
        if output == "lowpass":
            b0_lr2 = (1 - cos_w0) / 2
            b1_lr2 = 1 - cos_w0
            b2_lr2 = (1 - cos_w0) / 2
        else:
            b0_lr2 = (1 + cos_w0) / 2
            b1_lr2 = -(1 + cos_w0)
            b2_lr2 = (1 + cos_w0) / 2
        a0_lr2 = 1 + alpha_lr2
        a1_lr2 = -2 * cos_w0 / a0_lr2
        a2_lr2 = (1 - alpha_lr2) / a0_lr2
        b0_lr2 /= a0_lr2
        b1_lr2 /= a0_lr2
        b2_lr2 /= a0_lr2
        return compute_biquad_response_db(b0_lr2, b1_lr2, b2_lr2, a1_lr2, a2_lr2, freq_points, sample_rate)
    elif ct in ("butterworth2", "bw2", "butterworth12"):
        # Single 2nd-order Butterworth
        return section_db
    else:
        # Default: LR4
        return [2 * d for d in section_db]


def compute_minimum_phase_from_spl(
    freq: list[float], spl: list[float]
) -> list[float] | None:
    """Compute minimum phase from magnitude response using cepstral method.

    Args:
        freq: Frequency points in Hz
        spl: SPL values in dB

    Returns:
        Phase in degrees at the given frequency points, or None if unable to compute
    """
    if not freq or not spl or len(freq) < 2:
        return None

    freq_arr = np.array(freq, dtype=np.float64)
    spl_arr = np.array(spl, dtype=np.float64)

    # Dense linear frequency grid for FFT-based computation
    n_fft = 16384
    sample_rate = float(max(freq)) * 3.0
    freq_fft = np.fft.rfftfreq(n_fft, d=1.0 / sample_rate)

    # Interpolate magnitude (dB) to FFT grid
    mag_db_interp = np.interp(freq_fft, freq_arr, spl_arr, left=float(spl_arr[0]), right=float(spl_arr[-1]))

    # Convert dB to natural log of linear magnitude: ln(|H|) = dB * ln(10) / 20
    log_mag = mag_db_interp * (np.log(10.0) / 20.0)

    # Real cepstrum via inverse FFT
    cepstrum = np.fft.irfft(log_mag)

    # Minimum phase cepstrum: keep causal part, double it (except DC and Nyquist)
    n = len(cepstrum)
    min_cep = np.zeros(n)
    min_cep[0] = cepstrum[0]
    min_cep[1:n // 2] = 2.0 * cepstrum[1:n // 2]
    if n % 2 == 0:
        min_cep[n // 2] = cepstrum[n // 2]

    # Back to frequency domain
    spectrum = np.fft.rfft(min_cep)

    # Minimum phase is the angle of the complex spectrum
    phase_deg = np.degrees(np.angle(np.exp(spectrum)))

    # Interpolate to original frequency points
    result = np.interp(freq_arr, freq_fft, phase_deg)
    return result.tolist()


def compute_fir_frequency_response(
    ir_path: Path, freq_points: list[float]
) -> list[float] | None:
    """
    Compute frequency response of an FIR filter from its impulse response WAV file.

    Args:
        ir_path: Path to the impulse response WAV file
        freq_points: Frequency points (Hz) to evaluate at

    Returns:
        Magnitude response in dB at the given frequency points, or None if unable to compute
    """
    result = load_wav_file(ir_path)
    if result is None:
        return None

    _, ir_samples, sample_rate = result

    # Compute FFT with good frequency resolution
    n_fft = max(len(ir_samples), 8192)
    n_fft = 1 << (n_fft - 1).bit_length()  # Next power of 2

    spectrum = np.fft.rfft(ir_samples, n=n_fft)
    magnitudes = np.abs(spectrum)

    # Frequency bins
    fft_freqs = np.fft.rfftfreq(n_fft, d=1.0 / sample_rate)

    # Convert to dB (avoid log of zero)
    magnitudes_db = np.where(magnitudes > 1e-10, 20 * np.log10(magnitudes), -200.0)

    # Interpolate at desired frequency points
    freq_array = np.array(freq_points)
    response_db = np.interp(freq_array, fft_freqs, magnitudes_db)

    return response_db.tolist()


def generate_freq_points(min_freq: float = 20.0, max_freq: float = 20000.0, n_points: int = 200) -> list[float]:
    """Generate logarithmically spaced frequency points."""
    log_min = math.log10(min_freq)
    log_max = math.log10(max_freq)
    return [10 ** (log_min + (log_max - log_min) * i / (n_points - 1)) for i in range(n_points)]


def load_wav_file(wav_path: Path) -> tuple[np.ndarray, np.ndarray, int] | None:
    """
    Load a WAV file and return the impulse response data.

    Args:
        wav_path: Path to the WAV file

    Returns:
        Tuple of (time_ms, samples, sample_rate) or None if file cannot be loaded
    """
    if not wav_path.exists():
        print(f"Warning: WAV file not found: {wav_path}")
        return None

    samples = None
    sample_rate = None

    # Try scipy first (handles IEEE float and other extended formats)
    if HAS_SCIPY:
        try:
            from scipy.io import wavfile
            sr, data = wavfile.read(str(wav_path))
            sample_rate = sr
            samples = data.astype(np.float64)

            # Normalize integer formats to [-1, 1]
            if data.dtype == np.int16:
                samples /= 32768.0
            elif data.dtype == np.int32:
                samples /= 2147483648.0
            elif data.dtype == np.uint8:
                samples = (samples - 128) / 128.0
            # float32/float64 are already in [-1, 1] range

            # If stereo or more, take first channel
            if samples.ndim > 1:
                samples = samples[:, 0]
        except Exception:
            samples = None

    # Fall back to wave module (PCM integer only)
    if samples is None:
        try:
            with wave.open(str(wav_path), 'rb') as wf:
                sample_rate = wf.getframerate()
                n_channels = wf.getnchannels()
                n_frames = wf.getnframes()
                sample_width = wf.getsampwidth()

                # Read raw audio data
                raw_data = wf.readframes(n_frames)

                # Convert to numpy array based on sample width
                if sample_width == 1:
                    samples = np.frombuffer(raw_data, dtype=np.uint8).astype(np.float64) - 128
                    samples /= 128.0
                elif sample_width == 2:
                    samples = np.frombuffer(raw_data, dtype=np.int16).astype(np.float64)
                    samples /= 32768.0
                elif sample_width == 3:
                    # 24-bit audio - need to handle byte by byte
                    samples = np.zeros(n_frames * n_channels, dtype=np.float64)
                    for i in range(n_frames * n_channels):
                        b = raw_data[i*3:(i+1)*3]
                        val = int.from_bytes(b, byteorder='little', signed=True)
                        samples[i] = val / 8388608.0
                elif sample_width == 4:
                    samples = np.frombuffer(raw_data, dtype=np.int32).astype(np.float64)
                    samples /= 2147483648.0
                else:
                    print(f"Warning: Unsupported sample width {sample_width} in {wav_path}")
                    return None

                # If stereo or more, take first channel
                if n_channels > 1:
                    samples = samples[::n_channels]

        except Exception as e:
            print(f"Warning: Failed to load WAV file {wav_path}: {e}")
            return None

    if samples is None or sample_rate is None:
        print(f"Warning: Could not load WAV file {wav_path}")
        return None

    # Normalize
    max_val = np.max(np.abs(samples))
    if max_val > 0:
        samples = samples / max_val

    # Create time axis in milliseconds
    time_ms = np.arange(len(samples)) / sample_rate * 1000.0

    return time_ms, samples, sample_rate


def extract_ir_wav_paths(channel_data: dict) -> list[tuple[str, str]]:
    """
    Extract impulse response WAV file paths from a channel's plugin configuration.

    Looks for:
    - "convolution" plugins with "ir_file" parameter

    Returns:
        List of (name, path) tuples
    """
    ir_paths = []

    # Check main plugins for convolution
    plugins = channel_data.get("plugins", [])
    for plugin in plugins:
        if plugin.get("plugin_type") == "convolution":
            ir_file = plugin.get("parameters", {}).get("ir_file")
            if ir_file:
                ir_paths.append(("main", ir_file))

    # Check driver chains for convolution plugins
    drivers = channel_data.get("drivers", [])
    for driver in drivers:
        driver_name = driver.get("name", "driver")
        driver_plugins = driver.get("plugins", [])
        for plugin in driver_plugins:
            if plugin.get("plugin_type") == "convolution":
                ir_file = plugin.get("parameters", {}).get("ir_file")
                if ir_file:
                    ir_paths.append((driver_name, ir_file))

    return ir_paths


def get_all_ir_wav_paths(data: dict, json_dir: Path) -> dict[str, list[tuple[str, Path]]]:
    """
    Extract all impulse response WAV file paths from all channels.

    Args:
        data: The roomeq JSON data
        json_dir: Directory containing the JSON file (for resolving relative paths)

    Returns:
        Dict mapping channel name to list of (ir_name, resolved_path) tuples
    """
    all_paths = {}
    channels = data.get("channels", {})

    for channel_name, channel_data in channels.items():
        ir_paths = extract_ir_wav_paths(channel_data)
        if ir_paths:
            resolved_paths = []
            for ir_name, ir_file in ir_paths:
                ir_path = Path(ir_file)
                # Resolve relative paths against JSON file directory
                if not ir_path.is_absolute():
                    ir_path = json_dir / ir_path
                resolved_paths.append((ir_name, ir_path))
            all_paths[channel_name] = resolved_paths

    return all_paths


def extract_crossover_frequencies(channel_data: dict) -> list[float]:
    """
    Extract crossover frequencies from a channel's plugin configuration.

    Looks for:
    - "crossover" plugins in driver chains (active crossovers)
    - "band_split" plugins in main chain (mixed mode crossovers)

    Returns:
        Sorted list of unique crossover frequencies in Hz
    """
    crossover_freqs = set()

    # Check main plugins for band_split
    plugins = channel_data.get("plugins", [])
    for plugin in plugins:
        if plugin.get("plugin_type") == "band_split":
            freq = plugin.get("parameters", {}).get("frequency")
            if freq:
                crossover_freqs.add(float(freq))

    # Check driver chains for crossover plugins
    drivers = channel_data.get("drivers", [])
    for driver in drivers:
        driver_plugins = driver.get("plugins", [])
        for plugin in driver_plugins:
            if plugin.get("plugin_type") == "crossover":
                freq = plugin.get("parameters", {}).get("frequency")
                if freq:
                    crossover_freqs.add(float(freq))

    return sorted(crossover_freqs)


def get_driver_initial_curves(channel_data: dict) -> list[tuple[str, dict]] | None:
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


def compute_driver_correction_db(
    driver: dict,
    channel_plugins: list[dict],
    freq_points: list[float],
    sample_rate: float = 48000.0,
) -> list[float]:
    """Compute the total correction in dB for a single driver.

    Includes:
    - Per-driver gain
    - Per-driver crossover filters (LP/HP)
    - Channel-level EQ filters

    Args:
        driver: Driver dict from the output JSON
        channel_plugins: Channel-level plugins list
        freq_points: Frequency points to evaluate at
        sample_rate: Sample rate in Hz

    Returns:
        Correction in dB at each frequency point
    """
    correction_db = [0.0] * len(freq_points)

    # Per-driver plugins (gain, crossover)
    driver_plugins = driver.get("plugins", [])
    for plugin in driver_plugins:
        ptype = plugin.get("plugin_type", "")
        params = plugin.get("parameters", {})

        if ptype == "gain":
            gain_db = params.get("gain_db", 0.0)
            correction_db = [c + gain_db for c in correction_db]

        elif ptype == "crossover":
            xover_type = params.get("crossover_type", "LinkwitzRiley4")
            xover_freq = params.get("frequency", 1000.0)
            xover_output = params.get("output", "lowpass")
            xover_db = compute_crossover_response(
                xover_type, xover_freq, xover_output, freq_points, sample_rate
            )
            correction_db = [c + x for c, x in zip(correction_db, xover_db)]

    # Channel-level EQ
    eq_filters = []
    for plugin in channel_plugins:
        if plugin.get("plugin_type") == "eq":
            filters = plugin.get("parameters", {}).get("filters", [])
            eq_filters.extend(filters)

    if eq_filters:
        eq_db = compute_eq_response(eq_filters, freq_points, sample_rate)
        correction_db = [c + e for c, e in zip(correction_db, eq_db)]

    # Channel-level gain
    for plugin in channel_plugins:
        if plugin.get("plugin_type") == "gain":
            gain_db = plugin.get("parameters", {}).get("gain_db", 0.0)
            correction_db = [c + gain_db for c in correction_db]

    return correction_db


def get_all_crossover_frequencies(data: dict) -> list[float]:
    """
    Extract all unique crossover frequencies from all channels.

    Returns:
        Sorted list of unique crossover frequencies in Hz
    """
    all_freqs = set()
    channels = data.get("channels", {})

    for channel_data in channels.values():
        freqs = extract_crossover_frequencies(channel_data)
        all_freqs.update(freqs)

    return sorted(all_freqs)


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


def create_combined_figure(
    data: dict, json_path: Path | None = None, input_data: dict | None = None, input_path: Path | None = None
) -> go.Figure:
    """Create a combined figure with subplots: EQs, corrected curves, and impulse responses.

    Args:
        data: Output JSON data (roomeq result with correction filters)
        json_path: Path to output JSON (for resolving relative paths)
        input_data: Optional input JSON data (roomeq config with measurement WAV paths)
        input_path: Optional path to input JSON (for resolving relative paths)

    When input_data is provided, shows corrected room IRs (original convolved with corrections).
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
    measurement_wav_paths = {}
    if input_data:
        measurement_wav_paths = extract_measurement_wav_paths(input_data, input_dir)
        if measurement_wav_paths:
            print(f"Found {len(measurement_wav_paths)} measurement WAV files for corrected IR computation")
        else:
            print("No measurement WAV paths found in input JSON")

    # Check if we have WAV files for impulse responses
    # Priority: corrected IRs (from input) > FIR correction filters (from output)
    ir_wav_paths = get_all_ir_wav_paths(data, json_dir)
    has_corrected_ir = bool(measurement_wav_paths)
    has_fir_ir = bool(ir_wav_paths)
    has_ir_wavs = has_corrected_ir or has_fir_ir

    # Create subplot layout based on available data
    if has_ir_wavs:
        ir_title = "Corrected Room Impulse Responses" if has_corrected_ir else "FIR Correction Filters"
        fig = make_subplots(
            rows=4,
            cols=1,
            subplot_titles=["All Original Curves", "All EQ Responses", "All Corrected Curves", ir_title],
            vertical_spacing=0.06,
            specs=[[{"secondary_y": True}], [{}], [{"secondary_y": True}], [{}]],
        )
    else:
        fig = make_subplots(
            rows=3,
            cols=1,
            subplot_titles=["All Original Curves", "All EQ Responses", "All Corrected Curves"],
            vertical_spacing=0.08,
            specs=[[{"secondary_y": True}], [{}], [{"secondary_y": True}]],
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

        if eq_filters:
            eq_response = compute_eq_response(eq_filters, freq_points)
            all_eq_responses.append(eq_response)

        # Compute full-range corrected curve (initial + all corrections)
        initial_curve = channel_data.get("initial_curve")
        if initial_curve:
            freq = initial_curve["freq"]
            spl = initial_curve["spl"]
            correction_db = [0.0] * len(freq)

            # Add IIR EQ correction
            if eq_filters:
                eq_response = compute_eq_response(eq_filters, freq)
                correction_db = [c + e for c, e in zip(correction_db, eq_response)]

            # Add FIR convolution correction (main chain)
            for plugin in plugins:
                if plugin.get("plugin_type") == "convolution":
                    ir_file = plugin.get("parameters", {}).get("ir_file")
                    if ir_file:
                        ir_path = Path(ir_file)
                        if not ir_path.is_absolute():
                            ir_path = json_dir / ir_path
                        fir_response = compute_fir_frequency_response(ir_path, freq)
                        if fir_response is not None:
                            correction_db = [c + f for c, f in zip(correction_db, fir_response)]

            # Add gain plugin offset
            for plugin in plugins:
                if plugin.get("plugin_type") == "gain":
                    gain_db = plugin.get("parameters", {}).get("gain_db", 0.0)
                    correction_db = [c + gain_db for c in correction_db]

            corrected_spl = [s + c for s, c in zip(spl, correction_db)]
            corrected_curves[channel_name] = {"freq": freq, "spl": corrected_spl}

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

    # Plot EQ responses (row 2)
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
                    showlegend=False,
                ),
                row=2,
                col=1,
            )
            trace_y_data.append(eq_response)

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
                y=[-eq_y_limit, eq_y_limit],
                mode="lines",
                name=f"Xover {freq_label} Hz",
                line=dict(color="rgba(180, 80, 180, 0.7)", width=1.5, dash="dashdot"),
                showlegend=False,
                legendgroup="crossover",
            ),
            row=2,
            col=1,
        )
        trace_y_data.append([-eq_y_limit, eq_y_limit])

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

    # Plot impulse responses (row 3) - shifted vertically
    ir_shift = 1.5  # Vertical shift between impulse responses
    ir_time_limit = 50.0  # Show first 50ms of impulse response
    ir_count = 0

    # Determine total IRs for vertical positioning
    if has_corrected_ir:
        total_irs = len(measurement_wav_paths)
    else:
        total_irs = sum(len(paths) for paths in ir_wav_paths.values())

    if has_ir_wavs:
        if has_corrected_ir:
            # Plot corrected room impulse responses
            for i, (channel_name, wav_path) in enumerate(measurement_wav_paths.items()):
                color = channel_colors[i % len(channel_colors)]

                # Load original measurement WAV
                result = load_wav_file(wav_path)
                if result is None:
                    print(f"Warning: Could not load measurement WAV for {channel_name}: {wav_path}")
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
                        row=4,
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
                            row=4,
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

    # Update axes for frequency plots (rows 1, 2, and 3)
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
        range=[-eq_y_limit, eq_y_limit],
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

    # Configure impulse response axes if we have WAV files
    if has_ir_wavs:
        # Impulse response x-axis (linear time in ms)
        fig.update_xaxes(
            title_text="Time (ms)",
            title_font=dict(size=11),
            tickfont=dict(size=10),
            gridcolor="rgba(128, 128, 128, 0.2)",
            range=[0, ir_time_limit],
            row=4,
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
            row=4,
            col=1,
        )
        fig_height = 1250
        dropdown_y = 0.70
    else:
        fig_height = 950
        dropdown_y = 0.60

    # Update smoothing dropdown position
    if updatemenus:
        updatemenus[0]["y"] = dropdown_y

    fig.update_layout(
        height=fig_height,
        plot_bgcolor="white",
        paper_bgcolor="white",
        legend=dict(yanchor="top", y=0.99, xanchor="right", x=0.99, font=dict(size=10)),
        margin=dict(l=60, r=60, t=80, b=60),
        updatemenus=updatemenus,
    )

    return fig


def create_html_report(
    data: dict,
    output_path: Path,
    output_json_path: Path | None = None,
    input_data: dict | None = None,
    input_json_path: Path | None = None,
) -> None:
    """Create an HTML report with all channel plots.

    Args:
        data: Output JSON data (roomeq result)
        output_path: Path to write HTML report
        output_json_path: Path to output JSON (for resolving relative paths)
        input_data: Optional input JSON data (roomeq config with measurement WAV paths)
        input_json_path: Optional path to input JSON (for resolving relative paths)
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
    combined_fig = create_combined_figure(data, output_json_path, input_data, input_json_path)
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
measurement WAV paths, the IR graph shows corrected room impulse responses
(original room IR convolved with correction filters).
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
        help="Path to roomeq input JSON file (contains measurement WAV paths for corrected IR display)",
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

    # Check for measurement WAV files in input
    if input_data:
        input_dir = input_json_path.parent if input_json_path else Path(".")
        measurement_paths = extract_measurement_wav_paths(input_data, input_dir)
        if measurement_paths:
            print(f"Found {len(measurement_paths)} measurement WAV file(s) for corrected IR:")
            for ch_name, path in measurement_paths.items():
                exists = "exists" if path.exists() else "MISSING"
                print(f"  {ch_name}: {path} ({exists})")
        else:
            print("No measurement WAV paths found in input JSON")

    # Generate HTML report
    create_html_report(data, html_output_path, output_json_path, input_data, input_json_path)


if __name__ == "__main__":
    main()
