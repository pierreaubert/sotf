---
title: "Hiss Reducer"
description: "Reduces high-frequency hiss and tape noise while preserving program content."
---

Reduces low-level, stationary high-frequency noise while preserving program content. The
threshold is a calibrated high-band level threshold, not a direct SNR measurement.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Enabled | Bool | On / Off | On | - | Enable high-frequency hiss reduction |
| Threshold | Float | -60 .. -10 | -30 | dB | SNR threshold for stationary hiss detection |
| Frequency | Float | 1000 .. 16000 | 4000 | Hz | Frequency above which hiss reduction applies |
| Strength | Float | 0 .. 1 | 0.5 | - | Hiss attenuation strength |
