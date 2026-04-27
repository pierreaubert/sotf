---
title: "Hiss Reducer"
description: "Reduces high-frequency hiss and tape noise while preserving program content."
---

Reduces high-frequency hiss and tape noise while preserving program content.

## Parameters

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Enabled | Bool | On / Off | On | - | Enable high-frequency hiss reduction |
| Threshold | Float | -60 .. -10 | -30 | dB | SNR threshold for stationary hiss detection |
| Frequency | Float | 1000 .. 16000 | 4000 | Hz | Frequency above which hiss reduction applies |
| Strength | Float | 0 .. 1 | 0.5 | - | Hiss attenuation strength |
| Low Latency | Bool | On / Off | Off | - | Use a smaller FFT |

:::note
**Structural parameters** (Low Latency) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
