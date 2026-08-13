# Multiband Expander

## Overview

A multiband downward expander that splits the audio into 2-5 frequency bands using Linkwitz-Riley crossovers, expands each band independently, then sums them back together. Use it for frequency-selective noise reduction, tightening specific ranges without affecting others, or isolating bleed in particular bands.

The separate `expander` factory entry is broadband: it uses one detector and one
gain decision for the complete signal and exposes no crossover/band-count UI.
Changing the multiband count or switching time-domain/spectral processing
requires rebuilding the plugin; realtime automation of structural controls is
rejected.

Time-domain detector input can be high-passed from 0-500 Hz. Lookahead delays
every active, passive, and bypassed band equally. Spectral mode is intentionally
restricted to linked Peak detection with lookahead, detector HPF, and auto makeup
off; presets requesting unsupported feature combinations are rejected.

## Features

### Band Splitting

The audio is split into frequency bands using 4th-order Linkwitz-Riley (LR4) crossover filters. Each crossover point creates a lowpass and highpass pair. The bands are processed independently and summed back for the output.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Bands | 2 to 5 | 3 | — | Number of frequency bands (structural — requires rebuild) |
| Crossover 1 | 20 to 500 | 200 | Hz | First crossover frequency (bass/mids split) |
| Crossover 2 | 500 to 5000 | 2000 | Hz | Second crossover frequency (mids/highs split) |
| Crossover 3 | 5000 to 15000 | 8000 | Hz | Third crossover (for 4+ bands) |
| Crossover 4 | 10000 to 18000 | 12000 | Hz | Fourth crossover (for 5 bands) |

**Crossover Presets:**

| Preset | X-Over 1 | X-Over 2 | X-Over 3 | X-Over 4 |
|--------|----------|----------|----------|----------|
| 0 | 200 Hz | 2000 Hz | 8000 Hz | 12000 Hz |
| 1 | 100 Hz | 3000 Hz | 8000 Hz | 12000 Hz |
| 2 | 250 Hz | 4000 Hz | 10000 Hz | 14000 Hz |

### Global Dynamics

Sets default expansion parameters for all bands. Individual bands can override these values.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Threshold | -80 to 0 | -40 | dB | Default threshold for all bands |
| Ratio | 1:1 to 20:1 | 2:1 | :1 | Default expansion ratio |
| Attack | 0.1 to 50 | 1 | ms | Default attack time |
| Release | 10 to 2000 | 100 | ms | Default release time |
| Knee | 0 to 20 | 6 | dB | Soft knee width |
| Range | 0 to 80 | 40 | dB | Maximum attenuation depth |
| Hysteresis | 0 to 12 | 4 | dB | Open/close threshold difference |
| Hold | 0 to 500 | 10 | ms | Hold time before expanding |

### Per-Band Controls

Each band has its own expansion parameters that override the global settings when set.

**Parameters (per band):**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Threshold | -80 to 0 | (global) | dB | Band-specific threshold (overrides global) |
| Ratio | 1:1 to 20:1 | (global) | :1 | Band-specific ratio |
| Attack | 0.1 to 50 | (global) | ms | Band-specific attack |
| Release | 10 to 2000 | (global) | ms | Band-specific release |
| Knee | 0 to 20 | (global) | dB | Band-specific knee |
| Range | 0 to 80 | (global) | dB | Band-specific maximum attenuation |
| Hysteresis | 0 to 12 | (global) | dB | Band-specific hysteresis |
| Hold | 0 to 500 | (global) | ms | Band-specific hold time |
| Auto Makeup | On/Off | Off | — | Automatic makeup gain for this band |
| Active | Active/Passive | Active | — | When passive, the band passes through unexpanded |
| Solo | On/Off | Off | — | Solo this band (mutes all other bands) |
| Bypass | On/Off | Off | — | Bypass expansion for this band |

### Output

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Mix | 0 to 100 | 100 | % | Dry/wet blend between original and processed signal |
| Link Channels | On/Off | On | — | Shared detection across channels |

## Demos

### Demo: Multiband Noise Reduction

**Scenario:** A recording has different noise characteristics in different frequency ranges — low-frequency rumble and high-frequency hiss.
**Before:** Rumble below 200 Hz and hiss above 5 kHz are audible in quiet passages.
**After:** Each frequency range has targeted noise reduction without affecting the clean mid-range.
**Config:**
```json
{
  "num_bands": 3,
  "crossover_frequencies": [200.0, 5000.0],
  "threshold_db": -50.0,
  "ratio": 2.0,
  "range_db": 20.0,
  "bands": [
    {"threshold_db": -45.0, "ratio": 3.0, "range_db": 25.0},
    {"ratio": 1.0, "active": false},
    {"threshold_db": -55.0, "ratio": 2.5, "range_db": 15.0}
  ]
}
```

### Demo: Drum Overhead Tightening

