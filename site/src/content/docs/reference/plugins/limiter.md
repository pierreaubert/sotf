---
title: "Limiter"
description: "Peak limiter to prevent clipping. Ensures output never exceeds the ceiling level."
---

Peak limiter to prevent clipping. Ensures output never exceeds the ceiling level.

## Parameters


### Dynamics

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Threshold | Float | -20 .. 0 | -0.1 | dB | Ceiling level (max output) |
| Soft Knee | Bool | On / Off | Hard | - | Gradual vs hard limiting onset |

### Timing

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Release | Float | 10 .. 1000 | 50 | ms | Time to return to unity gain |
| Lookahead | Float | 0 .. 20 | 5 | ms | Pre-delay for peak catching |
| Dual Release | Bool | On / Off | Off | - | Fast+slow release envelopes |

### Detection

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| True Peak | Bool | On / Off | Off | - | Detect inter-sample peaks |
| ISP Limit | Bool | On / Off | Off | - | Guarantee output has no inter-sample peaks above ceiling |
| Link | Float | 0 .. 1 | 1 | % | Channel linking: 0%=independent, 100%=linked (all channels see max peak) |
| Feed Forward | Bool | On / Off | Off | - | Scan lookahead buffer for anticipatory gain reduction |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet blend |
