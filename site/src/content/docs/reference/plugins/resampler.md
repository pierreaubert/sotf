---
title: "Resampler"
description: "High-quality sample-rate converter using sinc interpolation. Supports fixed and dynamic resampling ratios."
---

High-quality sample-rate converter using sinc interpolation. Supports fixed and dynamic resampling ratios.

## Parameters


### Quality

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Quality | Choice (Fast, Medium, High) | 3 options | Medium | - | Resampling quality: fast (64-tap), medium (128-tap), high (256-tap) sinc filter |

### Ratio

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Dynamic Ratio | Bool | On / Off | Off | - | Enable runtime ratio changes without rebuilding the resampler |
| Ratio | Float | 0.25 .. 4 | 1 | - | Current resampling ratio (only adjustable when Dynamic Ratio is enabled) |

:::note
**Structural parameters** (Quality) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
