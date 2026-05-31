---
title: "De-Esser"
description: "Sibilance reduction targeting harsh high-frequency content (s, t, sh sounds)."
---

Sibilance reduction targeting harsh high-frequency content (s, t, sh sounds).

## Parameters


### Detection

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Frequency | Float | 2000 .. 16000 | 7000 | Hz | Center frequency for sibilance detection |
| Q | Float | 0.5 .. 5 | 1.5 | - | Bandwidth of detection filter |

### Dynamics

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Threshold | Float | -60 .. 0 | -20 | dB | Sibilance detection threshold |
| Ratio | Float | 1 .. 20 | 4 | :1 | Compression ratio for sibilance |
| Attack | Float | 0.1 .. 10 | 0.5 | ms | Attack time |
| Release | Float | 5 .. 200 | 20 | ms | Release time |

### Mode

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mode | Choice (Wideband, Split-Band) | 2 options | Split-Band | - | Wideband reduces full signal; Split-band only reduces HF |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet mix |
