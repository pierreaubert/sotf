---
title: "A/B Compare"
description: "Side-by-side A/B comparison. Instantly toggle between processed and bypass to evaluate your plugin chain."
---

Side-by-side A/B comparison. Instantly toggle between processed and bypass to evaluate your plugin chain.

## Parameters


### Mix

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mix (A/B) | Float | -1 .. 1 | 0 | - | Crossfade between path A and B |
| Mix Mode | Choice (Pot, Binary) | 2 options | Pot | - | Smooth pot or instant switch |
| Selected Path | Choice (A, B) | 2 options | A | - | Active path in binary mode |
| Bypass | Bool | On / Off | No | - | Bypass both paths (dry signal) |
| Mix Transition | Float | 1 .. 500 | 50 | ms | A/B crossfade duration |
| Difference Mode | Bool | On / Off | Off | - | Output A minus B difference |

### Auto Gain

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Auto Gain | Bool | On / Off | On | - | Loudness-match A and B paths |
| Loudness Type | Choice (Momentary, ShortTerm) | 2 options | Momentary | - | Loudness measurement window |
| Max Auto Gain | Float | 0 .. 24 | 12 | dB | Maximum auto gain correction |
| Gain Smoothing | Float | 1 .. 500 | 100 | ms | Auto gain transition time |

### Configuration

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Path A Config | File Path | - | - | - | Plugin chain config for path A |
| Path B Config | File Path | - | - | - | Plugin chain config for path B |

### Phase

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Phase Invert A | Bool | On / Off | Off | - | Invert polarity of path A |
| Phase Invert B | Bool | On / Off | Off | - | Invert polarity of path B |

:::note
**Structural parameters** (Path A Config, Path B Config) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
