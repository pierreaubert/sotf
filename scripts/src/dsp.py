"""DSP computation functions for biquad filters, crossovers, and IR processing."""

import math
from pathlib import Path

import numpy as np

try:
    from scipy import signal as scipy_signal
    HAS_SCIPY = True
except ImportError:
    HAS_SCIPY = False

from .loaders import load_wav_file


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

    # Normalize output names: accept both "low"/"lowpass" and "high"/"highpass"
    if output in ("low", "lowpass"):
        output = "lowpass"
    elif output in ("high", "highpass"):
        output = "highpass"

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


def compute_plugin_chain_response(
    plugins: list[dict],
    freq_points: list[float],
    sample_rate: float = 48000.0,
    json_dir: Path | None = None,
) -> list[float]:
    """Compute the total magnitude correction in dB by walking the plugin chain in order.

    For SPL (magnitude) plots, all effects are additive in dB:
    - gain: flat +dB offset
    - eq: biquad filter response in dB
    - crossover: LP/HP filter response in dB
    - convolution: FIR frequency response in dB
    - delay: phase-only, no magnitude effect — skipped

    Args:
        plugins: Ordered list of plugin dicts from the channel's plugin chain
        freq_points: Frequency points to evaluate at (Hz)
        sample_rate: Sample rate in Hz
        json_dir: Directory for resolving relative IR file paths

    Returns:
        Cumulative correction in dB at each frequency point
    """
    correction_db = [0.0] * len(freq_points)

    for plugin in plugins:
        ptype = plugin.get("plugin_type", "")
        params = plugin.get("parameters", {})

        if ptype == "gain":
            gain_db = params.get("gain_db", 0.0)
            correction_db = [c + gain_db for c in correction_db]

        elif ptype == "eq":
            filters = params.get("filters", [])
            if filters:
                eq_db = compute_eq_response(filters, freq_points, sample_rate)
                correction_db = [c + e for c, e in zip(correction_db, eq_db)]

        elif ptype == "crossover":
            xover_type = params.get("type", "LinkwitzRiley4")
            xover_freq = params.get("frequency", 1000.0)
            xover_output = params.get("output", "lowpass")
            xover_db = compute_crossover_response(
                xover_type, xover_freq, xover_output, freq_points, sample_rate
            )
            correction_db = [c + x for c, x in zip(correction_db, xover_db)]

        elif ptype == "convolution":
            ir_file = params.get("ir_file")
            if ir_file:
                ir_path = Path(ir_file)
                if not ir_path.is_absolute() and json_dir is not None:
                    ir_path = json_dir / ir_path
                fir_response = compute_fir_frequency_response(ir_path, freq_points)
                if fir_response is not None:
                    correction_db = [c + f for c, f in zip(correction_db, fir_response)]

        elif ptype == "delay":
            pass  # Phase-only, no magnitude effect

    return correction_db
