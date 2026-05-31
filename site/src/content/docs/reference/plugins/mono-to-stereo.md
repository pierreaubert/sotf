---
title: "Mono to Stereo"
description: "Converts mono signals to stereo output."
---

Converts mono signals to stereo output.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Width | Float | 0 .. 1 | 0.5 | - | Stereo spread amount |
| Haas Delay | Float | 0 .. 5 | 1.5 | ms | Inter-channel delay for Haas effect |
| Decor Low | Float | 100 .. 500 | 300 | Hz | Decorrelation low crossover |
| Decor High | Float | 1000 .. 5000 | 2000 | Hz | Decorrelation high crossover |
| Freq Dependent | Bool | On / Off | On | - | Vary width by frequency band |
