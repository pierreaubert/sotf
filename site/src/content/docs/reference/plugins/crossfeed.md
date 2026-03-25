---
title: "Crossfeed"
description: "Headphone crossfeed that simulates speaker spacing. Supports Bauer, Meier, and multiband algorithms."
---

Headphone crossfeed that simulates speaker spacing. Supports Bauer, Meier, and multiband algorithms.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mode | Choice (Disable, Bauer, Meier, Multiband) | 4 options | Disable | - | Crossfeed algorithm selection |
| Preset | Choice (Default, Cmoy, Meier, Mb, Off) | 5 options | Default | - | Load preset parameter values |
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
