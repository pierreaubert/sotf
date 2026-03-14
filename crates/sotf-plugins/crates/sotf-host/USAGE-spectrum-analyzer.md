# Spectrum Analyzer

## Overview

A real-time FFT-based spectrum analyzer that displays the frequency content of audio as it plays. Passes audio through unmodified — it only extracts data for visualization. Uses a 4096-point FFT with Hann windowing, logarithmic frequency binning, and configurable smoothing.

## Features

### Display Configuration

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Bins | 10 to 100 | 30 | — | Number of display bins (frequency resolution) |
| Min Freq | 10 to 500 | 20 | Hz | Lower frequency bound |
| Max Freq | 1000 to 22050 | 20000 | Hz | Upper frequency bound |
| Smoothing | 0 to 0.999 | 0.7 | — | Temporal smoothing (0 = instant, higher = slower decay) |

### Tilt Correction

Compensates for the natural spectral slope of audio content so the display better represents perceived loudness per band.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Tilt Correction | None/3dB-oct/6dB-oct/Pink | None | — | Spectral tilt compensation |
| Tilt Reference | Standard/1kHz/2kHz/Min Freq | Standard | — | Reference frequency for tilt normalization |

### Frequency Binning

Bins are distributed logarithmically between Min Freq and Max Freq, matching human pitch perception. Each display bin shows the peak magnitude of all FFT bins that map to it. The geometric mean of each bin's frequency range is used as its center frequency.

### Color Coding

The spectrum display uses level-based coloring:
- **Green:** Below -6 dB (safe zone)
- **Yellow:** -6 dB to -1 dB (caution zone)
- **Red:** Above -1 dB (clipping danger)

## Demos

### Demo: Full-Range Music Analysis

**Scenario:** Monitoring the spectral balance of a music track during playback.
**Before:** No visibility into frequency distribution.
**After:** Real-time visualization shows bass energy, midrange content, and treble balance.
**Config:**
```json
{
  "num_bins": 30,
  "min_freq": 20.0,
  "max_freq": 20000.0,
  "smoothing": 0.7
}
```

### Demo: Bass-Focused Monitoring

**Scenario:** Checking sub-bass content and low-end balance.
**Before:** Hard to tell if sub-bass is present or excessive.
**After:** Zoomed-in view of the low end reveals sub-bass energy distribution.
**Config:**
```json
{
  "num_bins": 40,
  "min_freq": 20.0,
  "max_freq": 500.0,
  "smoothing": 0.5
}
```

### Demo: High-Resolution Analysis

**Scenario:** Detailed spectral analysis with many bins for precise frequency identification.
**Before:** Coarse 30-bin view doesn't show narrow peaks or dips.
**After:** 100-bin view reveals individual resonances and spectral details.
**Config:**
```json
{
  "num_bins": 100,
  "min_freq": 20.0,
  "max_freq": 20000.0,
  "smoothing": 0.5
}
```

## Presets

### Standard
**Use case:** General-purpose spectrum monitoring
```json
{
  "num_bins": 30,
  "min_freq": 20.0,
  "max_freq": 20000.0,
  "smoothing": 0.7
}
```
**Tips:** Good balance between resolution and readability. Higher smoothing keeps the display calm.

### High Resolution
**Use case:** Detailed spectral analysis
```json
{
  "num_bins": 80,
  "min_freq": 20.0,
  "max_freq": 20000.0,
  "smoothing": 0.5
}
```
**Tips:** More bins reveal finer spectral detail. Lower smoothing shows transients better.

### Sub-Bass Monitor
**Use case:** Focus on low frequencies
```json
{
  "num_bins": 40,
  "min_freq": 20.0,
  "max_freq": 500.0,
  "smoothing": 0.6
}
```
**Tips:** Narrow frequency range with more bins gives excellent low-end resolution.

## Tips & Best Practices

- The analyzer does not modify audio — it's purely a visualization tool.
- Higher smoothing values (0.7+) give a calmer, more readable display.
- Lower smoothing values (0.3–0.5) are better for spotting transients and fast changes.
- More bins = more frequency detail but smaller bars. 30 bins is good for overview, 80+ for analysis.
- Use Tilt Correction to compensate for the natural pink-noise slope of music — makes the display flatter for typical content.
- Multi-channel audio is downmixed to mono for analysis.
- The FFT uses a 4096-point Hann window, giving ~11.7 Hz resolution at 48 kHz sample rate.

## Signal Flow

```
Input (passthrough) → Mono Downmix → Ring Buffer → FFT (4096-point, Hann window)
                                                        ↓
                                                   Magnitude (dB)
                                                        ↓
                                                   Log Binning (N bins)
                                                        ↓
                                                   Temporal Smoothing
                                                        ↓
                                                   Display Data (via RealTimeCache)

Output = Input (unmodified)
```
