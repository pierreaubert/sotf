---
title: "Denoiser"
description: "Audio denoising using MCRA (Minima Controlled Recursive Averaging) and Wiener filtering."
---

Audio denoising using MCRA (Minima Controlled Recursive Averaging) and Wiener filtering.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Reduction | Float | 0 .. 40 | 10 | dB | Noise attenuation amount |
| Floor | Float | -60 .. -10 | -20 | dB | Minimum gain floor (artifact limit) |
| Smoothing | Float | 0 .. 0.99 | 0.3 | - | Gain curve temporal smoothing |
| Low Latency | Bool | On / Off | Off | - | Smaller FFT for lower latency |
| Transparency | Float | 0 .. 1 | 0 | - | Blend denoised toward dry signal |
| Algorithm | Choice (Classical, RNNoise, DeepFilter, HybridNeural) | 4 options | Classical | - | Denoising algorithm selection |
| Multi-Res | Bool | On / Off | Off | - | Multi-resolution FFT analysis |

### Timing

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Attack | Float | 0.1 .. 100 | 5 | ms | Time to apply reduction |
| Release | Float | 10 .. 500 | 50 | ms | Time to release reduction |

### Analysis

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Polyphonic | Bool | On / Off | Off | - | Detect multiple pitched signals |
| Crack Sens. | Float | 1 .. 100 | 10 | - | Click/crack detection sensitivity |
| DD SNR | Bool | On / Off | On | - | Decision-Directed SNR estimator |
| DD Alpha | Float | 0.5 .. 0.999 | 0.98 | - | DD SNR smoothing coefficient |
| Psychoacoustic | Bool | On / Off | On | - | Use auditory masking curves |
| Transient | Bool | On / Off | On | - | Preserve transient details |
| Spectral Smooth | Bool | On / Off | On | - | Smooth gain across frequency bins |
| Temporal Smooth | Bool | On / Off | On | - | Smooth gain across time frames |

### Advanced

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| MCRA Alpha S | Float | 0.5 .. 0.99 | 0.9 | - | Noise spectrum smoothing factor |
| MCRA Alpha P | Float | 0.1 .. 0.99 | 0.7 | - | Speech presence probability smooth |
| MCRA Window | Int | 10 .. 200 | 50 | fr | Min statistics window length |
| MCRA Delta | Float | 1 .. 20 | 5 | - | Speech/noise discrimination bias |

### Hiss

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Hiss Remover | Bool | On / Off | Off | - | Enable dedicated hiss reduction |
| Hiss Threshold | Float | -60 .. -10 | -30 | dB | Level above which hiss is removed |
| Hiss Frequency | Float | 1000 .. 16000 | 4000 | Hz | Corner freq for hiss detection |
| Hiss Strength | Float | 0 .. 1 | 0.5 | - | Hiss removal aggressiveness |

### Spectral Sub

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Spectral Sub | Bool | On / Off | Off | - | Enable spectral subtraction |
| Oversub Factor | Float | 0.5 .. 6 | 2 | - | Over-subtraction factor (alpha) |
| Spectral Floor | Float | 0.001 .. 0.5 | 0.02 | - | Spectral floor factor (beta) |

### Noise Profile

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Learn Noise | Bool | On / Off | Off | - | Capture noise-only reference |
| Use Profile | Bool | On / Off | Off | - | Use captured noise profile |
| Clear Profile | Bool | On / Off | Off | - | Discard captured noise profile |

### Formant

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Formant Preserve | Bool | On / Off | Off | - | Protect vocal formant structure |
| Formant Strength | Float | 0 .. 1 | 0.5 | - | Formant preservation amount |

:::note
**Structural parameters** (Low Latency, Learn Noise, Clear Profile, Algorithm, Multi-Res) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
