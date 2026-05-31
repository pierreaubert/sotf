---
title: "Expander"
description: "Dynamic range expansion with configurable threshold, ratio, attack, release, and range. Opens up dynamics below the threshold."
---

Dynamic range expansion with configurable threshold, ratio, attack, release, and range. Opens up dynamics below the threshold.

## Parameters


### Dynamics

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Threshold | Float | -80 .. 0 | -40 | dB | Level below which expansion starts |
| Ratio | Float | 1 .. 20 | 2 | :1 | Expansion amount (input:output) |
| Range | Float | 0 .. 80 | 40 | dB | Max attenuation below threshold |
| Knee | Float | 0 .. 20 | 6 | dB | Softness of threshold transition |
| Hysteresis | Float | 0 .. 12 | 4 | dB | Open/close threshold difference |

### Timing

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Attack | Float | 0.1 .. 50 | 1 | ms | Time to reach full expansion |
| Release | Float | 10 .. 2000 | 100 | ms | Time to return to unity gain |
| Hold | Float | 0 .. 500 | 10 | ms | Minimum open time after trigger |
| Lookahead | Float | 0 .. 20 | 0 | ms | Pre-delay for transient catching |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet blend |
| Auto Makeup | Bool | On / Off | Off | - | Auto-compensate for gain reduction |
| Measured Auto Makeup | Bool | On / Off | Off | - | Makeup based on measured reduction |

### Channels

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Link Channels | Bool | On / Off | Linked | - | Stereo-link detector for L/R |

### Sidechain

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Sidechain HPF | Float | 0 .. 500 | 80 | Hz | High-pass on detector input |
| Detection Mode | Choice (Peak, RMS) | 2 options | Peak | - | Peak or RMS level detection |
