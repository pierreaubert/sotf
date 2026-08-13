# Matrix

## Overview

A flexible channel routing and mixing matrix that maps N input channels to P output channels with configurable per-connection gains. Supports mute, solo, and dim per output channel with smoothed gain transitions. Used for surround routing, channel remapping, M/S encoding, and custom mixes.

## Features

### Gain Matrix

An N×P matrix where each cell represents the gain from one input channel to one output channel. Values and generic host parameters are bounded linear coefficients. A custom UI may display their magnitude in dB, but converts to/from the linear API explicitly.

**Per-Cell Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Gain | -1.0 to +1.0 | varies | linear | Signed gain from input to output. Identity matrix: diagonal = 1, off-diagonal = 0 |

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
| `custom` | User-defined matrix |
| `stereo_downmix` | 2→2 pass-through, or normalized SMPTE/WAVE 5.1→2 (LFE omitted) |
| `ms_encode` | M=(L+R)/2, S=(L-R)/2 in the first two outputs |
| `ms_decode` | L=M+S, R=M-S from the first two inputs |
| `5.1_remap` | Exact 6×6 SMPTE/WAVE `[L,R,C,LFE,Ls,Rs]` to AAC `[C,L,R,Ls,Rs,LFE]` |

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

Presets are selected by the `preset` integer parameter in the order listed above. A failed preset selection leaves both the prior matrix and preset unchanged.

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
The global gain, signed connection gain (including phase changes), and state gain are smoothed over ~5 ms.
```
