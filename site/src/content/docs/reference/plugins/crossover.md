---
title: "Crossover"
description: "Splits audio into frequency bands using Linkwitz-Riley or linear-phase FIR filters for multiband processing or multi-way speaker management."
---

Splits audio into frequency bands using Linkwitz-Riley or linear-phase FIR filters for multiband processing or multi-way speaker management.

## Parameters

### Global Parameters


### Global

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Type | Choice (LR24, LinearPhase) | 2 options | LR24 | - | Crossover filter family: LR24 (Linkwitz-Riley 24 dB/oct) or LinearPhase (FIR) |
| Frequency | Float | 20 .. 20000 | 1000 | Hz | Primary crossover frequency |
| Mode | Choice (Lowpass, Highpass, Both) | 3 options | Lowpass | - | Output mode: low band only, high band only, or both bands |
| FIR Taps | Int | 31 .. 16385 | 1025 | - | FIR filter length in taps (LinearPhase only) |

### Multi-way Parameters

These parameters appear when the crossover is configured with additional frequencies (3-way, 4-way, etc.).

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Frequency 2 | Float | 20 .. 20000 | - | Hz | Second crossover frequency |
| Frequency 3 | Float | 20 .. 20000 | - | Hz | Third crossover frequency |

### Per-Channel Parameters

These parameters appear when the crossover is configured in per-channel mode. Each channel gets its own independent LR24 crossover.

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Frequency ChN | Float | 20 .. 20000 | - | Hz | Crossover frequency for channel N |
| Mode ChN | Choice (Lowpass, Highpass, Mute, Passthrough) | 4 options | - | - | Operation mode for channel N |

:::note
**Structural parameters** (Type, extra crossover frequencies, per-channel configuration, FIR Taps) require rebuilding the plugin when changed. Frequency and Mode update in real-time with zero dropout.
:::
