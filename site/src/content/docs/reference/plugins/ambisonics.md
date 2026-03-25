---
title: "Ambisonics"
description: "Ambisonics encoding and decoding for immersive spatial audio."
---

Ambisonics encoding and decoding for immersive spatial audio.

## Parameters

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Order | Int | 1 .. 3 | 1 | - | Ambisonics order (1-3) |
| Target Layout | Choice (5.1, 7.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, 9.1.6) | 8 options | 5.1 | - | Target speaker layout for decode |
| Max-rE | Bool | On / Off | On | - | Apply max-rE energy optimization |
| Dual-Band | Bool | On / Off | Off | - | Separate LF/HF decode weights |

:::note
**Structural parameters** (Order, Target Layout) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
