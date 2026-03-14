# Downmix

## Overview

A phase-coherent surround-to-stereo downmixer that converts multichannel audio (5.1, 7.1, Atmos, etc.) to stereo. Uses speaker geometry to compute constant-power panning coefficients, with optional FFT-based phase alignment to reduce comb-filtering artifacts. Supports any speaker configuration with automatic coefficient calculation.

## Features

### Channel Mixing

Converts multichannel audio to stereo using speaker-aware panning coefficients. Each input channel is mapped to left and right outputs based on its speaker position (azimuth and elevation). Front L/R pass through directly, center is split equally, surrounds and heights use constant-power panning.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Center Gain | -24 to 12 | -3 | dB | Gain applied to center channel before mixing |
| Surround Gain | -24 to 12 | -3 | dB | Gain applied to surround channels (side + rear) |
| Height Gain | -24 to 12 | -6 | dB | Gain applied to height/Atmos channels |
| LFE Gain | -24 to 12 | -10 | dB | Gain applied to LFE/subwoofer channel |

### Phase Coherence

Optional FFT-based phase alignment that reduces comb-filtering artifacts when multiple channels are summed. Works by aligning bin phases to the dominant source per frequency band, with a smooth crossover between direct mixing (low frequencies) and phase-aligned mixing (high frequencies).

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Phase Coherence | On/Off | On | — | Enable FFT-based phase alignment |
| Phase Blend Low | 50 to 500 | 200 | Hz | Below this frequency, use direct mixing (no phase alignment) |
| Phase Blend High | 1000 to 10000 | 5000 | Hz | Above this frequency, use full phase alignment |

## Demos

### Demo: Standard 5.1 Downmix

**Scenario:** A 5.1 surround mix needs to be converted to stereo for headphone listening.
**Before:** 6-channel surround audio that can't play on stereo headphones.
**After:** Clean stereo downmix with proper center fold-down and surround blending.
**Config:**
```json
{
  "input_channels": 6,
  "center_gain_db": -3.0,
  "surround_gain_db": -3.0,
  "lfe_gain_db": -10.0,
  "phase_coherence": false
}
```

### Demo: Atmos to Stereo with Phase Alignment

**Scenario:** A 7.1.4 Atmos mix with height channels needs stereo conversion without comb-filtering artifacts.
**Before:** Direct summing causes phase cancellation at certain frequencies, making the mix thin.
**After:** Phase-coherent downmix preserves fullness and spatial cues.
**Config:**
```json
{
  "input_channels": 12,
  "center_gain_db": -3.0,
  "surround_gain_db": -3.0,
  "height_gain_db": -6.0,
  "lfe_gain_db": -10.0,
  "phase_coherence": true,
  "phase_blend_low_hz": 200.0,
  "phase_blend_high_hz": 5000.0
}
```

### Demo: Cinema-Style Downmix

**Scenario:** A film 5.1 mix needs to preserve dialogue clarity while maintaining surround atmosphere.
**Before:** Surrounds are too quiet or dialogue is buried.
**After:** Dialogue-forward mix with subtle surround ambience.
**Config:**
```json
{
  "input_channels": 6,
  "center_gain_db": 0.0,
  "surround_gain_db": -6.0,
  "lfe_gain_db": -12.0,
  "phase_coherence": true
}
```

## Presets

### Standard Downmix
**Use case:** Balanced surround-to-stereo conversion
```json
{
  "center_gain_db": -3.0,
  "surround_gain_db": -3.0,
  "height_gain_db": -6.0,
  "lfe_gain_db": -10.0,
  "phase_coherence": false
}
```
**Tips:** Works well for most content. No latency since phase coherence is off.

### Phase-Coherent
**Use case:** High-quality downmix that avoids comb-filtering
```json
{
  "center_gain_db": -3.0,
  "surround_gain_db": -3.0,
  "height_gain_db": -6.0,
  "lfe_gain_db": -10.0,
  "phase_coherence": true,
  "phase_blend_low_hz": 200.0,
  "phase_blend_high_hz": 5000.0
}
```
**Tips:** Adds ~2048 samples of latency. Best for offline or non-real-time playback.

### Dialogue-Forward
**Use case:** Preserve speech intelligibility from center channel
```json
{
  "center_gain_db": 0.0,
  "surround_gain_db": -6.0,
  "height_gain_db": -9.0,
  "lfe_gain_db": -12.0,
  "phase_coherence": true
}
```
**Tips:** Boosts center (dialogue) relative to surrounds. Good for film and TV content.

### Surround-Heavy
**Use case:** Maximize the surround experience in stereo
```json
{
  "center_gain_db": -3.0,
  "surround_gain_db": 0.0,
  "height_gain_db": -3.0,
  "lfe_gain_db": -6.0,
  "phase_coherence": true
}
```
**Tips:** Brings surrounds up to full level. Good for music and immersive content.

## Tips & Best Practices

- Phase coherence adds latency (2048 samples) — disable for live monitoring.
- LFE is low-pass filtered at 120 Hz and split equally to L/R at -3 dB.
- The plugin automatically detects speaker configurations (5.1, 7.1, 5.1.4, etc.) from channel count.
- Normalization prevents clipping when many channels sum together — gain sum is capped at 2.0.
- Constant-power panning ensures no energy loss for surround and height channels.
- For unknown channel counts without a speaker config, channels are linearly panned across the stereo field.

## Signal Flow

```
Simple mode (phase coherence off):
  Input (N channels) → Per-channel gain × panning coefficients → Sum to L/R → Output (stereo)

Phase-coherent mode:
  Input (N channels) → FFT per channel → Frequency domain mixing with phase alignment
                                              ↓
                        Per-bin: standard sum + phase-aligned sum, blended by frequency
                                              ↓
                        IFFT → Overlap-Add (75% overlap, Hann window) → Output (stereo)
```
