---
title: "Linear Phase EQ"
description: "Zero-phase parametric equalizer using FFT convolution. No phase shift, but adds latency."
---

Zero-phase parametric equalizer using FFT convolution. No phase shift, but adds latency.

## Parameters


### EQ

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Num Filters | Int | 1 .. 10 | 5 | - | Number of EQ bands |

### Quality

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| FIR Length | Choice (1024, 2048, 4096, 8192) | 4 options | 2048 | - | FIR length in taps (higher = better bass resolution, more latency) |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Auto Gain | Bool | On / Off | Off | - | Compensate output level |
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet mix |

:::note
**Structural parameters** (Num Filters) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
