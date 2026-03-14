# Fletcher-Munson

## Overview

A volume-dependent loudness compensation plugin based on the Fletcher-Munson equal-loudness contours (ISO 226). Automatically adjusts 4 parametric EQ bands based on the difference between the current playback volume and a reference level, compensating for the ear's reduced sensitivity to bass and treble at lower volumes. The compensation increases as volume decreases — at the reference level, no correction is applied.

## Features

### Levels

The compensation amount is driven by the delta between reference and playback volume. When playback volume equals the reference, no compensation is applied. As playback drops below the reference, bass and treble are progressively boosted.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Playback Volume | -80 to 0 | 0 | dB | Current playback volume (set by engine or UI) |
| Reference Level | -40 to 0 | -14 | dB | Volume where response is flat (~80 dB SPL typical) |
| Enabled | On/Off | On | — | Enable/disable compensation |

### Compensation Bands

Four peak EQ bands targeting different frequency regions. Each band has a maximum gain cap and a slope that determines how quickly gain increases as volume decreases.

**Band 1 — Sub-Bass (~60 Hz):**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Frequency | 20 to 20000 | 60 | Hz | Band center frequency |
| Q | 0.1 to 10 | 0.5 | — | Bandwidth |
| Max Gain | 0 to 24 | 15 | dB | Maximum compensation gain |
| Slope | 0 to 1.0 | 0.6 | — | Gain per dB of volume delta |

**Band 2 — Mid-Bass (~250 Hz):**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Frequency | 20 to 20000 | 250 | Hz | Band center frequency |
| Q | 0.1 to 10 | 0.707 | — | Bandwidth |
| Max Gain | 0 to 24 | 8 | dB | Maximum compensation gain |
| Slope | 0 to 1.0 | 0.4 | — | Gain per dB of volume delta |

**Band 3 — Presence (~3.5 kHz):**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Frequency | 20 to 20000 | 3500 | Hz | Band center frequency |
| Q | 0.1 to 10 | 1.0 | — | Bandwidth |
| Max Gain | 0 to 24 | 3 | dB | Maximum compensation gain |
| Slope | 0 to 1.0 | 0.15 | — | Gain per dB of volume delta |

**Band 4 — Air (~12 kHz):**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Frequency | 20 to 20000 | 12000 | Hz | Band center frequency |
| Q | 0.1 to 10 | 0.5 | — | Bandwidth |
| Max Gain | 0 to 24 | 10 | dB | Maximum compensation gain |
| Slope | 0 to 1.0 | 0.45 | — | Gain per dB of volume delta |

### Auto Gain

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Auto Gain | On/Off | Off | — | Automatic loudness compensation |
| Max Correction | 0 to 24 | 12 | dB | Maximum auto-gain correction |
| AG Smoothing | 10 to 500 | 100 | ms | Smoothing time for gain transitions |
| AG Loudness Type | Momentary/ShortTerm | Momentary | — | Measurement window for auto gain |

## Demos

### Demo: Late-Night Listening

**Scenario:** Listening at very low volume late at night (-40 dB below reference).
**Before:** Bass and treble are inaudible — only midrange comes through.
**After:** Sub-bass gets +15 dB boost, treble gets +10 dB — full-range sound at whisper volume.
**Config:**
```json
{
  "playback_volume_db": -40.0,
  "reference_level_db": -14.0,
  "enabled": true
}
```

### Demo: Moderate Volume Correction

**Scenario:** Background music at moderate volume (-25 dB).
**Before:** Bass is slightly weak, highs are a touch dull.
**After:** Gentle compensation restores tonal balance without being obvious.
**Config:**
```json
{
  "playback_volume_db": -25.0,
  "reference_level_db": -14.0,
  "enabled": true
}
```

### Demo: Reference Level (No Correction)

**Scenario:** Listening at reference level where the mix was intended to be heard.
**Before/After:** No change — playback equals reference, so delta is 0 and no compensation is applied.
**Config:**
```json
{
  "playback_volume_db": -14.0,
  "reference_level_db": -14.0,
  "enabled": true
}
```

## Presets

### Default ISO 226
**Use case:** Standard Fletcher-Munson compensation
```json
{
  "playback_volume_db": 0.0,
  "reference_level_db": -14.0,
  "enabled": true,
  "band1": {"frequency": 60.0, "q": 0.5, "max_gain_db": 15.0, "slope": 0.6},
  "band2": {"frequency": 250.0, "q": 0.707, "max_gain_db": 8.0, "slope": 0.4},
  "band3": {"frequency": 3500.0, "q": 1.0, "max_gain_db": 3.0, "slope": 0.15},
  "band4": {"frequency": 12000.0, "q": 0.5, "max_gain_db": 10.0, "slope": 0.45}
}
```
**Tips:** Set playback_volume_db to match your actual volume control. The plugin calculates compensation automatically.

### Gentle
**Use case:** Subtle correction for moderate volume listening
```json
{
  "playback_volume_db": 0.0,
  "reference_level_db": -14.0,
  "enabled": true,
  "band1": {"frequency": 60.0, "q": 0.5, "max_gain_db": 8.0, "slope": 0.3},
  "band2": {"frequency": 250.0, "q": 0.707, "max_gain_db": 4.0, "slope": 0.2},
  "band3": {"frequency": 3500.0, "q": 1.0, "max_gain_db": 2.0, "slope": 0.1},
  "band4": {"frequency": 12000.0, "q": 0.5, "max_gain_db": 5.0, "slope": 0.2}
}
```
**Tips:** Halved slopes and max gains for less aggressive correction. Good if the default feels heavy-handed.

### Bass-Heavy
**Use case:** Extra bass compensation for small speakers
```json
{
  "playback_volume_db": 0.0,
  "reference_level_db": -14.0,
  "enabled": true,
  "band1": {"frequency": 50.0, "q": 0.4, "max_gain_db": 20.0, "slope": 0.8},
  "band2": {"frequency": 200.0, "q": 0.6, "max_gain_db": 12.0, "slope": 0.5},
  "band3": {"frequency": 3500.0, "q": 1.0, "max_gain_db": 3.0, "slope": 0.15},
  "band4": {"frequency": 12000.0, "q": 0.5, "max_gain_db": 10.0, "slope": 0.45},
  "auto_gain_enabled": true
}
```
**Tips:** Aggressive bass compensation — enable Auto Gain to prevent clipping.

## Tips & Best Practices

- The plugin is designed to be driven by the system volume control — `playback_volume_db` should track the master volume.
- At the reference level, compensation is zero (flat response). Below the reference, compensation increases.
- Above the reference level, no negative compensation is applied — gains are clamped to 0.
- The reference level of -14 dB corresponds roughly to 80 dB SPL for calibrated systems.
- Band gains are smoothed (50 ms) to avoid clicks when volume changes.
- A compensation attenuator automatically reduces output by the maximum band gain to prevent clipping.
- For simpler fixed-gain compensation, use the Loudness Compensation plugin instead.
- The presence band (3.5 kHz) has the lowest slope because the ear is most sensitive there — less correction needed.

## Signal Flow

```
Volume Delta = reference_level_db - playback_volume_db
                        ↓
For each band: gain = min(slope × delta, max_gain_db)  [clamped to 0 if delta ≤ 0]
                        ↓
Input → [Band 1 Peak EQ] → [Band 2 Peak EQ] → [Band 3 Peak EQ] → [Band 4 Peak EQ]
      → Compensation Attenuator (smoothed) → Auto Gain (optional) → Output
```
