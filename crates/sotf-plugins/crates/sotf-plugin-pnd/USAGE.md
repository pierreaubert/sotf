# PND (Polyphonic Note Detection & Drift Correction)

## Overview

Detects and corrects subtle pitch drift in audio playback using FFT-based polyphonic pitch analysis and real-time varispeed resampling. Designed for correcting clock drift between source and playback devices, or compensating for speed instabilities in analog transfers.

## Features

### Parameters

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Correction Strength | 0 to 200 | 100 | % | How much of the detected drift to correct. 0% = monitoring only, 100% = full correction, 200% = overcorrection |
| Analysis Window | 20 to 500 | 100 | ms | FFT analysis window length. Longer = more accurate but slower to react |
| Drift Smoothing | 1 to 1000 | 100 | ×0.001 | Smoothing factor for drift estimate. Higher = more stable correction, lower = faster tracking |

### Monitoring

Real-time data exposed for display:
- **Drift Ratio**: Current raw drift ratio from analysis (1.0 = no drift)
- **Correction Ratio**: Correction ratio applied to the resampler
- **Confidence**: Confidence of the drift estimate (0.0 to 1.0)
- **Matched Partials**: Number of matched harmonic partials in the last FFT frame
- **Total Peaks**: Total detected spectral peaks

### Pitch Analysis

The analyzer uses FFT peak detection with parabolic interpolation to identify spectral peaks, then matches them against harmonic series to detect pitch drift. The algorithm:
1. Identifies spectral peaks in each FFT frame
2. Matches peaks to harmonic series (fundamental + overtones)
3. Estimates the drift ratio from matched partials
4. Applies exponential smoothing to the estimate

### Varispeed Resampling

Correction is applied via high-quality asynchronous resampling (rubato) that adjusts playback speed in real-time. Block buffering with ring buffers handles arbitrary host block sizes while maintaining the fixed resampler chunk size.

## Demos

### Demo: Clock Drift Correction

**Scenario:** Source material was recorded at slightly off-speed (e.g., 44100 Hz source playing back at 48000 Hz with imprecise conversion).
**Before:** Gradual pitch drift accumulates over the duration of playback.
**After:** PND detects and corrects the drift in real-time, maintaining correct pitch.
**Config:**
```json
{
  "correction_strength": 100.0,
  "analysis_window_ms": 100.0,
  "drift_smoothing": 0.1
}
```

### Demo: Monitoring Only

**Scenario:** Investigating whether pitch drift exists without applying correction.
**Before:** Suspected drift but no measurement tool.
**After:** Real-time drift ratio and confidence displayed — correction disabled.
**Config:**
```json
{
  "correction_strength": 0.0,
  "analysis_window_ms": 200.0,
  "drift_smoothing": 0.05
}
```

## Presets

### Standard Correction
**Use case:** General-purpose pitch drift correction
```json
{
  "correction_strength": 100.0,
  "analysis_window_ms": 100.0,
  "drift_smoothing": 0.1
}
```
**Tips:** Works well for most audio with tonal content. Increase analysis window for very low-pitched material.

### Gentle Correction
**Use case:** Subtle drift correction with minimal artifacts
```json
{
  "correction_strength": 50.0,
  "analysis_window_ms": 200.0,
  "drift_smoothing": 0.05
}
```
**Tips:** Lower correction strength and higher smoothing reduce the risk of over-correction on complex material.

## Tips & Best Practices

- PND works best with tonal content (music with sustained notes). Percussive-only material may have low confidence.
- The correction strength parameter changes are smoothed over 50 ms to prevent audible pitch jumps.
- A longer analysis window improves accuracy for low-frequency content but increases latency and reduces responsiveness.
- Higher drift smoothing values produce more stable corrections but are slower to track rapid changes.
- Monitor the confidence value — low confidence indicates the algorithm is uncertain and correction may be unreliable.
- The plugin uses block buffering with ring buffers to handle arbitrary host frame sizes internally.

## Signal Flow

```
Input → Ring buffer accumulation
  → FFT analysis (peak detection + harmonic matching)
  → Drift ratio estimation with exponential smoothing
  → Varispeed resampler (rubato async) with corrected ratio
  → Ring buffer output → Output
```
