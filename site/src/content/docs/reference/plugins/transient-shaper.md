---
title: "Transient Shaper"
description: "Shapes attack and sustain characteristics of audio transients."
---

Shapes attack and sustain characteristics of audio transients.

## Parameters


### Shape

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Attack | Float | -100 .. 100 | 0 | % | Transient emphasis (-100% to +100%) |
| Sustain | Float | -100 .. 100 | 0 | % | Sustain emphasis (-100% to +100%) |

### Detection

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Sensitivity | Float | -12 .. 12 | 0 | dB | Detection sensitivity offset |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Output | Float | -12 .. 12 | 0 | dB | Output gain compensation |
| Mix | Float | 0 .. 1 | 1 | % | Dry/wet mix |

