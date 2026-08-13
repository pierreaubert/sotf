# Gain

## Overview

A simple volume control plugin that applies a fixed gain (in dB) to the audio signal. Use it for level matching, volume trimming, or per-channel gain adjustments. Supports both global and per-channel modes with smooth parameter transitions.

## Features

### Volume Control

Applies a gain offset in decibels to all channels uniformly, or independently per channel when per-channel mode is active.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Gain | -60 to 20 | 0 | dB | Global gain applied to all channels. 0 dB = unity (no change) |
| Gain Ch N | -60 to 20 | 0 | dB | Per-channel gain (only visible in per-channel mode) |

### Per-Channel Mode

When per-channel gains are configured, each channel can have its own independent gain value. This is useful for correcting channel imbalances or creating custom downmix scenarios.

### Smoothing

Gain changes use the configured smoothing time (10 ms by default) to prevent clicks and pops during parameter changes. SIMD-optimized processing ensures minimal CPU usage.

## Demos

### Demo: Level Matching

**Scenario:** Two tracks at different recording levels need to play back at the same perceived volume.
**Before:** Track A is 6 dB louder than track B.
**After:** Track B boosted by 6 dB to match.
**Config:**
```json
{
  "gain_db": 6.0
}
```

### Demo: Channel Balance Correction

**Scenario:** A stereo recording where the right channel is 2 dB louder than the left.
**Before:** Stereo image is shifted to the right.
**After:** Channels are balanced.
**Config:**
```json
{
  "channel_gains": [0.0, -2.0]
}
```

## Presets

### Unity (Bypass)
**Use case:** Pass-through with no gain change
```json
{
  "gain_db": 0.0
}
```

### -6 dB Headroom
**Use case:** Create headroom before a processing chain
```json
{
  "gain_db": -6.0
}
```

### +3 dB Boost
**Use case:** Gentle boost for quiet sources
```json
{
  "gain_db": 3.0
}
```

## Tips & Best Practices

- Use gain before a compressor to drive more signal into the threshold.
- Use gain after processing to match the output level to the original.
- For level matching between tracks, use ReplayGain values as a starting point.
- Per-channel mode is useful for correcting L/R imbalance in stereo recordings.
- The plugin uses SIMD-optimized processing and adds negligible CPU overhead.
