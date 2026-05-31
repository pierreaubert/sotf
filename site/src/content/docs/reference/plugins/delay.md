---
title: "Delay"
description: "Audio delay with configurable delay time per channel."
---

Audio delay with configurable delay time per channel.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Delay | Float | 0 .. 5000 | 100 | ms | Delay time |
| Feedback | Float | -0.95 .. 0.95 | 0.3 | - | Amount fed back into delay line |
| Mix | Float | 0 .. 1 | 0.5 | % | Dry/wet blend |
| Allpass Coeff | Float | 0 .. 0.99 | 0.5 | - | Allpass filter coefficient |
| Allpass Feedback | Bool | On / Off | Off | - | Use allpass filter in feedback path |

### Modulation

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| LFO Rate | Float | 0 .. 20 | 0 | Hz | Modulation oscillator speed |
| LFO Depth | Float | 0 .. 10 | 0 | ms | Modulation amount on delay time |
