# Binaural Decoder

## Overview

Renders multichannel audio (stereo to 16 channels) to binaural stereo using Head-Related Transfer Functions (HRTFs) from SOFA files. Applies frequency-domain convolution with HRTF filters for each speaker position, producing a 3D spatial image over headphones. Includes LFE handling, diffuse-field equalization, room reflections, and externalization control.

## Features

### HRTF Configuration

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| SOFA File | file path | — | Path to HRTF data file (.sofa format) |
| Input Channels | 1,2,3,5,6,8,10,12,14,16 | 2 | Exact shared speaker layout; other widths are rejected |

### Spatial Processing (runtime-adjustable)

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Externalization | 0.0 to 1.0 | 0.0 | — | Blend between dry HRTF and externalized signal with room reflections |
| Near-Field Strength | 0.0 to 1.0 | 0.0 | — | Near-field head-shadowing intensity |

### Construction-Time Parameters (JSON config only)

The following parameters are set at construction time via JSON config and are not adjustable through the UI at runtime:

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Diffuse-Field EQ | On/Off | On | — | Compensate for HRTF coloration by equalizing the diffuse-field response |
| LFE Crossover | 20 to 500 | 120 | Hz | Low-pass crossover frequency for LFE channel |
| LFE Distance | 0.5 to 10.0 | 2.0 | m | Subwoofer distance for attenuation modeling |
| LFE Level | -20 to +20 | 0.0 | dB | Additional gain/attenuation for LFE channel |
| Room Model | — | default | — | Room dimensions, wall reflection coefficients, reflection count |

### Speaker Configurations

The plugin automatically selects the speaker layout based on input channel count:

| Channels | Layout |
|----------|--------|
| 2 | Stereo (L, R) |
| 3 | L, R, C |
| 5 | 5.0 (L, R, C, LS, RS) |
| 6 | 5.1 (L, R, C, LFE, LS, RS) |
| 8 | 7.1 (L, R, C, LFE, LS, RS, LB, RB) |
| 10 | shared 10-channel immersive layout |
| 12 | 7.1.4 immersive layout |
| 14 | shared 14-channel immersive layout |
| 16 | 9.1.6 immersive layout |

## Demos

### Demo: 5.1 Surround on Headphones

**Scenario:** Listening to a 5.1 surround mix on headphones with spatial imaging.
**Before:** 5.1 channels downmixed to stereo with no spatial cues.
**After:** Full surround field rendered binaurally — sounds positioned around the head.
**Config:**
```json
{
  "hrtf_file": "default.sofa",
  "input_channels": 6,
  "externalization": 0.3,
  "diffuse_field_eq": true
}
```

### Demo: Stereo Enhancement

**Scenario:** Adding subtle spatial depth to a stereo recording.
**Before:** Flat stereo image inside the head.
**After:** Wider, more externalized stereo image with front projection.
**Config:**
```json
{
  "hrtf_file": "default.sofa",
  "input_channels": 2,
  "externalization": 0.5,
  "near_field_strength": 0.2
}
```

## Presets

### Natural Binaural
**Use case:** Transparent multichannel rendering with minimal coloration
```json
{
  "input_channels": 6,
  "externalization": 0.0,
  "near_field_strength": 0.0,
  "diffuse_field_eq": true
}
```
**Tips:** Diffuse-field EQ reduces HRTF coloration. Start here and add externalization to taste.

### Externalized
**Use case:** Out-of-head perception for a more speaker-like experience
```json
{
  "input_channels": 6,
  "externalization": 0.5,
  "near_field_strength": 0.3,
  "diffuse_field_eq": true
}
```
**Tips:** Externalization adds early reflections from the room model. Higher values increase the room effect.

## Tips & Best Practices

- Load a personalized SOFA file for best spatial accuracy — generic HRTFs work but individual HRTF measurement produces dramatically better results.
- Diffuse-field EQ should usually be enabled to prevent the timbral coloration that most HRTFs introduce.
- Externalization blends in room reflections — use sparingly (0.2-0.5) for natural results; high values sound reverberant.
- The FFT size (2048 default) determines the HRTF filter resolution. Larger FFTs give better frequency resolution but add latency.
- LFE channels are automatically detected from the speaker configuration and processed separately with a low-pass crossover.
- Near-field strength models the head-shadowing effect for nearby sources — subtle values (0.1-0.3) add realism.
- Initial and explicit runtime SOFA loads run on the control thread and commit
  transactionally. Head-rotation recomputation runs on a rebound background worker.
- The Sum-Before-IFFT optimization reduces computational cost by accumulating frequency-domain contributions before a single inverse FFT per ear.
- VBAP interpolation between measured HRTF positions ensures smooth spatial transitions for arbitrary speaker angles.

## Signal Flow

```
Input (N channels) → hop partitions (zero-padded for causal linear OLA)
  → For each main channel:
      Multiply by HRTF filter (L ear + R ear)
      Accumulate into sum_left / sum_right
  → For LFE channels:
      Low-pass filter at crossover frequency
      Mix equally into both ears with distance attenuation
  → Optional: diffuse-field EQ correction
  → IFFT + overlap-add → Stereo output (L, R)
  → Optional: source-owned broadband-ILD room reflections
→ Output (2 channels)
```
