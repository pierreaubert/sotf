---
title: "Loudness Compensation"
description: "Equal-loudness contour compensation (Fletcher-Munson). Adjusts frequency response based on playback volume to maintain perceived tonal balance."
---

Equal-loudness contour compensation (Fletcher-Munson). Adjusts frequency response based on playback volume to maintain perceived tonal balance.

## Parameters


### Low

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Low Freq | Float | 20 .. 500 | 100 | Hz | Low shelf center frequency |
| Low Gain | Float | -20 .. 20 | 6 | dB | Low shelf boost/cut at low volume |

### High

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| High Freq | Float | 2000 .. 20000 | 8000 | Hz | High shelf center frequency |
| High Gain | Float | -20 .. 20 | 6 | dB | High shelf boost/cut at low volume |

### Mid

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mid Enabled | Bool | On / Off | On | - | Enable mid-range compensation band |
| Mid Freq | Float | 500 .. 8000 | 3500 | Hz | Mid peak center frequency |
| Mid Gain | Float | -20 .. 20 | 3 | dB | Mid peak boost/cut at low volume |
| Mid Q | Float | 0.1 .. 5 | 0.707 | - | Mid peak bandwidth (Q factor) |

### Auto Gain

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Auto Gain | Bool | On / Off | Off | - | Auto-normalize output level |
| Max Auto Gain | Float | 0 .. 24 | 12 | dB | Maximum auto gain correction |
| Smoothing | Float | 1 .. 1000 | 100 | ms | Auto gain transition time |

### Compensation

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mode | Choice (Manual, ISO 226, Auto) | 3 options | Manual | - | Manual 3-band or ISO 226 automatic contour |
| Playback Level | Float | 40 .. 90 | 70 | dB SPL | Current playback level — compensation adjusts for this level vs reference |
| Reference Level | Float | 60 .. 100 | 83 | dB SPL | Reference listening level (no compensation applied at this level) |

### Auto

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Playback Volume | Float | -80 .. 0 | 0 | dB | Engine playback volume (set automatically by the engine) |

:::note
**Structural parameters** (Mid Enabled, Auto Gain, Max Auto Gain, Smoothing, Mode) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
