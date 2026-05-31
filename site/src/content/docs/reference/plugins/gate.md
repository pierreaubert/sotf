---
title: "Gate"
description: "Noise gate that silences audio below a configurable threshold."
---

Noise gate that silences audio below a configurable threshold.

## Parameters


### Dynamics

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Threshold | Float | -80 .. 0 | -40 | dB | Level below which gate closes |
| Ratio | Float | 1 .. 100 | 10 | :1 | Attenuation depth when closed |
| Range | Float | 0 .. 120 | 80 | dB | Max attenuation when gate closed |
| Hysteresis | Float | 0 .. 12 | 4 | dB | Open/close threshold difference |
| Knee | Float | 0 .. 20 | 0 | dB | Softness of threshold transition |

### Timing

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Attack | Float | 0.1 .. 50 | 1 | ms | Time for gate to open |
| Hold | Float | 0 .. 1000 | 10 | ms | Minimum open time after trigger |
| Release | Float | 10 .. 2000 | 100 | ms | Time for gate to close |
| Lookahead | Float | 0 .. 20 | 0 | ms | Pre-delay for transient catching |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet blend |

### Channels

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Link Channels | Bool | On / Off | Linked | - | Stereo-link detector for L/R |

### Sidechain

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Sidechain HPF | Float | 0 .. 200 | 0 | Hz | High-pass on detector input |
| HPF Order | Choice (2nd, 4th) | 2 options | 2nd | - | Sidechain HPF filter order |
| Detection | Choice (Peak, RMS) | 2 options | Peak | - | Level detection mode |
| Ext Sidechain | Bool | On / Off | Off | - | Use external sidechain input |
