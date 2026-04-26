# Denoiser

## Overview

Classical spectral denoiser using MCRA (Minimum Controlled Recursive Averaging) for automatic noise estimation and Wiener filtering for noise reduction. It processes audio in the STFT domain with overlap-add, and keeps broadband spectral denoising modes such as multi-resolution processing, formant preservation, harmonic/percussive weighting, manual noise profiles, psychoacoustic masking, and spectral subtraction.

RNNoise speech cleanup, focused high-frequency hiss reduction, and click/transient repair live in separate plugins.

## Features

### General

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Reduction | 0 to 40 | 10 | dB | Maximum noise reduction depth |
| Floor | -60 to -10 | -20 | dB | Residual noise floor below which reduction stops |
| Smoothing | 0 to 99 | 30 | % | Temporal smoothing of noise estimate |
| Attack | 0.1 to 100 | 5 | ms | How quickly reduction engages |
| Release | 10 to 500 | 50 | ms | How quickly reduction releases |
| Low Latency | On/Off | Off | - | Use smaller FFT for lower latency at reduced quality |
| Polyphonic | On/Off | Off | - | Protect tonal content using polyphonic pitch detection |

### MCRA Noise Estimation

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Alpha S | 0.5 to 0.99 | 0.9 | - | Signal presence probability smoothing |
| Alpha P | 0.1 to 0.99 | 0.7 | - | Noise spectrum smoothing factor |
| L | 10 to 200 | 50 | frames | Minimum statistics tracking window length |
| Delta | 1.0 to 20.0 | 5.0 | - | Speech/presence detection threshold |

### Transparency & SNR

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Transparency | 0 to 100 | 0 | % | Blend between aggressive reduction and signal preservation |
| DD SNR | On/Off | On | - | Enable decision-directed SNR estimation |
| DD Alpha | 0.9 to 0.999 | 0.98 | - | Decision-directed estimator smoothing |
| Psychoacoustic Masking | On/Off | On | - | Preserve masked noise to reduce artifacts |

### Spectral Modes

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| Spectral Smoothing | On/Off | On | Smooth gains across neighboring frequency bins |
| Temporal Smoothing | On/Off | On | Smooth gain changes between frames |
| Formant Preservation | On/Off | Off | Protect spectral peaks associated with formants |
| Formant Strength | 0 to 100 | 50 | Strength of formant peak protection |
| Multi-Resolution | On/Off | Off | Blend small and large FFT denoising paths |
| Harmonic/Percussive | On/Off | Off | Treat tonal and transient/percussive regions differently |
| Spatial Denoise | On/Off | Off | Apply multichannel spatial denoising |
| Spatial Strength | 0 to 100 | 50 | Spatial denoising intensity |

### Spectral Subtraction

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Enable | On/Off | Off | - | Enable spectral subtraction as an alternate reduction path |
| Over-subtraction | 0.5 to 6.0 | 2.0 | - | Higher values are more aggressive |
| Floor | 0.001 to 0.5 | 0.02 | - | Spectral floor to reduce musical noise |

### Noise Profile

| Parameter | Description |
|-----------|-------------|
| Learn | Start capturing a noise profile from noise-only portions |
| Use | Apply the captured noise profile instead of MCRA estimation |
| Clear | Discard the captured noise profile and revert to automatic estimation |

## Demos

### Removing Background Hum

**Scenario:** Recording has steady low-frequency hum from electrical interference.
**Config:**
```json
{
  "reduction_db": 20.0,
  "floor_db": -40.0,
  "smoothing": 70.0,
  "attack_ms": 5.0,
  "release_ms": 50.0
}
```

### Gentle Noise Reduction for Music

**Scenario:** Light noise reduction that preserves musical detail.
**Config:**
```json
{
  "reduction_db": 6.0,
  "floor_db": -50.0,
  "smoothing": 60.0,
  "transparency": 80.0,
  "psychoacoustic_masking": true,
  "formant_preservation": true
}
```

### Broadband Restoration

**Scenario:** Noisy archive material with steady broadband noise.
**Config:**
```json
{
  "reduction_db": 24.0,
  "floor_db": -32.0,
  "smoothing": 55.0,
  "spectral_smoothing_enabled": true,
  "temporal_smoothing_enabled": true,
  "multi_resolution": true
}
```

## Presets

### Gentle
```json
{
  "reduction_db": 6.0,
  "floor_db": -50.0,
  "smoothing": 60.0,
  "transparency": 80.0
}
```

### Moderate
```json
{
  "reduction_db": 12.0,
  "floor_db": -40.0,
  "smoothing": 50.0,
  "transparency": 70.0
}
```

### Aggressive
```json
{
  "reduction_db": 25.0,
  "floor_db": -30.0,
  "smoothing": 40.0,
  "transparency": 50.0,
  "spectral_smoothing_enabled": true,
  "temporal_smoothing_enabled": true
}
```

## Tips & Best Practices

- Start with low reduction and increase until noise is adequately suppressed; over-reduction causes musical-noise artifacts.
- Enable psychoacoustic masking and smoothing when using deeper reduction.
- Use captured noise profiles for stationary noise when a clean noise-only section is available.
- Use low latency mode for monitoring, and the default larger FFT for quality-focused offline or playback use.
- Use the Speech Denoiser plugin for RNNoise speech cleanup.
- Use the Hiss Reducer plugin for focused stationary high-frequency hiss.
- Use the Declick plugin for click, crackle, and time-domain transient repair.

## Signal Flow

```
Input -> STFT (Hann window, overlap-add)
  -> MCRA noise estimation (or captured profile)
  -> Wiener or spectral-subtraction gain calculation
  -> Optional: psychoacoustic masking
  -> Optional: spectral/temporal smoothing
  -> Optional: formant, multi-resolution, harmonic/percussive, spatial modes
  -> Apply gains in frequency domain
  -> ISTFT + overlap-add -> Output
```
