"""File I/O functions for loading roomeq data files."""

import json
import sys
import wave
from pathlib import Path

import numpy as np

try:
    from scipy.io import wavfile as scipy_wavfile
    HAS_SCIPY = True
except ImportError:
    HAS_SCIPY = False


def load_roomeq_json(filepath: Path) -> dict:
    """Load and parse roomeq JSON output file."""
    if not filepath.exists():
        print(f"Error: File not found: {filepath}")
        sys.exit(1)
    with open(filepath, "r") as f:
        return json.load(f)


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
            sr, data = scipy_wavfile.read(str(wav_path))
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


def load_csv_measurement(csv_path: Path) -> tuple[list[float], list[float], list[float] | None] | None:
    """
    Load a measurement CSV file with freq, spl, and optional phase columns.

    Args:
        csv_path: Path to the CSV file

    Returns:
        Tuple of (freq, spl, phase_or_none) or None if file cannot be loaded.
        Phase is in degrees, or None if not present in the CSV.
    """
    if not csv_path.exists():
        print(f"Warning: CSV file not found: {csv_path}")
        return None

    try:
        import csv

        freq = []
        spl = []
        phase = []
        has_phase = False

        with open(csv_path, "r") as f:
            reader = csv.reader(f)
            header = next(reader, None)
            if header is None:
                print(f"Warning: Empty CSV file: {csv_path}")
                return None

            # Normalize header names
            header_lower = [h.strip().lower() for h in header]

            # Find column indices
            freq_idx = None
            spl_idx = None
            phase_idx = None

            for i, h in enumerate(header_lower):
                if h in ("freq", "frequency", "hz"):
                    freq_idx = i
                elif h in ("spl", "db", "magnitude", "level"):
                    spl_idx = i
                elif h in ("phase", "phase_deg", "deg"):
                    phase_idx = i

            if freq_idx is None or spl_idx is None:
                print(f"Warning: CSV missing required 'freq' and/or 'spl' columns: {csv_path}")
                return None

            has_phase = phase_idx is not None

            for row in reader:
                if len(row) <= max(freq_idx, spl_idx):
                    continue
                try:
                    freq.append(float(row[freq_idx]))
                    spl.append(float(row[spl_idx]))
                    if has_phase and phase_idx is not None and len(row) > phase_idx:
                        phase.append(float(row[phase_idx]))
                except (ValueError, IndexError):
                    continue

        if not freq or not spl:
            print(f"Warning: No valid data in CSV: {csv_path}")
            return None

        return freq, spl, phase if has_phase and len(phase) == len(freq) else None

    except Exception as e:
        print(f"Warning: Failed to load CSV file {csv_path}: {e}")
        return None


def synthesize_ir_from_measurement(
    freq: list[float],
    spl: list[float],
    phase: list[float] | None,
    sample_rate: int = 48000,
    n_samples: int = 4096,
) -> tuple[np.ndarray, np.ndarray, int]:
    """
    Synthesize a time-domain impulse response from frequency-domain measurement data.

    Uses actual phase data if available, otherwise computes minimum phase from magnitude.

    Args:
        freq: Frequency points in Hz
        spl: SPL values in dB
        phase: Phase values in degrees, or None for minimum-phase synthesis
        sample_rate: Sample rate for the synthesized IR
        n_samples: Number of samples in the output IR

    Returns:
        Tuple of (time_ms, ir_samples, sample_rate)
    """
    n_fft = n_samples
    freq_bins = np.fft.rfftfreq(n_fft, d=1.0 / sample_rate)

    freq_arr = np.array(freq, dtype=np.float64)
    spl_arr = np.array(spl, dtype=np.float64)

    # Interpolate magnitude (dB) to FFT frequency grid
    mag_db = np.interp(freq_bins, freq_arr, spl_arr,
                       left=float(spl_arr[0]), right=float(spl_arr[-1]))

    # Convert dB to linear magnitude
    mag_linear = 10.0 ** (mag_db / 20.0)

    if phase is not None and len(phase) == len(freq):
        # Use actual measured phase
        phase_arr = np.array(phase, dtype=np.float64)
        phase_rad = np.interp(freq_bins, freq_arr, np.radians(phase_arr),
                              left=float(np.radians(phase_arr[0])),
                              right=float(np.radians(phase_arr[-1])))
    else:
        # Compute minimum phase from magnitude using cepstral method
        log_mag = np.log(np.maximum(mag_linear, 1e-10))
        cepstrum = np.fft.irfft(log_mag)

        n = len(cepstrum)
        min_cep = np.zeros(n)
        min_cep[0] = cepstrum[0]
        min_cep[1:n // 2] = 2.0 * cepstrum[1:n // 2]
        if n % 2 == 0:
            min_cep[n // 2] = cepstrum[n // 2]

        analytic = np.fft.rfft(min_cep)
        phase_rad = np.imag(analytic)

    # Build complex spectrum and inverse FFT
    spectrum = mag_linear * np.exp(1j * phase_rad)
    ir = np.fft.irfft(spectrum, n=n_fft)

    # Normalize
    max_val = np.max(np.abs(ir))
    if max_val > 0:
        ir = ir / max_val

    time_ms = np.arange(len(ir)) / sample_rate * 1000.0

    return time_ms, ir, sample_rate


def load_measurement_ir(measurement_path: Path, sample_rate: int = 48000) -> tuple[np.ndarray, np.ndarray, int] | None:
    """
    Load a measurement file and return impulse response data.

    Supports both WAV files (time-domain IR) and CSV files (frequency-domain,
    synthesized to time-domain IR).

    Args:
        measurement_path: Path to measurement file (.wav or .csv)
        sample_rate: Sample rate for synthesized IR (only used for CSV)

    Returns:
        Tuple of (time_ms, samples, sample_rate) or None if unable to load
    """
    suffix = measurement_path.suffix.lower()

    if suffix == ".wav":
        return load_wav_file(measurement_path)

    if suffix == ".csv":
        result = load_csv_measurement(measurement_path)
        if result is None:
            return None
        freq, spl_values, phase = result
        return synthesize_ir_from_measurement(freq, spl_values, phase, sample_rate)

    print(f"Warning: Unsupported measurement file format: {measurement_path}")
    return None
