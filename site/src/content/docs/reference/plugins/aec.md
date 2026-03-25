---
title: "Acoustic Echo Cancellation"
description: "Cancels acoustic echoes from microphone input using reference signal correlation."
---

Cancels acoustic echoes from microphone input using reference signal correlation.

## Parameters

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Echo Tail | Float | 50 .. 500 | 200 | ms | Max echo path length to cancel |
| Step Size | Float | 0.1 .. 0.9 | 0.5 | - | Adaptive filter convergence rate |
| Post-Filter | Bool | On / Off | On | - | Apply residual echo suppression |

