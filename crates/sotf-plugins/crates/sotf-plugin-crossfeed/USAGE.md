# Crossfeed

## Overview

A headphone crossfeed plugin that blends a portion of each stereo channel into the opposite ear, simulating the natural acoustic crosstalk of speaker listening. Reduces the exaggerated stereo separation of headphones for a more natural, less fatiguing experience. Supports three algorithms: Bauer (classic), Meier (frequency-dependent), and Multiband (per-band control).

## Features

### Crossfeed Modes

Three algorithms for different listening preferences:

**Bauer Mode:** Classic crossfeed based on Bauer's bs2b algorithm. A high-pass filtered version of each channel is fed to the opposite ear. Simple and effective.

**Meier Mode:** Jan Meier's natural crossfeed. Uses a low-pass filter followed by an all-pass for frequency-dependent crossfeed that better models natural acoustic crosstalk.

**Multiband Mode:** Splits the signal into three frequency bands (low/mid/high) and applies independent crossfeed levels per band. Provides the most control over the crossfeed character.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Mode | Off/Bauer/Meier/Multiband | Bauer | — | Crossfeed algorithm selection |
| Mix | 0 to 1.0 | 1.0 | — | Dry/wet blend |

### Bauer Parameters

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Bauer Cutoff | 300 to 2000 | 700 | Hz | High-pass filter cutoff for the crossfeed signal |
| Bauer Feed | 0 to 12 | 4.5 | dB | Crossfeed level (how much opposite channel is mixed in) |

### Meier Parameters

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Meier Level | 0 to 100 | 30 | % | Crossfeed intensity (percentage of low-pass filtered signal) |

### Multiband Parameters

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| MB Low Freq | 50 to 500 | 150 | Hz | Low/mid crossover frequency |
| MB Mid-High Freq | 1000 to 10000 | 5700 | Hz | Mid/high crossover frequency |
| MB Low Feed | -12 to 12 | 0 | dB | Low band crossfeed level |
| MB Mid Feed | -12 to 12 | 6 | dB | Mid band crossfeed level |
| MB High Feed | -12 to 12 | 3 | dB | High band crossfeed level |

### Auto Gain

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Auto Gain | On/Off | Off | — | Automatic loudness compensation |

## Demos

### Demo: Natural Headphone Listening

**Scenario:** Extended headphone listening is fatiguing due to extreme stereo separation.
**Before:** Hard-panned instruments feel unnaturally pinned to one ear.
**After:** Instruments have a more natural, speaker-like spatial presentation.
**Config:**
```json
{
  "mode": "Bauer",
  "bauer_fcut_hz": 700.0,
  "bauer_feed_db": 4.5,
  "mix": 1.0
}
```

### Demo: Studio Monitoring Simulation

**Scenario:** A mix needs to be checked for mono compatibility while on headphones.
**Before:** Stereo monitoring on headphones doesn't reveal phase cancellation issues.
**After:** Crossfeed simulates speaker interaction, revealing potential mono problems.
**Config:**
```json
{
  "mode": "Meier",
  "meier_level": 30.0,
  "mix": 1.0
}
```

### Demo: Frequency-Selective Crossfeed

**Scenario:** Bass should remain wide (no crossfeed) while mids and highs need more natural blending.
**Before:** Uniform crossfeed reduces bass impact.
**After:** Bass stays wide and punchy, mids and highs blend naturally.
**Config:**
```json
{
  "mode": "Mb",
  "mb_low_freq_hz": 150.0,
  "mb_mid_high_freq_hz": 5700.0,
  "mb_low_feed_db": 0.0,
  "mb_mid_feed_db": 6.0,
  "mb_high_feed_db": 3.0,
  "mix": 1.0
}
```

## Presets

### Bauer Default
**Use case:** Classic crossfeed for general headphone listening
```json
{
  "mode": "Bauer",
  "bauer_fcut_hz": 700.0,
  "bauer_feed_db": 4.5,
  "mix": 1.0
}
```
**Tips:** The standard starting point. Increase feed for more blending, decrease cutoff for more low-end crossfeed.

### Cmoy
**Use case:** Stronger crossfeed inspired by the Cmoy headphone amp circuit
```json
{
  "mode": "Bauer",
  "bauer_fcut_hz": 700.0,
  "bauer_feed_db": 6.0,
  "mix": 1.0
}
```
**Tips:** More aggressive crossfeed. Good for highly stereo-separated recordings from the 60s-70s.

### Meier Natural
**Use case:** Frequency-dependent crossfeed modeling natural acoustic crosstalk
```json
{
  "mode": "Meier",
  "meier_level": 30.0,
  "mix": 1.0
}
```
**Tips:** Meier crossfeed applies more blending at low frequencies where natural crosstalk is strongest.

### Multiband Custom
**Use case:** Full control over per-band crossfeed
```json
{
  "mode": "Mb",
  "mb_low_freq_hz": 150.0,
  "mb_mid_high_freq_hz": 5700.0,
  "mb_low_feed_db": 0.0,
  "mb_mid_feed_db": 6.0,
  "mb_high_feed_db": 3.0,
  "mix": 1.0
}
```
**Tips:** Set low feed to 0 dB to keep bass width. Increase mid feed for more vocal blending.

## Tips & Best Practices

- Crossfeed is for headphone listening only — do not use with speakers.
- Start with Bauer mode for simplicity, switch to Multiband for fine-tuning.
- Higher feed values (6+ dB) narrow the stereo image significantly — start subtle.
- Use the Mix parameter to blend between original (0) and crossfed (1.0) for comparison.
- Auto Gain compensates for any loudness change from the crossfeed processing.
- Presets select the mode and set appropriate parameters — changing a preset overrides mode-specific settings.

## Signal Flow

```
Bauer:
  Input L/R → HPF → Crossfeed (L→R, R→L) × feed_gain → Sum with original → Mix → Output

Meier:
  Input L/R → LPF → AllPass → Crossfeed × meier_level → Sum with original → Mix → Output

Multiband:
  Input L/R → Band Split (Low/Mid/High per channel)
           → Per-band crossfeed (L→R, R→L) × band_feed_gain
           → Band Sum → Mix → Output
```
