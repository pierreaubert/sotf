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
| Externalization | Float | 0 .. 1 | 0 | - | Out-of-head perception strength |
| Near-field | Float | 0 .. 1 | 0 | - | Near-field compensation amount |

### Quality

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Crossfade Mode | Choice (Linear, Spectral) | 2 options | Linear | - | Linear: simple blend (may cause tonal shift). Spectral: magnitude interpolation + phase reconstruction (smoother) |

### Room

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Late Reverb | Bool | On / Off | Off | - | Add FDN-based late reverb tail after early reflections |
| Reverb Mix | Float | 0 .. 1 | 0.3 | - | Late reverb wet/dry mix |
| Reverb Time | Float | 0.1 .. 5 | 1 | s | RT60 decay time for late reverb |
| Reverb Damping | Float | 0 .. 1 | 0.3 | - | High-frequency damping (0=bright, 1=dark) |

:::note
**Structural parameters** (SOFA File, Input Channels, Near-field) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
