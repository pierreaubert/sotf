---
title: "Multiband Expander"
description: "Per-band dynamic range expansion with 2-5 frequency bands and independent expander settings per band."
---

Per-band dynamic range expansion with 2-5 frequency bands and independent expander settings per band.

## Parameters

### Global Parameters


### Global

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Bands | Int | 2 .. 5 | 3 | - | Number of frequency bands |
| Preset | Int | 0 .. 3 | 1 | - | Crossover frequency preset |
| Crossover 1 | Float | 20 .. 500 | 200 | Hz | Low/mid split frequency |
| Crossover 2 | Float | 500 .. 5000 | 2000 | Hz | Mid/high split frequency |
| Crossover 3 | Float | 5000 .. 15000 | 8000 | Hz | High/air split frequency |
| Crossover 4 | Float | 10000 .. 18000 | 12000 | Hz | Band 4/5 split frequency |
| Threshold | Float | -80 .. 0 | -40 | dB | Global expansion threshold |
| Ratio | Float | 1 .. 20 | 2 | :1 | Global expansion ratio |
| Attack | Float | 0.1 .. 50 | 1 | ms | Global attack time |
| Release | Float | 10 .. 2000 | 100 | ms | Global release time |
| Range | Float | 0 .. 80 | 40 | dB | Max attenuation below threshold |
| Knee | Float | 0 .. 20 | 6 | dB | Global knee softness |
| Hysteresis | Float | 0 .. 12 | 4 | dB | Open/close threshold difference |
| Hold | Float | 0 .. 500 | 10 | ms | Minimum open time after trigger |
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet blend |
| Link Channels | Bool | On / Off | Linked | - | Stereo-link detector for L/R |
| Detection Mode | Choice (Peak, RMS) | 2 options | Peak | - | Peak or RMS level detection |

### Timing

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Lookahead | Float | 0 .. 20 | 0 | ms | Pre-delay for transient catching |

### Single-Band Parameters


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

:::note
**Structural parameters** (Bands, Preset, Crossover 1, Crossover 2, Crossover 3, Crossover 4) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
