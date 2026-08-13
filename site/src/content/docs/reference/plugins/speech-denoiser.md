---
title: "Speech Denoiser"
description: "Neural speech-focused denoiser (RNNoise-derived) optimized for voice and dialogue cleanup."
---

Neural speech-focused denoiser (RNNoise-derived) optimized for voice and dialogue cleanup.

At 48 kHz, mono uses RNNoise's native synthesis. Stereo uses one
polarity-aware, energy-normalized detector and applies the same 22 bounded,
smoothed model gains to both original channels. The plugin accepts arbitrary
callback partitions with 480 samples of constant latency. Its typed monitoring
snapshot reports those model gains and bounded VAD probability without
allocating in the audio callback; these diagnostics are not an external-corpus
quality claim.

## Parameters


### General

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Enabled | Bool | On / Off | On | - | Enable RNNoise speech denoising |
