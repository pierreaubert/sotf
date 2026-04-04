---
title: "Multiband Compressor"
description: "Per-band dynamic range compression with 2-5 frequency bands and independent compressor settings per band."
---

Per-band dynamic range compression with 2-5 frequency bands and independent compressor settings per band.

## Parameters

### Global Parameters

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Bands | Int | 2 .. 5 | 3 | - | Number of frequency bands |
| Preset | Int | 0 .. 3 | 1 | - | Crossover frequency preset |
| Crossover 1 | Float | 20 .. 500 | 200 | Hz | Low/mid split frequency |
| Crossover 2 | Float | 500 .. 5000 | 2000 | Hz | Mid/high split frequency |
| Crossover 3 | Float | 5000 .. 15000 | 8000 | Hz | High/air split frequency |
| Crossover 4 | Float | 10000 .. 18000 | 12000 | Hz | Band 4/5 split frequency |
| Threshold | Float | -60 .. 0 | -20 | dB | Global compression threshold |
| Ratio | Float | 1 .. 20 | 4 | :1 | Global compression ratio |
| Attack | Float | 0.1 .. 100 | 5 | ms | Global attack time |
| Release | Float | 10 .. 1000 | 50 | ms | Global release time |
| Knee | Float | 0 .. 20 | 6 | dB | Global knee softness |
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet blend |
| Link Channels | Bool | On / Off | Linked | - | Stereo-link detector for L/R |
| Lookahead | Float | 0 .. 20 | 0 | ms | Per-band pre-delay |
| M/S Mode | Bool | On / Off | Off | - | Mid/Side processing mode |
| Sidechain Tilt | Float | -6 .. 6 | 0 | dB | Detection tilt: +dB = more HF sensitive, -dB = more LF sensitive |
| Link Amount | Float | 0 .. 1 | 1 | % | Channel linking: 0%=independent, 100%=linked |

### Single-Band Parameters


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

:::note
**Structural parameters** (Bands, Preset, Crossover 1, Crossover 2, Crossover 3, Crossover 4) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
