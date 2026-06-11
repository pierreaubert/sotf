---
title: "FIR Designer"
description: "Linear-phase and minimum-phase FIR equalizer designed from parametric EQ bands."
---

Linear-phase and minimum-phase FIR equalizer designed from parametric EQ bands.

## Parameters

### Per-Band Parameters

These parameters are repeated for each filter band.


### Band

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Type | Choice (Peak, Lowshelf, Highshelf, Lowpass, Highpass) | 5 options | Peak | - | Filter type |
| Frequency | Float | 20 .. 20000 | 1000 | Hz | Center frequency |
| Q | Float | 0.1 .. 10 | 1 | - | Bandwidth |
| Gain | Float | -24 .. 24 | 0 | dB | Boost/cut |
| Active | Bool | On / Off | On | - | Enable this band |

### Single-Band Parameters


### EQ

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Num Filters | Int | 1 .. 10 | 5 | - | Number of EQ bands |

### Quality

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| FIR Length | Choice (1024, 2048, 4096, 8192) | 4 options | 2048 | - | FIR length in taps (higher = better bass resolution, more latency) |

### Phase

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Phase Mode | Choice (Linear, Minimum) | 2 options | Linear | - | FIR phase design mode |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Auto Gain | Bool | On / Off | Off | - | Compensate output level |
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet mix |

:::note
**Structural parameters** (Num Filters) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
