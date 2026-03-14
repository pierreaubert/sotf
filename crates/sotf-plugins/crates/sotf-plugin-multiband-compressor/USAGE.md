# Multiband Compressor

## Overview

A multiband dynamic range compressor that splits the audio into 2-5 frequency bands using Linkwitz-Riley crossovers, compresses each band independently, then sums them back together. Use it for mastering, taming specific frequency ranges without affecting others, or de-essing.

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

Sets default compression parameters for all bands. Individual bands can override these values.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Threshold | -60 to 0 | -20 | dB | Default threshold for all bands |
| Ratio | 1:1 to 20:1 | 4:1 | :1 | Default compression ratio |
| Attack | 0.1 to 100 | 5 | ms | Default attack time |
| Release | 10 to 1000 | 50 | ms | Default release time |
| Knee | 0 to 20 | 6 | dB | Soft knee width |

### Per-Band Controls

Each band has its own dynamics parameters that override the global settings when set.

**Parameters (per band):**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Threshold | -60 to 0 | (global) | dB | Band-specific threshold (overrides global) |
| Ratio | 1:1 to 20:1 | (global) | :1 | Band-specific ratio |
| Attack | 0.1 to 100 | (global) | ms | Band-specific attack |
| Release | 10 to 1000 | (global) | ms | Band-specific release |
| Makeup Gain | -24 to 24 | 0 | dB | Per-band output gain compensation |
| Auto Makeup | On/Off | Off | — | Automatic makeup gain for this band |
| Active | Active/Passive | Active | — | When passive, the band passes through uncompressed |
| Solo | On/Off | Off | — | Solo this band (mutes all other bands) |
| Bypass | On/Off | Off | — | Bypass compression for this band |

### Output

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Mix | 0 to 100 | 100 | % | Dry/wet blend between original and processed signal |
| Link Channels | On/Off | On | — | Shared detection across channels |

## Demos

### Demo: Mastering Compression

**Scenario:** A stereo master needs balanced dynamics across the frequency spectrum.
**Before:** Bass is inconsistent, mids are too dynamic, highs are fine.
**After:** Each frequency range has appropriate dynamics — tight bass, controlled mids, untouched highs.
**Config:**
```json
{
  "num_bands": 3,
  "crossover_frequencies": [200.0, 3000.0],
  "threshold_db": -18.0,
  "ratio": 2.0,
  "attack_ms": 20.0,
  "release_ms": 150.0,
  "bands": [
    {"threshold_db": -20.0, "ratio": 3.0, "attack_ms": 30.0},
    {"threshold_db": -16.0, "ratio": 2.0},
    {"ratio": 1.5, "active": true}
  ]
}
```

### Demo: De-essing

**Scenario:** A vocal has harsh sibilance in the 4-8 kHz range.
**Before:** "S" and "T" sounds are painfully sharp.
**After:** Sibilance is controlled while the rest of the vocal remains natural.
**Config:**
```json
{
  "num_bands": 3,
  "crossover_frequencies": [4000.0, 8000.0],
  "bands": [
    {"ratio": 1.0},
    {"threshold_db": -25.0, "ratio": 6.0, "attack_ms": 1.0, "release_ms": 30.0},
    {"ratio": 1.0}
  ]
}
```

### Demo: Bass Control

**Scenario:** A mix has inconsistent bass levels that vary between sections.
**Before:** Bass is boomy in some sections and thin in others.
**After:** Bass is consistent throughout while mids and highs are untouched.
**Config:**
```json
{
  "num_bands": 2,
  "crossover_frequencies": [150.0],
  "bands": [
    {"threshold_db": -24.0, "ratio": 4.0, "attack_ms": 10.0, "release_ms": 100.0},
    {"ratio": 1.0}
  ]
}
```

## Presets

### Mastering (Gentle)
**Use case:** Transparent mastering compression
```json
{
  "num_bands": 3,
  "crossover_frequencies": [200.0, 3000.0, 8000.0, 12000.0],
  "threshold_db": -16.0,
  "ratio": 2.0,
  "attack_ms": 20.0,
  "release_ms": 150.0,
  "knee_db": 8.0,
  "link_channels": true,
  "mix": 1.0
}
```
**Tips:** Aim for 1-3 dB of gain reduction per band. Use the solo feature to audition each band.

### De-esser
**Use case:** Control vocal sibilance
```json
{
  "num_bands": 3,
  "crossover_frequencies": [4000.0, 8000.0, 8000.0, 12000.0],
  "threshold_db": -10.0,
  "ratio": 1.0,
  "bands": [
    {"ratio": 1.0, "active": false},
    {"threshold_db": -25.0, "ratio": 6.0, "attack_ms": 0.5, "release_ms": 30.0},
    {"ratio": 1.0, "active": false}
  ],
  "mix": 1.0
}
```
**Tips:** Only the mid band (4-8 kHz) compresses. Adjust threshold to match sibilance level.

### Bass Tightener
**Use case:** Tighten and control low end
```json
{
  "num_bands": 2,
  "crossover_frequencies": [150.0, 3000.0, 8000.0, 12000.0],
  "threshold_db": -10.0,
  "ratio": 1.0,
  "bands": [
    {"threshold_db": -20.0, "ratio": 4.0, "attack_ms": 10.0, "release_ms": 80.0},
    {"ratio": 1.0, "active": false}
  ],
  "mix": 1.0
}
```
**Tips:** Faster attack (5-10 ms) tightens bass transients. Slower attack preserves punch.

### Broadcast Leveler
**Use case:** Consistent levels for broadcast/podcast
```json
{
  "num_bands": 4,
  "crossover_frequencies": [100.0, 2000.0, 8000.0, 12000.0],
  "threshold_db": -20.0,
  "ratio": 3.0,
  "attack_ms": 15.0,
  "release_ms": 100.0,
  "knee_db": 6.0,
  "link_channels": true,
  "mix": 1.0
}
```
**Tips:** Uniform settings across bands with moderate ratio gives a consistent, broadcast-ready sound.

## Tips & Best Practices

- Use the **Solo** button to audition each band individually — this helps set per-band thresholds.
- Set crossover frequencies based on the content: 100-200 Hz for bass/mids, 2-4 kHz for mids/highs, 8-12 kHz for highs/air.
- Start with global dynamics settings, then override individual bands only where needed.
- Per-band **Bypass** lets you A/B the effect of compression on a single band.
- Passive bands pass through uncompressed — use this to focus compression only where it's needed.
- Crossover frequencies use smooth interpolation — you can adjust them without clicks.
- With ratio 1:1 on all bands, the crossover should reconstruct the original signal (useful for testing).
- Link channels for stereo content to preserve the stereo image per band.

## Signal Flow

```
Input → Crossover Filters (LR4) → Band 1 → Compressor → ┐
                                 → Band 2 → Compressor → ├→ Sum → Mix → Output
                                 → Band 3 → Compressor → ┘
                                   ...
                                 → Band N → Compressor → ┘

Per-band compressor:
  Band Signal → Level Detection → Gain Calc (threshold/ratio/knee)
                                     ↓
  Band Signal → Envelope (attack/release) → Gain × Makeup → Output
```

Each crossover point uses two cascaded biquad filters (4th-order Linkwitz-Riley) for clean phase-coherent splitting.
