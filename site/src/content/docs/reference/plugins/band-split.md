---
title: "Band Split"
description: "Splits the audio signal into separate frequency bands for independent processing."
---

Splits the audio signal into separate frequency bands for independent processing.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Frequency | Float | 20 .. 20000 | 300 | Hz | Crossover split frequency |
| Type | Choice (LR24, LR48) | 2 options | LR24 | - | Filter slope (24 or 48 dB/oct) |

:::note
**Structural parameters** (Frequency, Type) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
