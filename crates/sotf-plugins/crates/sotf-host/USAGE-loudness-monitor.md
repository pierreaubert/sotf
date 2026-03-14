# Loudness Monitor

## Overview

An EBU R128 loudness metering plugin that measures momentary, short-term, and integrated loudness in real time. Also tracks per-channel sample peaks, true peaks, and stereo L/R correlation. Passes audio through unmodified — it only extracts measurement data for display.

## Features

### Loudness Measurements

The plugin provides three standard EBU R128 loudness values, updated in real time:

| Measurement | Window | Description |
|-------------|--------|-------------|
| Momentary LUFS | 400 ms | Fast-responding loudness — reflects what you hear right now |
| Short-term LUFS | 3 s | Medium-responding — useful for mixing decisions |
| Integrated LUFS | Full duration | Overall loudness from start to current position |

### Peak Metering

Per-channel peak measurements for monitoring headroom:

| Measurement | Description |
|-------------|-------------|
| Sample Peak | Maximum sample value per channel (linear) |
| True Peak | Inter-sample peak via oversampling (dBTP) — catches clipping that sample peaks miss |

### Stereo Correlation

For stereo signals, the plugin computes a running L/R Pearson correlation coefficient:

| Value | Meaning |
|-------|---------|
| +1.0 | Mono (identical L and R) |
| 0.0 | Uncorrelated (wide stereo) |
| -1.0 | Out of phase (will cancel in mono) |

The correlation is smoothed with an EMA (alpha=0.15) to avoid jitter.

### Control

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Enabled | On/Off | On | — | Enable/disable metering (saves CPU when not needed) |

## Demos

### Demo: Mastering Loudness Check

**Scenario:** Verifying that a mastered track meets streaming platform loudness targets (-14 LUFS for Spotify).
**Before:** No objective loudness measurement — guessing by ear.
**After:** Integrated LUFS shows the track is at -13.2 LUFS, slightly hot for Spotify.
**Config:**
```json
{
  "enabled": true
}
```

### Demo: Monitoring for Clipping

**Scenario:** A live recording needs real-time monitoring for true peak clipping.
**Before:** Sample peaks look fine but inter-sample peaks cause distortion in the DAC.
**After:** True peak metering reveals +0.3 dBTP overs on the left channel.
**Config:**
```json
{
  "enabled": true
}
```

### Demo: Phase Check

**Scenario:** A stereo mix has phase issues that cause weak center image.
**Before:** The mix sounds thin on mono speakers but fine on headphones.
**After:** Correlation meter shows values near 0.0 — excessive stereo width with poor mono compatibility.
**Config:**
```json
{
  "enabled": true
}
```

## Tips & Best Practices

- The loudness monitor does not modify audio — it's purely a measurement tool.
- Place it at the end of the plugin chain to measure final output loudness.
- Integrated LUFS accumulates from the start of playback — reset the plugin to restart measurement.
- True peak metering is more accurate than sample peak for detecting clipping — use it for final delivery checks.
- Stereo correlation near -1.0 means severe phase cancellation — the signal will disappear in mono playback.
- The plugin supports up to 16 channels for surround and immersive formats.
- Disable the plugin when not actively monitoring to save CPU.

## Signal Flow

```
Input (passthrough) → Ring Buffer → EBU R128 Analysis
                                        ↓
                                   Momentary LUFS (400ms)
                                   Short-term LUFS (3s)
                                   Integrated LUFS (cumulative)
                                   Per-channel Sample Peak
                                   Per-channel True Peak (dBTP)
                                   L/R Correlation (stereo only)
                                        ↓
                                   Display Data (via RealTimeCache)

Output = Input (unmodified)
```
