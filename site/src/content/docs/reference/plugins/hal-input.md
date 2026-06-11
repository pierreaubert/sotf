---
title: "HAL Input"
description: "SOTF HAL Input plugin for macOS audio HAL input. Reads audio data from the macOS CoreAudio HAL driver via shared memory, acting as the input source for system-wide audio processing."
---

Reads audio data from the macOS CoreAudio HAL driver via shared memory, acting as the input source for system-wide audio processing.

## Parameters

### Global Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Input Channels | Int | 1 .. 16 | 2 | ch | Number of HAL input channels |

:::note
**Structural parameters** (Input Channels) require rebuilding the plugin when changed.
:::
