---
title: "Downmix"
description: "Surround to stereo downmixing with configurable channel contributions."
---

Surround to stereo downmixing with configurable channel contributions.

## Parameters


### Gains

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Center Gain | Float | -12 .. 0 | -3 | dB | Center channel fold-down level |
| Surround Gain | Float | -12 .. 0 | -3 | dB | Surround channels fold-down level |
| Height Gain | Float | -60 .. 0 | -6 | dB | Height channels fold-down level |
| LFE Gain | Float | -60 .. 0 | -10 | dB | LFE channel fold-down level |

### Phase

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Phase Coherence | Bool | On / Off | On | - | Phase-align channels before mix |
| Phase Blend Low | Float | 100 .. 1000 | 500 | Hz | Phase correction low crossover |
| Phase Blend High | Float | 1000 .. 5000 | 2000 | Hz | Phase correction high crossover |

### Mode

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| ITU-R BS.775 Mode | Bool | On / Off | Off | - | Use ITU standard downmix coeffs |

