---
title: "Resampler"
description: "High-quality sample rate conversion using rubato with sinc interpolation for transparent resampling between any sample rates."
---

High-quality sample rate conversion using rubato with sinc interpolation for transparent resampling between any sample rates.

## Parameters

### Global Parameters


### Quality

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Quality | Choice (Fast, Medium, High) | 3 options | Medium | - | Resampling quality: fast (64-tap), medium (128-tap), high (256-tap) |

### Ratio

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Dynamic Ratio | Bool | On / Off | Off | - | Enable runtime ratio changes without rebuilding |
| Ratio | Float | nominal / 2 .. nominal * 2 | Nominal | - | Current resampling ratio (only adjustable when Dynamic Ratio is enabled) |

:::note
**Structural parameters** (Quality) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
