---
title: "Crossfeed"
description: "Headphone crossfeed with Bauer, Meier, multiband, and compact HRTF algorithms."
---

Headphone crossfeed that simulates speaker spacing. HRTF mode uses a fixed
700 Hz head-shadow low-pass, 0.25 ms base ITD, and -9 dB cross-ear path with
equal near-ear subtraction. It preserves mono fold and reports zero latency;
it is not a personalized or measured SOFA renderer.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mode | Choice (Disable, Bauer, Meier, Multiband, HRTF) | 5 options | Multiband | - | Crossfeed algorithm selection |
| Preset | Choice (Default, Cmoy, Meier, Mb, Off, HRTF) | 6 options | Default | - | Load preset parameter values |
| Enabled | Bool | On / Off | On | - | Enable crossfeed processing |
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet blend |
| ITD Delay | Float | 0 .. 1 | 0 | ms | Interaural time difference |

### Bauer

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Bauer Cutoff | Float | 400 .. 1000 | 700 | Hz | Bauer shelving filter frequency |
| Bauer Feed | Float | 0 .. 15 | 4.5 | dB | Bauer cross-feed level |

### Meier

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Meier Level | Float | 0 .. 100 | 30 | % | Meier crossfeed strength |

### Multiband

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| MB Low Freq | Float | 50 .. 500 | 150 | Hz | Low/mid band split frequency |
| MB Mid/High Freq | Float | 2000 .. 15000 | 5700 | Hz | Mid/high band split frequency |
| MB Low Feed | Float | -20 .. 0 | 0 | dB | Low band cross-feed level |
| MB Mid Feed | Float | 0 .. 15 | 6 | dB | Mid band cross-feed level |
| MB High Feed | Float | 0 .. 15 | 3 | dB | High band cross-feed level |

### Auto Gain

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Auto Gain | Bool | On / Off | Off | - | Auto-normalize output level |
| Target LUFS | Float | -40 .. -12 | -18 | LUFS | Target loudness level |
| Max Gain | Float | 0 .. 24 | 12 | dB | Maximum auto gain correction |
| Smoothing | Float | 10 .. 5000 | 100 | ms | Auto gain transition time |

:::note
**Structural parameters** (Mode, Preset) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
