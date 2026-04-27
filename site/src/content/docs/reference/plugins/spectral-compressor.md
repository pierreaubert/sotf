---
title: "Spectral Compressor"
description: "Frequency-dependent compression operating in the spectral domain for transparent dynamic control."
---

Frequency-dependent compression operating in the spectral domain for transparent dynamic control.

## Parameters


### Analysis

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| FFT Size | Choice (1024, 2048, 4096) | 3 options | 2048 | - | FFT window size (higher = better frequency resolution, more latency) |
| Target | Choice (All, Tonal, Transient) | 3 options | All | - | Compress all bins, tonal only, or transient only |
| Adaptive Threshold | Bool | On / Off | Off | - | Auto-set threshold relative to long-term spectral average per bin |
| Adaptive Offset | Float | -20 .. 20 | 0 | dB | Offset from adaptive threshold (positive = less compression) |

### Dynamics

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Threshold | Float | -60 .. 0 | -20 | dB | Compression threshold per bin |
| Ratio | Float | 1 .. 20 | 2 | :1 | Compression ratio |
| Attack | Float | 0.1 .. 100 | 5 | ms | Per-bin attack time |
| Release | Float | 10 .. 1000 | 50 | ms | Per-bin release time |
| Knee | Float | 0 .. 20 | 6 | dB | Soft knee width |

### Quality

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Spectral Smooth | Float | 0 .. 1 | 0.3 | - | Frequency-axis smoothing (reduces musical artifacts) |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet blend |
| Delta Listen | Bool | On / Off | Off | - | Solo the compression delta (hear what's being removed) |

:::note
**Structural parameters** (FFT Size) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