**Scenario:** Drum overhead mics have too much cymbal wash between hits and low-end bleed.
**Before:** Overheads are washy with excessive cymbal sustain and kick drum bleed.
**After:** Tighter overhead sound — cymbals ring naturally during hits but decay faster, kick bleed reduced.
**Config:**
```json
{
  "num_bands": 3,
  "crossover_frequencies": [250.0, 4000.0],
  "bands": [
    {"threshold_db": -35.0, "ratio": 4.0, "attack_ms": 5.0, "release_ms": 80.0, "range_db": 30.0},
    {"ratio": 1.0},
    {"threshold_db": -40.0, "ratio": 2.0, "release_ms": 150.0, "range_db": 15.0}
  ]
}
```

### Demo: Vocal Isolation Cleanup

**Scenario:** A vocal recording has low-frequency room rumble and breath noise in the 2-4 kHz range.
**Before:** Room rumble and breathy sibilance between phrases.
**After:** Clean silence between phrases with targeted expansion per frequency range.
**Config:**
```json
{
  "num_bands": 4,
  "crossover_frequencies": [150.0, 2000.0, 8000.0],
  "bands": [
    {"threshold_db": -40.0, "ratio": 4.0, "range_db": 30.0},
    {"ratio": 1.0, "active": false},
    {"threshold_db": -45.0, "ratio": 2.5, "range_db": 20.0},
    {"ratio": 1.0, "active": false}
  ]
}
```

## Presets

### Broadband Noise Reduction
**Use case:** General-purpose noise reduction across all bands
```json
{
  "num_bands": 3,
  "crossover_frequencies": [200.0, 5000.0, 8000.0, 12000.0],
  "threshold_db": -50.0,
  "ratio": 2.0,
  "range_db": 20.0,
  "knee_db": 8.0,
  "hysteresis_db": 6.0,
  "hold_ms": 50.0,
  "link_channels": true,
  "mix": 1.0
}
```
**Tips:** Low ratio and limited range keep this transparent. Solo each band to set thresholds individually.

### Drum Tightener
**Use case:** Tighten drum mics with frequency-selective expansion
```json
{
  "num_bands": 3,
  "crossover_frequencies": [250.0, 4000.0, 8000.0, 12000.0],
  "threshold_db": -35.0,
  "ratio": 3.0,
  "range_db": 25.0,
  "knee_db": 4.0,
  "attack_ms": 1.0,
  "release_ms": 80.0,
  "hold_ms": 20.0,
  "hysteresis_db": 4.0,
  "link_channels": true,
  "mix": 1.0
}
```
**Tips:** Fast attack in the low band tightens kick bleed. Bypass mid band to preserve stick articulation.

### Vocal De-Noise
**Use case:** Targeted noise removal from vocal recordings
```json
{
  "num_bands": 4,
  "crossover_frequencies": [150.0, 2000.0, 8000.0, 12000.0],
  "threshold_db": -45.0,
  "ratio": 2.5,
  "range_db": 20.0,
  "knee_db": 6.0,
  "hold_ms": 100.0,
  "release_ms": 300.0,
  "hysteresis_db": 6.0,
  "link_channels": true,
  "mix": 1.0
}
```
**Tips:** Set mid band (2-8 kHz) to passive to preserve vocal presence. Long hold prevents cutting off word endings.

### Aggressive Multi-Gate
**Use case:** Strong per-band gating for maximum noise isolation
```json
{
  "num_bands": 4,
  "crossover_frequencies": [150.0, 2000.0, 8000.0, 12000.0],
  "threshold_db": -30.0,
  "ratio": 10.0,
  "range_db": 60.0,
  "knee_db": 2.0,
  "attack_ms": 0.5,
  "release_ms": 50.0,
  "hold_ms": 10.0,
  "hysteresis_db": 3.0,
  "link_channels": true,
  "mix": 1.0
}
```
**Tips:** High ratio with wide range approaches gate behavior. Use per-band bypass to protect important frequency ranges.

## Tips & Best Practices

- Use the **Solo** button to audition each band individually — this helps set per-band thresholds.
- Passive bands pass through unexpanded — use this to focus expansion only where noise exists.
- Hysteresis (4-6 dB) prevents chattering near the threshold per band.
- The range parameter limits maximum attenuation — set it to 15-25 dB for natural-sounding noise reduction.
- Start with global dynamics settings, then override individual bands only where needed.
- Per-band **Bypass** lets you A/B the effect of expansion on a single band.
- Crossover frequencies use smooth interpolation — you can adjust them without clicks.
- With ratio 1:1 on all bands, the crossover should reconstruct the original signal (useful for testing).

## Signal Flow

```
Input → Crossover Filters (LR4) → Band 1 → Expander → ┐
                                 → Band 2 → Expander → ├→ Sum → Mix → Output
                                 → Band 3 → Expander → ┘
                                   ...
                                 → Band N → Expander → ┘

Per-band expander:
  Band Signal → Sidechain → Level Detection → Hysteresis State Machine
                                                  ↓
                                  (Open → Hold → Closing → Open)
                                                  ↓
                                  Expansion Attenuation (ratio/knee/range)
                                                  ↓
  Band Signal → Envelope Follower (attack/release) → Gain × Auto Makeup → Output
```

Each crossover point uses two cascaded biquad filters (4th-order Linkwitz-Riley) for clean phase-coherent splitting.
