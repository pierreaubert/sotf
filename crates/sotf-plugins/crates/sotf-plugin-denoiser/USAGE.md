# Denoiser

## Overview

Spectral denoiser using MCRA (Minimum Controlled Recursive Averaging) for automatic noise estimation and Wiener filtering for optimal noise reduction. Processes audio in the STFT domain with overlap-add for artifact-free results. Supports both automatic and manual noise profile modes.

## Features

### General

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Reduction | 0 to 40 | 10 | dB | Maximum noise reduction depth |
| Floor | -60 to -10 | -20 | dB | Noise floor — residual noise level below which reduction stops |
| Smoothing | 0 to 99 | 30 | % | Temporal smoothing of noise estimate (higher = more stable, slower adaptation) |
| Attack | 0.1 to 100 | 5 | ms | How quickly reduction engages when noise is detected |
| Release | 10 to 500 | 50 | ms | How quickly reduction releases when signal returns |
| Low Latency | On/Off | Off | — | Use smaller FFT (512 vs 2048) for lower latency at reduced quality |

### MCRA Noise Estimation

Advanced parameters for the MCRA algorithm:

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Alpha S | 0.5 to 0.99 | 0.9 | — | Signal presence probability smoothing |
| Alpha P | 0.1 to 0.99 | 0.7 | — | Noise spectrum smoothing factor |
| Delta | 1.0 to 20.0 | 5.0 | — | Speech presence detection threshold |
| L | 10 to 200 | 50 | frames | Minimum statistics tracking window length |

### Transparency & SNR

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Transparency | 0 to 100 | 0 | % | Blend between aggressive reduction and signal preservation (0 = full denoising, 100 = pass-through) |
| DD Alpha | 0.9 to 0.999 | 0.98 | — | Decision-directed SNR estimator smoothing |
| DD Beta | 0.5 to 1.0 | 0.8 | — | Decision-directed a priori SNR weight |

### Psychoacoustic Masking

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| Enable | On/Off | On | Use psychoacoustic masking model to preserve masked noise |
| Bark Scale | On/Off | On | Use Bark-scale critical bands for masking calculation |

### Transient Detection

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| Enable | On/Off | On | Protect transients from over-reduction |
| Sensitivity | 0.5 to 5.0 | 2.0 | Transient detection sensitivity |

### Polyphonic Detection

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| Enable | On/Off | Off | Enable polyphonic pitch tracking to protect tonal content |

### Hiss Remover

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Enable | On/Off | Off | — | Enable dedicated high-frequency hiss reduction |
| Threshold | -60 to -10 | -30 | dB | Noise floor threshold for hiss detection |
| Frequency | 1000 to 16000 | 4000 | Hz | Frequency above which hiss reduction is applied |
| Strength | 0 to 100 | 50 | % | Hiss reduction intensity |

### Spectral Subtraction

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Enable | On/Off | Off | — | Enable spectral subtraction (alternative to Wiener) |
| Over-subtraction | 0.5 to 6.0 | 2.0 | — | Over-subtraction factor (higher = more aggressive) |
| Floor | 0.001 to 0.5 | 0.02 | — | Spectral floor to prevent musical noise |

### Noise Profile

| Parameter | Description |
|-----------|-------------|
| Learn | Start capturing a noise profile from silent/noise-only portions |
| Use | Apply the captured noise profile instead of MCRA estimation |
| Clear | Discard the captured noise profile and revert to automatic estimation |

## Demos

### Demo: Removing Background Hum

**Scenario:** Recording has steady low-frequency hum from electrical interference.
**Before:** Constant hum masks quiet passages and low-frequency content.
**After:** Hum removed with minimal impact on program material.
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

### Demo: Gentle Noise Reduction for Music

**Scenario:** Light noise reduction that preserves musical detail.
**Before:** Faint hiss or room noise audible during quiet passages.
**After:** Noise floor lowered without audible artifacts or loss of detail.
**Config:**
```json
{
  "reduction_db": 6.0,
  "floor_db": -50.0,
  "smoothing": 60.0,
  "transparency": 80.0,
  "psychoacoustic_masking": true,
  "transient_detection": true
}
```

### Demo: Aggressive Denoising with Hiss Remover

**Scenario:** Old recording with heavy tape hiss and broadband noise.
**Before:** Significant hiss and broadband noise obscure the recording.
**After:** Deep noise reduction with dedicated hiss remover for high frequencies.
**Config:**
```json
{
  "reduction_db": 30.0,
  "floor_db": -30.0,
  "smoothing": 50.0,
  "hiss_enabled": true,
  "hiss_frequency_hz": 4000.0,
  "hiss_strength": 0.8
}
```

## Presets

### Gentle
**Use case:** Minimal noise reduction for clean recordings with faint background noise
```json
{
  "reduction_db": 6.0,
  "floor_db": -50.0,
  "smoothing": 60.0,
  "transparency": 80.0
}
```
**Tips:** Prioritizes signal preservation over noise removal. Ideal for music mastering.

### Moderate
**Use case:** Balanced noise reduction for recordings with noticeable noise
```json
{
  "reduction_db": 12.0,
  "floor_db": -40.0,
  "smoothing": 50.0,
  "transparency": 70.0
}
```
**Tips:** Good default for most situations. Adjust reduction_db to taste.

### Aggressive
**Use case:** Heavy noise reduction for very noisy recordings
```json
{
  "reduction_db": 25.0,
  "floor_db": -30.0,
  "smoothing": 40.0,
  "transparency": 50.0
}
```
**Tips:** May introduce audible artifacts. Enable psychoacoustic masking and transient detection to minimize.

## Tips & Best Practices

- Start with low reduction and increase until noise is adequately suppressed — over-reduction causes "musical noise" artifacts.
- Enable psychoacoustic masking to let the algorithm skip noise that's already inaudible (masked by the signal).
- Transient detection protects percussive elements from being smeared by the noise gate.
- The noise profile feature gives better results than automatic MCRA for stationary noise — capture a few seconds of noise-only audio.
- Low latency mode halves the FFT size (512 vs 2048), reducing quality but suitable for monitoring.
- Smoothing controls the trade-off between stability (high) and responsiveness (low) of the noise estimate.
- Spectral subtraction is an alternative to Wiener filtering — simpler but more prone to musical noise.
- The hiss remover is a dedicated high-frequency noise reducer that complements the broadband Wiener filter.

## Signal Flow

```
Input → STFT (Hann window, 75% overlap)
  → MCRA noise estimation (or captured profile)
  → Wiener filter gain calculation
  → Optional: psychoacoustic masking adjustment
  → Optional: transient detection (bypass reduction during transients)
  → Optional: hiss remover (additional HF reduction)
  → Optional: spectral subtraction (alternative path)
  → Apply gains in frequency domain
  → ISTFT + overlap-add → Output
```
