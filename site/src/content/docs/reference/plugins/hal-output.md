---
title: "HAL Output"
description: "SOTF HAL Output plugin for macOS audio HAL output. Writes processed audio data back to the macOS CoreAudio HAL driver via shared memory, completing the system-wide audio processing chain."
---

SOTF HAL Output plugin for macOS audio HAL output. Writes processed audio data back to the macOS CoreAudio HAL driver via shared memory, completing the system-wide audio processing chain.

## Parameters

### Diagnostics

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Underrun Count | Int | 0 .. 2147483647 | 0 | - | Number of buffer underruns detected (read-only diagnostic) |
| Write Success | Float | 0 .. 100 | 100 | % | Percentage of samples accepted by the HAL writer in the last process block (100 = all accepted, <100 = back-pressure / partial write; read-only diagnostic) |
| Connected | Bool | On / Off | Off | - | HAL output writer currently connected to shared memory |
| Backpressure | Bool | On / Off | Off | - | HAL output reported a partial write in the last process block |

:::note
All HAL Output parameters are **read-only diagnostics**. They cannot be adjusted at runtime and do not affect the audio signal.
:::
