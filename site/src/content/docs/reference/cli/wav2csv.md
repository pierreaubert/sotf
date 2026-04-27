---
title: wav2csv
description: Analyze a WAV file and output frequency response as a CSV (frequency, SPL, phase).
---

Converts a WAV recording into a frequency/SPL/phase CSV suitable for use as a
measurement input to `autoeq` or `roomeq`.

## Synopsis

```
wav2csv <INPUT> [OPTIONS]
```

## Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `INPUT` | Yes | Input WAV file |

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| `-o` / `--output` | *(input with `.csv`)* | Output CSV file path |
| `-n` / `--num-points` | `2000` | Number of frequency points in the output |
| `--min-freq` | `20.0` | Minimum frequency (Hz) |
| `--max-freq` | `20000.0` | Maximum frequency (Hz) |
| `--fft-size` | auto | FFT size (default chosen from file length) |
| `--overlap` | `0.5` | Window overlap ratio for Welch's method (0.0–1.0) |
| `--single-fft` | off | Use a single FFT instead of Welch's averaged method |
| `--pink-compensation` | off | Apply −3 dB/octave compensation (for log sweeps) |
| `--no-window` | off | Use rectangular window instead of Hann |

## Choosing the Right Mode

The correct flag combination depends on the type of signal in the WAV file:

| Signal type | Recommended flags |
|-------------|-------------------|
| Music / stationary noise | *(defaults — Welch's method with Hann window)* |
| Log sweep (room measurement) | `--single-fft --pink-compensation --no-window` |
| Impulse response | `--single-fft` |

## Output Format

The CSV has three columns:

```csv
freq,spl,phase
20.0,68.3,12.5
25.1,71.0,9.8
...
20000.0,55.2,-87.4
```

This matches the format accepted by both `autoeq --curve` and `roomeq` speaker measurement paths.

## Examples

Analyze a room measurement sweep:
```bash
wav2csv room-sweep.wav --single-fft --pink-compensation --no-window
# writes room-sweep.csv
```

Analyze with a custom output path and frequency range:
```bash
wav2csv measurement.wav -o speaker-left.csv --min-freq 50 --max-freq 16000
```

Convert an impulse response:
```bash
wav2csv ir.wav --single-fft -o ir.csv
```

## Typical Workflow

```bash
# 1. Record a log sweep with generate-audio-tests or your measurement software
generate-audio-tests --signals sweep --channels 1 --sample-rates 48000 --bits 24

# 2. Play the sweep through your speaker and record the microphone input
#    (use SotF Recording screen or any DAW)

# 3. Convert the recording to CSV
wav2csv recorded-sweep.wav --single-fft --pink-compensation --no-window

# 4. Feed the CSV into autoeq or roomeq
autoeq --curve recorded-sweep.csv --loss speaker-flat -n 7
```
