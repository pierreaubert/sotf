# Limiter

## Overview

A peak limiter that prevents audio from exceeding a ceiling threshold. Use it as the last plugin in a chain to prevent clipping, or to maximize loudness without distortion. Supports lookahead for transparent peak catching and optional soft knee clipping.

## Features

### Peak Limiting

Applies instant-attack gain reduction when the signal exceeds the threshold. Unlike a compressor, a limiter has an effectively infinite ratio — nothing gets past the ceiling.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Threshold | -20 to 0 | -0.1 | dB | Ceiling level. Output will not exceed this value |
| Release | 10 to 1000 | 50 | ms | Time to recover from gain reduction after peaks pass |

### Lookahead

Reads ahead in the audio stream to anticipate peaks and apply gain reduction before the peak arrives. This prevents transient overshoot at the cost of added latency.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Lookahead | 0 to 20 | 5 | ms | Lookahead time for peak anticipation. Higher = more transparent but more latency |

### Clipping Mode

Choose between hard limiting and a one-dB, dB-domain soft knee in the gain computer. The soft mode changes onset without adding a post-gain saturation waveshaper.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Soft Knee | Soft/Hard | Hard | — | Hard: brick-wall onset. Soft: gradual one-dB gain-reduction knee |

### Mix

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Mix | 0 to 100 | 100 | % | Dry/wet blend. 0% = bypass, 100% = fully limited |

## Demos

### Demo: Transparent Peak Control

**Scenario:** A mastered track needs to stay below -0.1 dBFS for streaming platform compliance.
**Before:** Occasional peaks at 0 dBTP trigger platform limiting or rejection.
**After:** All peaks cleanly caught below -0.1 dB with no audible artifacts.
**Config:**
```json
{
  "threshold_db": -0.1,
  "release_ms": 100.0,
  "lookahead_ms": 5.0,
  "soft": false
}
```

### Demo: Loudness Maximizer

**Scenario:** A mix needs to sound louder without increasing peaks.
**Before:** Mix peaks at -6 dB with plenty of dynamic range.
**After:** 6 dB of gain applied upstream, with the limiter catching all peaks at -0.1 dB. Perceived loudness increases significantly.
**Config:**
```json
{
  "threshold_db": -0.1,
  "release_ms": 50.0,
  "lookahead_ms": 5.0,
  "soft": false
}
```

### Demo: Warm Soft Limiting

**Scenario:** Drums or bass need limiting with a warmer, more analog character.
**Before:** Hard limiter creates sharp transient shaping.
**After:** Soft knee creates gradual saturation that preserves some transient character.
**Config:**
```json
{
  "threshold_db": -3.0,
  "release_ms": 30.0,
  "lookahead_ms": 3.0,
  "soft": true
}
```

## Presets

### Transparent Ceiling
**Use case:** Final limiter for streaming/broadcast compliance
```json
{
  "threshold_db": -0.1,
  "release_ms": 100.0,
  "lookahead_ms": 5.0,
  "soft": false,
  "mix": 1.0
}
```
**Tips:** Keep threshold at -0.1 to -1.0 dB. Longer release (100-200 ms) for transparency.

### Aggressive Loudness
**Use case:** Maximize loudness for competitive levels
```json
{
  "threshold_db": -0.3,
  "release_ms": 30.0,
  "lookahead_ms": 5.0,
  "soft": false,
  "mix": 1.0
}
```
**Tips:** Add gain before the limiter. Watch for pumping with very short release times.

### Soft Saturation
**Use case:** Warm limiting with analog character
```json
{
  "threshold_db": -3.0,
  "release_ms": 50.0,
  "lookahead_ms": 3.0,
  "soft": true,
  "mix": 1.0
}
```
**Tips:** Soft mode works best when you're pushing 3-6 dB into the limiter.

### Safety Net
**Use case:** Catch unexpected peaks without altering the sound
```json
{
  "threshold_db": -0.5,
  "release_ms": 200.0,
  "lookahead_ms": 10.0,
  "soft": false,
  "mix": 1.0
}
```
**Tips:** Long release and generous lookahead ensure the limiter only activates on true peaks.

## Tips & Best Practices

- Place the limiter last in the plugin chain — after EQ, compression, and other processing.
- The lookahead adds latency equal to the lookahead time. Use 3-5 ms for a good balance.
- For true peak limiting (inter-sample peaks), set threshold to -1 dBTP or lower.
- ISP limiting requires hard mode, 100% wet mix, and at least six samples of lookahead.
- Lookahead changes require a graph rebuild so host latency compensation remains correct.
- If the GR meter never returns to 0, the input is too hot — reduce gain before the limiter.
- Release time affects pumping: too short = audible pumping, too long = sustained gain reduction.

## Signal Flow

```
Input → Lookahead Buffer → Peak Detection (instant attack)
                               ↓
              Envelope Follower (release smoothing)
                               ↓
Input (delayed) → Gain Reduction (soft/hard knee) → Safety Ceiling → Mix → Output
```

The lookahead buffer delays the audio path while the peak detector sees the signal ahead of time. This allows gain reduction to begin before the peak arrives.
