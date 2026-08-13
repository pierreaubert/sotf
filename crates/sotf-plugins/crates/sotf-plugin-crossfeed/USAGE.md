# Crossfeed

## Overview

A headphone crossfeed plugin that blends a portion of each stereo channel into the opposite ear, simulating the natural acoustic crosstalk of speaker listening. Reduces the exaggerated stereo separation of headphones for a more natural, less fatiguing experience. Supports three algorithms: Bauer (classic), Meier (frequency-dependent), and Multiband (per-band control).

## Features

### Crossfeed Modes

Three algorithms for different listening preferences:

**Bauer Mode:** A low-shelf cut is applied to the stereo difference signal, reducing low-frequency width while preserving mono content.

**Meier Mode:** Jan Meier's natural crossfeed. Uses a low-pass filter followed by an all-pass for frequency-dependent crossfeed that better models natural acoustic crosstalk.

**Multiband Mode:** Splits the signal into three frequency bands (low/mid/high) and applies independent crossfeed levels per band. Provides the most control over the crossfeed character.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Mode | Off/Bauer/Meier/Multiband | Multiband | — | Crossfeed algorithm selection |
| Mix | 0 to 1.0 | 1.0 | — | Dry/wet blend |

### Bauer Parameters

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Bauer Cutoff | 400 to 1000 | 700 | Hz | Low-shelf transition frequency on the stereo difference |
| Bauer Feed | 0 to 15 | 4.5 | dB | Low-frequency difference attenuation |

### Meier Parameters

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Meier Level | 0 to 100 | 30 | % | Crossfeed intensity (percentage of low-pass filtered signal) |

### Multiband Parameters

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| MB Low Freq | 50 to 500 | 150 | Hz | Low/mid crossover frequency |
| MB Mid-High Freq | 2000 to 15000 | 5700 | Hz | Mid/high crossover frequency |
| MB Low Feed | -60 to 15 | 0 | dB | Opposite-channel low-band gain; -60 dB is Off and 0 dB is unity |
| MB Mid Feed | -60 to 15 | 6 | dB | Opposite-channel mid-band gain; -60 dB is Off |
| MB High Feed | -60 to 15 | 3 | dB | Opposite-channel high-band gain; -60 dB is Off |

### Auto Gain

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Auto Gain | On/Off | Off | — | Automatic loudness compensation |
| Target LUFS | -40 to -12 | -18 | LUFS | Reserved compatibility control; currently has no DSP effect |
| Max Gain | 0 to 24 | 12 | dB | Maximum compensation |
| Smoothing | 10 to 5000 | 100 | ms | Compensation transition time |

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

**Scenario:** Bass needs less opposite-channel feed than mids and highs.
**Before:** Uniform crossfeed narrows every band equally.
**After:** Bass receives the minimum available feed while mids and highs blend more strongly.
**Config:**
```json
{
  "mode": "Mb",
  "mb_low_freq_hz": 150.0,
  "mb_mid_high_freq_hz": 5700.0,
  "mb_low_feed_db": -60.0,
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
  "mb_low_feed_db": -20.0,
  "mb_mid_feed_db": 6.0,
  "mb_high_feed_db": 3.0,
  "mix": 1.0
}
```
**Tips:** Use -60 dB for a true low-band Off endpoint. A 0 dB setting is unity crossfeed, not bypass. Increase mid feed for more vocal blending.

## Tips & Best Practices

- Crossfeed is for headphone listening only — do not use with speakers.
- Start with Bauer mode for simplicity, switch to Multiband for fine-tuning.
- Higher feed values (6+ dB) narrow the stereo image significantly — start subtle.
- Use the Mix parameter to blend between original (0) and crossfed (1.0) for comparison.
- Auto Gain compensates for any loudness change from the crossfeed processing.
- Presets select the mode and set appropriate parameters — changing a preset overrides mode-specific settings.

## Realtime and block-size contract

`max_block_frames` is a setup-time serialized field, not an automatable control. The plugin
allocates four scratch buffers at exactly that size during construction (default 16,384 frames)
and rejects larger callbacks instead of allocating on the audio thread. Set it to the graph's
known maximum callback size for tighter memory use or large offline blocks. Parameter changes and
`reset()` are allocation-free; enabled and mode changes use a deterministic reset-on-transition
policy rather than replaying inactive filter history.

## Signal Flow

```
Bauer:
  Input L/R → Difference (L-R) → Low-shelf attenuation → Width reconstruction → Mix → Output

Meier:
  Input L/R → LPF → AllPass → Crossfeed × meier_level → Sum with original → Mix → Output

Multiband:
  Input L/R → Band Split (Low/Mid/High per channel)
           → Per-band crossfeed (L→R, R→L) × band_feed_gain
           → Band Sum → Mix → Output
```
