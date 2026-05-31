---
title: "Parametric EQ"
description: "Biquad-based parametric equalizer with peak, shelf, and pass filters. Supports multiple filter bands for precise frequency response shaping."
---

Biquad-based parametric equalizer with peak, shelf, and pass filters. Supports multiple filter bands for precise frequency response shaping.

## Parameters

### Global Parameters


### Global

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Max Filters | Int | 1 .. 20 | 20 | - | Maximum number of EQ bands |

### Algorithm

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| TDF-II | Bool | On / Off | Off | - | Use Transposed Direct Form II |
| Topology | Choice (Biquad, SVF) | 2 options | Biquad | - | Filter topology: Biquad (classic) or SVF (zero-delay feedback, modulation-stable) |

### Per-Band Parameters

These parameters are repeated for each filter band.


### Filter

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Frequency | Float | 20 .. 20000 | 1000 | Hz | Filter center/corner frequency |
| Q | Float | 0.1 .. 10 | 1 | - | Filter bandwidth (quality factor) |
| Gain | Float | -24 .. 24 | 0 | dB | Boost or cut amount |
| Type | Choice (Peak, Lowshelf, Highshelf, Lowpass, Highpass, Bandpass, Notch, AllPass) | 8 options | Peak | - | Biquad filter shape |

:::note
**Structural parameters** (Max Filters, Topology) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
