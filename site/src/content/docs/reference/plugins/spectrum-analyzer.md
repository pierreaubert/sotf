---
title: "Spectrum Analyzer"
description: "FFT-based spectrum analysis with configurable bin count, frequency range, smoothing, and tilt correction."
---

FFT-based spectrum analysis with configurable bin count, frequency range, smoothing, and tilt correction.

## Parameters

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Num Bins | Int | 8 .. 120 | 30 | - | Number of frequency bands |
| Min Freq | Float | 10 .. 100 | 20 | Hz | Lowest displayed frequency |
| Max Freq | Float | 5000 .. 22050 | 20000 | Hz | Highest displayed frequency |
| Smoothing | Float | 0 .. 1 | 0.7 | - | Temporal averaging factor |
| Tilt Correction | Choice (None, 3dB/oct, 6dB/oct, Pink) | 4 options | None | - | Slope compensation for display |
| Tilt Reference | Choice (Standard, 1kHz, 2kHz, Min Freq) | 4 options | Standard | - | Reference frequency for tilt |

:::note
**Structural parameters** (Num Bins) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
