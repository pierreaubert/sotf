# math-dsp

DSP utilities for audio signal generation and FFT-based analysis.

**Library name:** `math_audio_dsp`
**Version:** 0.3.1

## Overview

This crate provides two core modules:

- **`signals`** -- Test signal generation (tones, sweeps, noise) with utility functions for fade, padding, and channel manipulation
- **`analysis`** -- FFT-based frequency analysis including spectrum computation (Welch's method and single-FFT), acoustic metrics (RT60, clarity C50/C80, THD), microphone compensation, and CSV I/O

## Signal Generation

Generate common test signals as `Vec<f32>`:

| Function | Description |
|---|---|
| `gen_tone(freq, amp, sample_rate, duration)` | Pure sine wave |
| `gen_two_tone(f1, a1, f2, a2, sample_rate, duration)` | Sum of two sines (IMD testing), auto-normalized to prevent clipping |
| `gen_log_sweep(f_start, f_end, amp, sample_rate, duration)` | Logarithmic frequency sweep for frequency response measurements |
| `gen_white_noise(amp, sample_rate, duration)` | Flat spectrum noise (deterministic LCG) |
| `gen_pink_noise(amp, sample_rate, duration)` | 1/f spectrum noise (Voss-McCartney / Paul Kellett) |
| `gen_m_noise(amp, sample_rate, duration)` | ITU-R 468 weighted noise (emphasis around 6.3 kHz) |

### Signal Utilities

| Function | Description |
|---|---|
| `apply_fade_in(signal, fade_samples)` | Hann window fade-in (in-place) |
| `apply_fade_out(signal, fade_samples)` | Hann window fade-out (in-place) |
| `add_silence_padding(signal, pre, post)` | Silence padding before/after |
| `prepare_signal_for_playback(signal, sr, fade_ms, pad_ms)` | Fade + padding in one call |
| `prepare_signal_for_playback_channels(...)` | Same with optional mono-to-stereo |
| `mono_to_stereo(signal)` | Duplicate mono to both channels |
| `interleave_per_channel(channels)` | Interleave per-channel buffers into one |
| `replicate_mono(signal, channels)` | Copy mono to N channels (interleaved) |
| `clip(x)` | Clamp to +/- 0.999999 |
| `frames_for(duration, sample_rate)` | Sample count for a given duration |

### Example

```rust
use math_audio_dsp::signals;

// Generate a 1 kHz tone at 0.5 amplitude, 48 kHz, 2 seconds
let tone = signals::gen_tone(1000.0, 0.5, 48000, 2.0);

// Generate a 20-20k Hz log sweep
let sweep = signals::gen_log_sweep(20.0, 20000.0, 0.8, 48000, 5.0);

// Prepare for playback with 20ms fade and 250ms silence padding
let ready = signals::prepare_signal_for_playback(tone, 48000, 20.0, 250.0);
```

## Analysis

### Standalone WAV Analysis

Analyze a WAV file or sample buffer and get frequency response (magnitude + phase) on a log-spaced frequency grid:

```rust
use math_audio_dsp::analysis::{WavAnalysisConfig, analyze_wav_buffer, analyze_wav_file};

// Analyze a buffer
let config = WavAnalysisConfig::default(); // Welch's method, 2000 points, 20-20kHz
let result = analyze_wav_buffer(&samples, 48000, &config)?;
// result.frequencies, result.magnitude_db, result.phase_deg

// Analyze a WAV file directly
let result = analyze_wav_file(Path::new("recording.wav"), &config)?;
```

Pre-built configs for common use cases:

| Config | Method | Use case |
|---|---|---|
| `WavAnalysisConfig::default()` | Welch's (averaged periodograms) | Stationary signals (music, noise) |
| `WavAnalysisConfig::for_log_sweep()` | Single FFT + pink compensation | Log sweep measurements |
| `WavAnalysisConfig::for_impulse_response()` | Single FFT | Impulse response analysis |

Configuration fields: `num_points`, `min_freq`, `max_freq`, `fft_size`, `overlap`, `single_fft`, `pink_compensation`, `no_window`.

### Recording Analysis (Reference vs Recorded)

Compare a recorded WAV against a known reference signal to extract the system's transfer function:

```rust
use math_audio_dsp::analysis::analyze_recording;

let result = analyze_recording(
    Path::new("recorded.wav"),
    &reference_signal,
    48000,
    Some((20.0, 20000.0)), // sweep range for THD computation
)?;
```

The `AnalysisResult` includes:

| Field | Description |
|---|---|
| `frequencies`, `spl_db`, `phase_deg` | Frequency response (2000 log-spaced points, 20-20kHz) |
| `estimated_lag_samples` | Latency between reference and recording (FFT cross-correlation) |
| `impulse_response`, `impulse_time_ms` | Time-domain impulse response |
| `excess_group_delay_ms` | Excess group delay |
| `thd_percent` | Total Harmonic Distortion (Farina's method from log sweep IR) |
| `harmonic_distortion_db` | Per-harmonic distortion curves (H2-H5) |
| `rt60_ms` | RT60 reverberation time (T20 extrapolation, octave-band filtered) |
| `clarity_c50_db`, `clarity_c80_db` | Speech/music clarity metrics |
| `spectrogram_db` | Time-frequency spectrogram |

### Acoustic Metrics

Available as standalone functions for custom impulse responses:

```rust
use math_audio_dsp::analysis::*;

// Broadband RT60 (T20 extrapolation from Schroeder decay)
let rt60 = compute_rt60_broadband(&impulse, 48000.0);

// Broadband clarity
let (c50, c80) = compute_clarity_broadband(&impulse, 48000.0);

// Frequency-dependent RT60 and clarity (octave-band filtered, interpolated)
let rt60_spectrum = compute_rt60_spectrum(&impulse, 48000.0, &frequencies);
let (c50_spectrum, c80_spectrum) = compute_clarity_spectrum(&impulse, 48000.0, &frequencies);

// Spectrogram
let (matrix_db, freq_bins, time_bins) = compute_spectrogram(&impulse, 48000.0, 512, 128);

// Group delay from phase data
let group_delay = compute_group_delay(&frequencies, &phase_deg);
```

### Microphone Compensation

Load calibration data and apply inverse compensation to measurements:

```rust
use math_audio_dsp::analysis::MicrophoneCompensation;

// Load from CSV (freq_hz,spl_db) or TXT (space/tab-separated)
let mic = MicrophoneCompensation::from_file(Path::new("mic_cal.csv"))?;

// Interpolate at a specific frequency
let deviation_db = mic.interpolate_at(1000.0);

// Pre-compensate a sweep signal
let compensated = mic.apply_to_sweep(&sweep, 20.0, 20000.0, 48000, true);
```

### Smoothing and CSV I/O

```rust
use math_audio_dsp::analysis::*;

// Octave smoothing (f32 and f64 variants)
let smoothed = smooth_response_f32(&frequencies, &magnitudes, 1.0 / 24.0);

// Write/read analysis CSV
write_wav_analysis_csv(&wav_result, Path::new("output.csv"))?;
write_analysis_csv(&recording_result, Path::new("output.csv"), Some(&mic_compensation))?;
let loaded = read_analysis_csv(Path::new("output.csv"))?;
```

## Binary: `wav2csv`

CLI tool to analyze a WAV file and output frequency/SPL/phase as CSV.

```bash
# Default (Welch's method)
wav2csv recording.wav

# Log sweep analysis
wav2csv sweep.wav --single-fft --pink-compensation --no-window

# Custom parameters
wav2csv recording.wav -o output.csv -n 4000 --min-freq 10 --max-freq 24000 --fft-size 32768
```

Options: `--num-points`, `--min-freq`, `--max-freq`, `--fft-size`, `--overlap`, `--single-fft`, `--pink-compensation`, `--no-window`, `-o`/`--output`.

## Dependencies

| Crate | Purpose |
|---|---|
| `rustfft` | FFT computation |
| `num-complex` | Complex number types |
| `hound` | WAV file I/O |
| `math-iir-fir` | Biquad filters (bandpass for octave-band analysis) |
| `log` | Logging |
| `clap` | CLI argument parsing (for `wav2csv`) |

## Testing

```bash
cargo test -p math-dsp --lib
cargo check -p math-dsp && cargo clippy -p math-dsp
```
