---
title: "Crosstalk Cancellation (XTC)"
description: "Crosstalk cancellation for speaker playback. Creates a wider stereo image by cancelling inter-speaker interference."
---

Crosstalk cancellation for speaker playback. Creates a wider stereo image by cancelling inter-speaker interference.

## Parameters


### Geometry

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Distance | Float | 0.5 .. 10 | 2 | m | Listener-to-speaker distance |
| Speaker Angle | Float | 10 .. 90 | 30 | ° | Half-angle between speakers |
| Head Radius | Float | 0.05 .. 0.12 | 0.0875 | m | Acoustic head radius |
| Head Model | Choice (Woodworth, Brown-Duda) | 2 options | Woodworth | - | Head diffraction model: Woodworth (classic) or Brown-Duda (rigid sphere, more accurate above 1.5kHz) |

### Head Tracking

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Head Offset X | Float | -0.5 .. 0.5 | 0 | m | Lateral head position offset |
| Head Offset Z | Float | -0.5 .. 0.5 | 0 | m | Forward/back head position |
| Head Yaw | Float | -90 .. 90 | 0 | ° | Head rotation angle |
| Head Tracking Smooth | Float | 0 .. 1 | 0.1 | s | Tracking data smoothing time |

### Beta

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Beta Base | Float | 0.0001 .. 0.1 | 0.001 | - | Regularization base level |
| Beta Low Boost | Float | 0 .. 30 | 10 | - | Extra regularization at low freq |
| Beta High Boost | Float | 0 .. 30 | 10 | - | Extra regularization at high freq |

### Shadow

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Shadow Cutoff | Float | 1000 .. 10000 | 4000 | Hz | Head shadow filter onset freq |
| Shadow Slope | Float | 0 .. 12 | 6 | dB/oct | Head shadow attenuation rate |

### Filter

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Max Gain | Float | 3 .. 30 | 12 | dB | Maximum XTC filter boost |

### Advanced

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Spectral Norm | Bool | On / Off | On | - | Normalize filter energy |
| Pinna Model | Bool | On / Off | Off | - | Include pinna diffraction model |

### Room

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Room Reflections | Bool | On / Off | Off | - | Include first-order reflections |
| Room IR | File Path | - | - | - | Optional measured room impulse response |
| Room Width | Float | 2 .. 10 | 4 | m | Listening room width |
| Room Depth | Float | 2 .. 15 | 5 | m | Listening room depth |
| Wall Absorption | Float | 0 .. 1 | 0.3 | - | Wall absorption coefficient |
| Reflection Beta | Float | 1 .. 10 | 3 | - | Reflection path regularization |

### Diagnostic

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Bypass XTC Filters | Bool | On / Off | Off | - | Skip crosstalk cancellation |
| Bypass Spectral Norm | Bool | On / Off | Off | - | Skip spectral normalization |
| Bypass Neumann | Bool | On / Off | Off | - | Skip Neumann KH refinement |

### Auto Gain

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Auto Gain | Bool | On / Off | On | - | Auto-normalize output level |
| AG Max | Float | 0 .. 24 | 12 | dB | Maximum auto gain correction |
| AG Smoothing | Float | 10 .. 500 | 100 | ms | Auto gain transition time |

:::note
**Structural parameters** (Room IR) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
