---
title: "Convolution"
description: "FFT-based convolution engine for applying impulse responses (room correction, cabinet simulation, reverb)."
---

FFT-based convolution engine for applying impulse responses (room correction, cabinet simulation, reverb).

## Parameters

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| IR File | File Path | - | - | - | Impulse response WAV file path |
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet blend |
| Gain | Float | -20 .. 20 | 0 | dB | Output level trim |
| Use NUPC | Bool | On / Off | On | - | Non-uniform partitioned convolution |

:::note
**Structural parameters** (IR File, Use NUPC) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
