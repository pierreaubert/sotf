# Parametric EQ

## Overview

A fully parametric equalizer with up to 20 filter bands, supporting multiple filter types (peak, shelving, pass, notch, bandpass). Each band has independent frequency, Q, and gain controls. Supports per-channel filter configurations and automatic gain compensation.

## Features

### Filter Bands

Each band applies an independent filter to the signal. Bands are processed in series (cascaded). By default, bands use standard biquad implementations. Advanced configs may opt into `warped_biquad` or `kautz_filter` per band.

**Per-Band Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Frequency | 20 to 20000 | 1000 | Hz | Center/cutoff frequency |
| Q | 0.1 to 20.0 (Notch: 40.0) | 1.0 | — | Quality factor (bandwidth) |
| Gain | -24 to 24 | 0 | dB | Boost/cut amount (peak and shelf types only) |
| Type | Peak/Lowshelf/Highshelf/Lowpass/Highpass/Bandpass/Notch/AllPass | Peak | — | Filter type |
| Order | 2/4/6/8 | 2 | — | Even standard-biquad cascade order |
| Topology | biquad/warped_biquad/kautz_filter | biquad | — | Runtime filter family |
| Lambda | -0.9999 to 0.9999 | Bark lambda | — | Warping coefficient for warped biquads |

### Filter Types

| Type | Description |
|------|-------------|
| Peak | Bell-shaped boost/cut at the center frequency |
| Lowshelf | Boost/cut all frequencies below the shelf frequency |
| Highshelf | Boost/cut all frequencies above the shelf frequency |
| Lowpass | Passes frequencies below the cutoff, attenuates above |
| Highpass | Passes frequencies above the cutoff, attenuates below |
| Bandpass | Passes a narrow band around the center frequency |
| Notch | Removes a narrow band around the center frequency |
| AllPass | Changes phase while preserving magnitude |

For orders above two, low/high-pass and shelf filters use Butterworth prototype
section Q values. Peak, notch, bandpass, and all-pass cascades scale those section
values by the requested Q, so Q retains its documented bandwidth or phase meaning
and round-trips as the user value.

### Advanced Topologies

Warped biquad bands use the same `filter_type`, `freq`, `q`, and `db_gain` fields as standard biquads, plus `topology: "warped_biquad"`. If `lambda` is omitted, the plugin uses the Bark-scale lambda for the active sample rate.

```json
{
  "filters": [
    {
      "topology": "warped_biquad",
      "filter_type": "peak",
      "freq": 80.0,
      "q": 4.0,
      "db_gain": -6.0,
      "lambda": 0.876
    }
  ]
}
```

Kautz filters are configured as a dry signal plus a parallel Kautz correction bank. Use `kautz_sections` for multiple pole-tuned sections; each section has `pole_freq`, `q`, and `gain`.

```json
{
  "filters": [
    {
      "topology": "kautz_filter",
      "filter_type": "peak",
      "freq": 80.0,
      "q": 6.0,
      "db_gain": 0.0,
      "kautz_sections": [
        {"pole_freq": 63.0, "q": 8.0, "gain": -2.0},
        {"pole_freq": 100.0, "q": 5.0, "gain": -1.5}
      ]
    }
  ]
}
```

### Auto Gain

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Auto Gain | On/Off | On | — | Automatic loudness compensation to match output level to input level |

### Global controls

`tdf2` selects the biquad realization. `topology` selects standard biquad or SVF;
SVF supports only order 2. `oversampling` accepts 1, 2, or 4 for the biquad route.
SVF plus oversampling is rejected. Oversampled callbacks support at most 4096 frames;
larger blocks return an error instead of allocating on the audio thread.

### Per-Channel Mode

The EQ supports independent filter configurations per channel via `channel_filters`. When configured, each channel gets its own filter chain — useful for correcting per-speaker response differences.

## Demos

### Demo: Room Correction

