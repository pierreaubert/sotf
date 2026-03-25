---
title: "Beamformer"
description: "Microphone array beamforming for directional audio capture."
---

Microphone array beamforming for directional audio capture.

## Parameters


### Array

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Microphones | Int | 2 .. 8 | 2 | - | Number of array microphones |
| Mic Spacing | Float | 1 .. 50 | 5 | cm | Distance between microphones |

### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Steer Angle | Float | -180 .. 180 | 0 | ° | Beam steering direction |
| Algorithm | Choice (MVDR, Superdirective, GSC) | 3 options | MVDR | - | Beamforming algorithm |

:::note
**Structural parameters** (Microphones, Mic Spacing, Algorithm) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
