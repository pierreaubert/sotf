---
title: "Compressor"
description: "Dynamic range compression with configurable threshold, ratio, attack, release, and makeup gain."
---

Dynamic range compression with configurable threshold, ratio, attack, release, and makeup gain.

## Parameters


### Dynamics

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Threshold | Float | -60 .. 0 | -20 | dB | Level above which compression starts |
| Ratio | Float | 1 .. 20 | 4 | :1 | Compression amount (input:output) |
| Knee | Float | 0 .. 20 | 6 | dB | Softness of threshold transition |

### Timing

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Attack | Float | 0.1 .. 100 | 5 | ms | Time to reach full compression |
| Release | Float | 10 .. 1000 | 50 | ms | Time to return to unity gain |
| Lookahead | Float | 0 .. 20 | 0 | ms | Pre-delay for transient catching |
| Program Dependent Release | Bool | On / Off | Off | - | Adapts release to signal content |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Makeup Gain | Float | -24 .. 24 | 0 | dB | Post-compression gain boost |
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet blend (parallel comp) |
| Auto Makeup | Bool | On / Off | Off | - | Auto-compensate for gain reduction |
| Measured Auto Makeup | Bool | On / Off | Off | - | Makeup based on measured reduction |

### Channels

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Link Channels | Bool | On / Off | Linked | - | Stereo-link detector for L/R |

### Sidechain

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Sidechain HPF | Float | 0 .. 200 | 80 | Hz | High-pass on detector input |
| Sidechain HPF Order | Choice (2nd, 4th) | 2 options | 2nd | - | Butterworth HPF slope |
| Detection Mode | Choice (Peak, RMS) | 2 options | Peak | - | Peak or RMS level detection |
| External Sidechain | Bool | On / Off | Off | - | Use external signal for detection |
