---
title: "Crossover"
description: "N-way crossover with selectable filter family (LR24 or linear-phase FIR) and per-channel configuration."
---

N-way crossover with selectable filter family (LR24 or linear-phase FIR) and per-channel configuration.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Type | Choice (LR24, LinearPhase) | 2 options | LR24 | - | Crossover filter family: LR24 (Linkwitz-Riley 24 dB/octave) or linear-phase FIR |
| Frequency | Float | 20 .. 20000 | 1000 | Hz | Primary crossover frequency |
| Mode | Choice (Lowpass, Highpass, Both) | 3 options | Lowpass | - | Output mode for the primary crossover |
| FIR Taps | Int | 31 .. 16385 | 1025 | - | FIR length for linear-phase mode (odd values are rounded up) |

:::note
**Structural parameters** (Type, FIR Taps) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
