---
title: "Dither"
description: "Adds dither noise for bit-depth reduction, minimizing quantization distortion."
---

Adds dither noise for bit-depth reduction, minimizing quantization distortion.

## Parameters

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Bit Depth | Choice (16, 20, 24) | 3 options | 16 | - | Target bit depth for quantization |
| Noise Shaping | Bool | On / Off | On | - | F-weighted noise shaping (Wannamaker 1992) |
| Dither Type | Choice (TPDF, None) | 2 options | TPDF | - | Dither algorithm: TPDF or no dither |

