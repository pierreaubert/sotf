---
title: "HAL Input"
description: "Reads audio from a system HAL device into the engine. Used by the macOS system-wide daemon."
---

Reads audio from a system HAL device into the engine. Used by the macOS system-wide daemon.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Input Channels | Int | 1 .. 16 | 2 | ch | Number of HAL input channels |

:::note
**Structural parameters** (Input Channels) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
