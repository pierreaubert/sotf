---
title: Spectrum Analyzer Screen
description: Real-time FFT-based frequency spectrum display.
---

The Spectrum screen shows a real-time frequency spectrum of the audio currently being
processed. It updates continuously during playback.

## Display

The spectrum is displayed as a frequency-domain graph:
- **X axis** — frequency (logarithmic scale, 20 Hz – 20 kHz)
- **Y axis** — level in dBFS
- **Curve** — instantaneous spectrum with configurable peak-hold

The analyzer reads signal data after the full plugin chain, so it reflects the
post-EQ frequency content.

## Controls

The Spectrum Analyzer plugin (which powers this screen) can be configured directly:

| Parameter | Description |
|-----------|-------------|
| **FFT size** | Frequency resolution (larger = finer bins, higher latency) |
| **Smoothing** | Time-domain averaging to reduce jitter |
| **Tilt** | Adds a rising shelf to compensate for pink noise statistics |
| **Peak hold** | Duration peaks are held before falling |
| **Min / Max dB** | Vertical scale range |

Add or adjust the Spectrum Analyzer plugin from the **Plugins** screen. See
[Spectrum Analyzer plugin reference](/reference/plugins/spectrum-analyzer/) for all parameters.

## Usage Tips

- Use the spectrum to visually verify EQ changes — filter peaks and dips should be visible
- Enable **tilt** correction when analyzing pink noise or music to get a visually "flat" display
- Comparing spectrum before and after an EQ plugin is easier with the A/B Compare plugin
