---
title: "Upmixer"
description: "Stereo to surround upmixing (2ch to 5.0/5.1/7.1) using FFT-based spatial decomposition and VBAP panning."
---

Stereo to surround upmixing (2ch to 5.0/5.1/7.1) using FFT-based spatial decomposition and VBAP panning.

## Parameters


### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Speaker Config | Choice (2.0, 5.0, 5.1, 7.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, 9.1.6) | 10 options | 5.1 | - | Target surround speaker layout |
| Safety Cap | Float | 0 .. 3 | 3 | dB | Max output headroom limit |

### Gains

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Front Direct | Float | 0 .. 2 | 1 | x | Direct sound to front speakers |
| Front Ambient | Float | 0 .. 2 | 0.5 | x | Ambient sound to front speakers |
| Rear Ambient | Float | 0 .. 2 | 1 | x | Ambient sound to rear speakers |
| Height Gain | Float | 0 .. 2 | 1 | x | Level sent to height speakers |

### LFE

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| LFE Gain | Float | 0 .. 2 | 1 | x | Subwoofer channel level |
| LFE Cutoff | Float | 20 .. 180 | 120 | Hz | LFE low-pass filter frequency |
| Subharmonic Synth | Bool | On / Off | Off | - | Generate sub-bass harmonics |
| Sub Gain | Float | 0 .. 1 | 0.5 | x | Synthesized sub-bass level |
| Sub Freq | Float | 20 .. 80 | 40 | Hz | Sub-harmonic target frequency |
| Sub Attack | Float | 1 .. 100 | 10 | ms | Sub-harmonic envelope attack |
| Sub Release | Float | 10 .. 500 | 50 | ms | Sub-harmonic envelope release |

### Spatial

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Stereo Width | Float | 0 .. 1 | 0.5 | - | Front L/R separation amount |
| Center Spread | Float | 0 .. 1 | 0 | - | Center image width to L/R |
| Upmix Crossover | Float | 150 .. 350 | 250 | Hz | Direct/ambient split frequency |

### HR Direct

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| HR Direct | Bool | On / Off | On | - | Enable high-resolution direct path |
| HR Sharpen | Float | 0 .. 1 | 1 | - | Spatial image sharpening |
| Ambient Boost | Float | 0.5 .. 2 | 1.2 | x | Incoherent signal boost factor |

### Decorrelation

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Decor Mode | Choice (Velvet Noise, LFO Phase) | 2 options | Velvet Noise | - | Channel decorrelation method |
| Decor LFO Rate | Float | 0.01 .. 1 | 0.15 | Hz | LFO decorrelation modulation rate |
| Velvet Duration | Float | 10 .. 100 | 30 | ms | Velvet noise impulse length |
| Velvet Density | Float | 500 .. 5000 | 2000 | - | Velvet noise pulses per second |

### Height

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Height HF Cap | Float | 8000 .. 20000 | 16000 | Hz | High-frequency limit for heights |
| Height Trans Red | Float | 0 .. 1 | 0.6 | - | Soften transients in height feed |
| Height Direct Leak | Float | 0 .. 0.5 | 0.15 | - | Direct signal bleed into heights |

### Surround

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Surround Bleed | Float | 0 .. 1 | 0.5 | - | Direct signal bleed to surrounds |
| Rear Amb Boost | Float | 1 .. 3 | 1.5 | x | Extra gain for rear ambience |
| Rear Late Refl | Float | 0 .. 0.5 | 0.1 | - | Simulated rear room reflections |

### Dialogue

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Dialogue Weight | Float | 0 .. 1 | 0.4 | - | Voice routing to center channel |
| Voice Freq Min | Float | 200 .. 800 | 500 | Hz | Voice detection low bound |
| Voice Freq Max | Float | 2000 .. 5000 | 3000 | Hz | Voice detection high bound |
| Diag Centroid W | Float | 0 .. 1 | 0.3 | - | Spectral centroid score weight |
| Diag Variance W | Float | 0 .. 1 | 0.2 | - | Spectral variance score weight |
| Diag Coherence W | Float | 0 .. 1 | 0.5 | - | L/R coherence score weight |

### Analysis

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Low Latency | Bool | On / Off | Off | - | Smaller FFT for lower latency |
| Freq Resolution | Choice (ERB, Fine ERB, Per Bin) | 3 options | ERB | - | Frequency band grouping method |
| Multi-Source Extraction | Bool | On / Off | Off | - | Extract multiple sound sources |
| Multi-Source Threshold | Float | 0.05 .. 0.5 | 0.1 | - | Source separation sensitivity |

### Diagnostics

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Bypass Decor | Bool | On / Off | Off | - | Skip channel decorrelation |
| Bypass Transients | Bool | On / Off | Off | - | Skip transient detection |
| Bypass All | Bool | On / Off | Off | - | Pass audio through unprocessed |
| ML Detection | Bool | On / Off | Off | - | Use ML model for source detect |

:::note
**Structural parameters** (Speaker Config, Freq Resolution) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