**Scenario:** A room has a boomy bass peak at 80 Hz and a dip at 3 kHz from furniture reflections.
**Before:** Bass is muddy and overwhelming, vocals are recessed.
**After:** Flat response with tight bass and clear midrange presence.
**Config:**
```json
{
  "filters": [
    {"filter_type": "peak", "freq": 80.0, "q": 2.0, "db_gain": -6.0},
    {"filter_type": "peak", "freq": 3000.0, "q": 1.5, "db_gain": 3.0}
  ],
  "auto_gain": {"enabled": true}
}
```

### Demo: Headphone Target Curve

**Scenario:** Applying a Harman-style target curve to flat headphones for more enjoyable listening.
**Before:** Flat headphone response sounds thin and clinical.
**After:** Warm bass shelf, slight presence dip, and air boost for a natural sound.
**Config:**
```json
{
  "filters": [
    {"filter_type": "lowshelf", "freq": 105.0, "q": 0.71, "db_gain": 5.0},
    {"filter_type": "peak", "freq": 3500.0, "q": 1.5, "db_gain": -2.0},
    {"filter_type": "highshelf", "freq": 10000.0, "q": 0.71, "db_gain": 2.0}
  ],
  "auto_gain": {"enabled": false}
}
```

### Demo: Surgical Problem Fix

**Scenario:** A recording has a harsh resonance at 5.5 kHz and 60 Hz hum.
**Before:** Listening fatigue from the resonance, audible hum.
**After:** Clean signal with the resonance and hum removed.
**Config:**
```json
{
  "filters": [
    {"filter_type": "notch", "freq": 60.0, "q": 8.0, "db_gain": 0.0},
    {"filter_type": "peak", "freq": 5500.0, "q": 5.0, "db_gain": -8.0}
  ]
}
```

## Presets

### Flat (Bypass)
**Use case:** No EQ applied — reference listening
```json
{
  "filters": [],
  "auto_gain": {"enabled": false}
}
```
**Tips:** Use this as a comparison baseline.

### Harman Over-Ear
**Use case:** Harman target curve for over-ear headphones
```json
{
  "filters": [
    {"filter_type": "lowshelf", "freq": 105.0, "q": 0.71, "db_gain": 5.5},
    {"filter_type": "peak", "freq": 3500.0, "q": 1.5, "db_gain": -2.5},
    {"filter_type": "highshelf", "freq": 10000.0, "q": 0.71, "db_gain": 1.5}
  ],
  "auto_gain": {"enabled": true}
}
```
**Tips:** Adjust bass shelf gain to taste. The Harman curve is a preference target, not a measurement target.

### Bass Boost
**Use case:** Extra low-end for genres that benefit from it
```json
{
  "filters": [
    {"filter_type": "lowshelf", "freq": 80.0, "q": 0.5, "db_gain": 6.0},
    {"filter_type": "peak", "freq": 200.0, "q": 1.0, "db_gain": -2.0}
  ],
  "auto_gain": {"enabled": true}
}
```
**Tips:** The compensating cut at 200 Hz prevents bass from bleeding into the midrange.

### Vocal Clarity
**Use case:** Enhance speech intelligibility
```json
{
  "filters": [
    {"filter_type": "highpass", "freq": 80.0, "q": 0.71, "db_gain": 0.0},
    {"filter_type": "peak", "freq": 2500.0, "q": 1.0, "db_gain": 3.0},
    {"filter_type": "peak", "freq": 5000.0, "q": 1.5, "db_gain": 2.0}
  ],
  "auto_gain": {"enabled": true}
}
```
**Tips:** The highpass removes rumble while the presence boost improves clarity.

## Tips & Best Practices

- Use narrow Q values (3+) for surgical cuts and wide Q values (0.5–1.0) for musical shaping.
- Cut rather than boost when possible — cuts sound more natural and reduce clipping risk.
- Enable Auto Gain to maintain consistent perceived loudness while EQ'ing.
- Use Lowshelf/Highshelf for broad tonal changes, Peak for targeted corrections.
- Notch filters are ideal for removing specific resonances or hum frequencies.
- Per-channel mode allows correcting individual speaker responses in a multi-channel system.
- Filters are cascaded in series — order matters when filters overlap in frequency.

## Signal Flow

```
Input → [Band 1 Biquad] → [Band 2 Biquad] → ... → [Band N Biquad] → Auto Gain → Output
```
