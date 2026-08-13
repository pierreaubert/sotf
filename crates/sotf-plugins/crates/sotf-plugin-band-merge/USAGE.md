# Band Merge

## Overview

Sums multiple frequency bands back into a single channel stream. The inverse of Band Split — takes N×B input channels (N output channels × B bands) and sums corresponding band channels together to reconstruct the original channel count.

## Features

### Configuration

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Bands | 2 to 8 | 2 | — | Number of bands to merge (must match the splitting plugin) |

### Channel Layout

For B bands and N output channels, the input has B×N channels:
- Input channels 0..N-1: Band 0
- Input channels N..2N-1: Band 1
- ...
- Input channels (B-1)×N..B×N-1: Band B-1

Each output channel is the sum of all corresponding band channels.

## Demos

### Demo: Recombining After Parallel Processing

**Scenario:** After Band Split + per-band processing, merge bands back together.
**Before:** 4-channel split signal (low-L, low-R, high-L, high-R).
**After:** 2-channel stereo with per-band processing baked in.
**Config:**
```json
{
  "bands": 2
}
```

## Presets

### Two-Band Merge
**Use case:** Merge output from a single Band Split
```json
{
  "bands": 2
}
```
**Tips:** Standard pairing with Band Split.

## Tips & Best Practices

- The band count must match the number of bands from the splitting plugin.
- Band Merge simply sums. A two-band Linkwitz-Riley split is magnitude-
  complementary but phase-shifted; cascaded multiband outputs have unequal group
  delay and are not phase-perfect even without per-band processing.
- No gain scaling is applied — if per-band processing added gain, the sum may clip.
- Denormals are flushed after merging to prevent CPU spikes.

## Host and realtime contract

- Initialize before processing. Callback rate must match initialization and both
  buffers must have exact checked lengths.
- Band count is limited to 2–8 and changes require graph replacement; exact
  no-op writes succeed. Presets reject unknown fields and invalid gains.
- Gain and mute transitions use one allocation-free 10 ms sample-rate-aware
  smoother. Reset snaps each band to its configured mute/gain target.
- Reading `reconstruction_error_db` arms one callback to compute normalized
  RMS(`output - unity-band-sum`) without logging or allocation.

## Signal Flow

```
Input (B × N channels) → Sum per output channel:
  output[ch] = input[ch] + input[ch + N] + input[ch + 2N] + ... + input[ch + (B-1)×N]
→ Output (N channels)
```
