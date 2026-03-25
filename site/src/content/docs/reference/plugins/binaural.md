---
title: "Binaural Renderer"
description: "HRTF-based 3D spatial audio rendering. Converts multichannel audio to binaural headphone output using SOFA files."
---

HRTF-based 3D spatial audio rendering. Converts multichannel audio to binaural headphone output using SOFA files.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| SOFA File | File Path | - | - | - | HRTF data file (SOFA format) |
| Input Channels | Int | 2 .. 16 | 2 | ch | Number of surround input channels |
| Optimization | Bool | On / Off | On | - | Enable HRIR filter optimization |
| Externalization | Float | 0 .. 1 | 0 | - | Out-of-head perception strength |
| Near-field | Float | 0 .. 1 | 0 | - | Near-field compensation amount |

### Quality

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Crossfade Mode | Choice (Linear, Spectral) | 2 options | Linear | - | Linear: simple blend (may cause tonal shift). Spectral: magnitude interpolation + phase reconstruction (smoother) |

:::note
**Structural parameters** (SOFA File, Input Channels, Optimization, Near-field) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
