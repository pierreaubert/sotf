---
title: "Dynamic EQ"
description: "Frequency-dependent dynamic equalizer that adjusts filter gain based on signal level."
---

Frequency-dependent dynamic equalizer that adjusts filter gain based on signal level.

## Parameters


### Setup

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Num Bands | Int | 1 .. 8 | 4 | Bands | Number of dynamic EQ bands |

### Dynamics

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Threshold | Float | -60 .. 0 | -20 | dB | Global detection threshold |
| Ratio | Float | 1 .. 20 | 2 | :1 | Global dynamics ratio |
| Knee | Float | 0 .. 20 | 6 | dB | Global soft knee |

### Timing

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Attack | Float | 0.1 .. 100 | 5 | ms | Global attack time |
| Release | Float | 10 .. 1000 | 50 | ms | Global release time |

### Channels

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Link Channels | Bool | On / Off | Linked | - | Stereo-link detection |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet mix |

