# Matrix

## Overview

A flexible channel routing and mixing matrix that maps N input channels to P output channels with configurable per-connection gains. Supports mute, solo, and dim per output channel with smoothed gain transitions. Used for surround routing, channel remapping, M/S encoding, and custom mixes.

## Features

### Gain Matrix

An N×P matrix where each cell represents the gain from one input channel to one output channel. Values are stored as linear gains but displayed in dB.

**Per-Cell Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Gain | -144 to +24 | varies | dB | Gain from input to output (-∞ displayed as mute). Identity matrix: diagonal = 0 dB, off-diagonal = -∞. UI interaction clamps to -60 to +6 dB |

### Channel States

Per-output-channel controls for monitoring and isolation:

| State | Effect | Description |
|-------|--------|-------------|
| Mute | Gain → 0.0 | Silences the output channel |
| Solo | Others → 0.0 | Isolates this channel (others muted) |
| Dim | Gain → -20 dB | Reduces channel to background level |

### Presets

Built-in matrix presets for common routing configurations:

| Preset | Description |
|--------|-------------|
| Identity | 1:1 pass-through (diagonal = 0 dB) |
| Swap L/R | Swaps left and right channels |
| Mono Mix | Sums all inputs to all outputs equally |

## Demos

### Demo: Stereo to Mono Downmix

**Scenario:** Converting stereo to mono for speaker checking.
**Before:** Stereo signal with full L/R separation.
**After:** Mono signal with equal contribution from both channels.
**Config:**
```json
{
  "input_channels": 2,
  "output_channels": 2,
  "matrix": [0.707, 0.707, 0.707, 0.707]
}
```

### Demo: L/R Swap

**Scenario:** Speakers are wired backwards — need to swap channels.
**Before:** Left channel comes from right speaker and vice versa.
**After:** Correct stereo image with proper L/R alignment.
**Config:**
```json
{
  "input_channels": 2,
  "output_channels": 2,
  "matrix": [0.0, 1.0, 1.0, 0.0]
}
```

### Demo: Center Channel Extraction

**Scenario:** Extracting a phantom center channel from stereo by summing L+R.
**Before:** 2-channel stereo.
**After:** 3-channel output with L, R, and extracted center.
**Config:**
```json
{
  "input_channels": 2,
  "output_channels": 3,
  "matrix": [1.0, 0.0, 0.0, 1.0, 0.707, 0.707]
}
```

## Presets

### Identity
**Use case:** Pass-through with no routing changes
```json
{
  "matrix": "identity"
}
```
**Tips:** Starting point for custom routing. Modify individual cells from here.

### Swap L/R
**Use case:** Correct reversed speaker wiring
```json
{
  "matrix": "swap_lr"
}
```

### Mono Mix
**Use case:** Sum all channels to mono
```json
{
  "matrix": "mono_mix"
}
```
**Tips:** Gain is automatically scaled by 1/sqrt(N) to prevent clipping.

## Tips & Best Practices

- Gain changes are smoothed over ~5 ms to prevent clicks.
- The matrix is stored as a flat array: `[out0_in0, out0_in1, ..., out1_in0, out1_in1, ...]`.
- Solo overrides mute — if any channel is soloed, all non-soloed channels are silenced.
- Dim reduces gain to -20 dB (0.1 linear) — useful for A/B monitoring.
- The matrix supports negative gains for M/S encoding (Mid = L+R, Side = L-R).
- Channel labels automatically adapt to speaker configuration (L/R/C/LS/RS/LFE for surround).
- Use the interactive grid to click cells and scroll to adjust gain by 1 dB steps.

## Signal Flow

```
For each output channel:
  output[out] = Σ(input[in] × matrix[out][in] × state_gain[out])
                    for all input channels

Where state_gain = mute(0.0) | solo(1.0/0.0) | dim(0.1) | normal(1.0)
All gains smoothed over ~5ms
```
