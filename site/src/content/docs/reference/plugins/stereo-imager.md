---
title: "Stereo Imager"
description: "Controls stereo width from mono to extra-wide, using mid/side processing."
---

Controls stereo width from mono to extra-wide, using mid/side processing.

## Parameters


### Width

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Width | Float | 0 .. 2 | 1 | - | Global stereo width (0%=mono, 100%=original, 200%=wide) |

### Crossover

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Low-Mid | Float | 80 .. 1000 | 250 | Hz | Low/mid crossover frequency |
| Mid-High | Float | 1000 .. 16000 | 4000 | Hz | Mid/high crossover frequency |

### Band Width

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Low Width | Float | 0 .. 2 | 1 | - | Low band stereo width |
| Mid Width | Float | 0 .. 2 | 1 | - | Mid band stereo width |
| High Width | Float | 0 .. 2 | 1 | - | High band stereo width |

### Options

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mono Bass | Bool | On / Off | Off | - | Collapse stereo below low-mid crossover |

### Output

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mix | Float | 0 .. 1 | 1 | - | Dry/wet mix |
