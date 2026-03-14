# Loudness Compensation

## Overview

A simple loudness compensation plugin that applies low-shelf and high-shelf filters to boost bass and treble at lower listening volumes. Based on the principle that human hearing is less sensitive to low and high frequencies at lower volumes (equal-loudness contours). Uses cascaded biquad shelving filters with smoothed gain transitions.

## Features

### Shelf EQ

Two pairs of cascaded biquad shelving filters (2x low shelf + 2x high shelf) apply frequency-dependent gain compensation. The double-cascading provides a steeper shelf slope for more effective compensation.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Bass Boost | -20 to 20 | 6 | dB | Low-frequency shelf gain |
| Treble Boost | -20 to 20 | 6 | dB | High-frequency shelf gain |
| Low Frequency | 20 to 500 | 100 | Hz | Low shelf center frequency |
| High Frequency | 2000 to 20000 | 10000 | Hz | High shelf center frequency |

### Auto Gain

Optional automatic gain compensation to prevent overall volume increase from the shelving filters.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Auto Gain | On/Off | Off | — | Enable automatic loudness compensation |
| Max Auto Gain | 0 to 24 | 12 | dB | Maximum auto-gain correction |
| Smoothing | 1 to 1000 | 100 | ms | Auto-gain transition smoothing |

## Demos

### Demo: Low-Volume Listening

**Scenario:** Listening to music at low volume where bass and treble seem to disappear.
**Before:** At -30 dB playback, bass is inaudible and highs are dull.
**After:** Bass and treble are restored to perceptually balanced levels.
**Config:**
```json
{
  "low_freq": 100.0,
  "low_gain": 8.0,
  "high_freq": 10000.0,
  "high_gain": 4.0
}
```

### Demo: Desktop Speaker Correction

**Scenario:** Small desktop speakers lack bass extension.
**Before:** Thin sound with no low-end presence below 150 Hz.
**After:** Low shelf boost fills in the missing bass range.
**Config:**
```json
{
  "low_freq": 120.0,
  "low_gain": 10.0,
  "high_freq": 8000.0,
  "high_gain": 3.0
}
```

### Demo: Treble Rolloff Compensation

**Scenario:** Headphones have a noticeable treble rolloff above 8 kHz.
**Before:** Cymbals and hi-hats sound dull and lifeless.
**After:** High shelf restores air and sparkle.
**Config:**
```json
{
  "low_freq": 100.0,
  "low_gain": 0.0,
  "high_freq": 8000.0,
  "high_gain": 6.0
}
```

## Presets

### Moderate
**Use case:** Gentle compensation for moderate listening volumes
```json
{
  "low_freq": 100.0,
  "low_gain": 4.0,
  "high_freq": 10000.0,
  "high_gain": 3.0
}
```
**Tips:** Good starting point. Increase gains if listening at very low volumes.

### Strong Bass
**Use case:** Significant bass boost for small speakers or very low volumes
```json
{
  "low_freq": 80.0,
  "low_gain": 10.0,
  "high_freq": 10000.0,
  "high_gain": 4.0
}
```
**Tips:** May clip with loud bass-heavy content — enable Auto Gain or reduce source volume.

### Flat (Bypass)
**Use case:** No compensation applied
```json
{
  "low_freq": 100.0,
  "low_gain": 0.0,
  "high_freq": 10000.0,
  "high_gain": 0.0
}
```
**Tips:** Use for A/B comparison against compensated signal.

## Tips & Best Practices

- This is a simpler alternative to the Fletcher-Munson plugin — it uses fixed gain values rather than volume-dependent curves.
- The filters preserve delay state on parameter changes for click-free transitions.
- Each shelf is applied twice (cascaded) for a steeper slope — actual boost is approximately 2x the gain value at the extremes.
- A built-in compensation gain attenuates the output by the maximum of the two shelf gains to prevent clipping.
- Use Auto Gain for level-matched comparison when adjusting parameters.
- For volume-dependent compensation that adapts automatically, use the Fletcher-Munson plugin instead.

## Signal Flow

```
Input → [Low Shelf 1] → [Low Shelf 2] → [High Shelf 1] → [High Shelf 2]
      → Compensation Gain (smoothed) → Auto Gain (optional) → Output
```
