---
title: "Perceptual Noise Diffusion"
description: "Perceptual noise diffusion (PND) for improving perceived audio quality through controlled noise shaping."
---

Perceptual noise diffusion (PND) for improving perceived audio quality through controlled noise shaping.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Correction | Float | 0 .. 2 | 1 | - | Pitch correction strength |
| Analysis Window | Float | 20 .. 500 | 100 | ms | FFT analysis window size |
| Drift Smoothing | Float | 0.001 .. 1 | 0.1 | - | Pitch drift low-pass smoothing |

### Analysis

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Multi-Channel | Bool | On / Off | On | - | Analyze all channels together |

### Correction

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Confidence Threshold | Float | 0 .. 1 | 0.5 | - | Min detection confidence to apply |
| Phase Vocoder | Bool | On / Off | Off | - | Use phase vocoder for correction |

