---
title: "Saturation"
description: "Harmonic saturation and soft clipping for adding warmth and character."
---

Harmonic saturation and soft clipping for adding warmth and character.

## Parameters


### Saturation

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mode | Choice (Soft Clip, Tube, Tape, Exciter) | 4 options | Soft Clip | - | Saturation algorithm |
| Drive | Float | 1 .. 20 | 2 | - | Saturation intensity |
| Tone | Float | 1 .. 3 | 1.5 | - | Harmonic character (tube mode: even/odd balance) |

### Exciter

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Exciter Freq | Float | 500 .. 10000 | 3000 | Hz | Crossover frequency for exciter mode |

### Quality

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Oversampling | Choice (Off, 2x, 4x) | 3 options | 2x | - | Oversampling factor for alias suppression |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Output | Float | -12 .. 12 | 0 | dB | Output gain compensation |
| Mix | Float | 0 .. 1 | 0.5 | % | Dry/wet blend |

