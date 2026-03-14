# Compressor

## Overview

A dynamic range compressor that reduces the volume of loud signals above a threshold. Use it to control peaks, add sustain, glue a mix together, or achieve parallel compression via the mix control.

## Features

### Dynamic Range Compression

Reduces gain when the input signal exceeds the threshold. The amount of reduction is controlled by the ratio — higher ratios mean more aggressive compression.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Threshold | -60 to 0 | -20 | dB | Level above which compression begins |
| Ratio | 1:1 to 20:1 | 4:1 | :1 | How much the signal is reduced. 1:1 = no compression, 20:1 ≈ limiting |
| Knee | 0 to 20 | 6 | dB | Width of the soft knee. 0 = hard knee (abrupt onset), higher = smoother transition |

### Timing

Controls how fast the compressor reacts to level changes. Fast attack catches transients; slow attack lets them through. Short release recovers quickly but can pump; long release is smoother.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Attack | 0.1 to 100 | 5 | ms | Time to reach full compression after signal exceeds threshold |
| Release | 10 to 1000 | 50 | ms | Time to return to unity gain after signal drops below threshold |

### Output & Makeup Gain

Compression reduces overall level. Makeup gain compensates. The mix control enables parallel compression (blending dry + compressed signal).

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Makeup Gain | -24 to 24 | 0 | dB | Output gain to compensate for volume reduction |
| Auto Makeup | On/Off | Off | — | Automatically estimates and applies makeup gain based on threshold and ratio |
| Mix | 0 to 100 | 100 | % | Dry/wet blend. 0% = unprocessed, 100% = fully compressed |

### Sidechain & Channel Linking

Controls how the compressor detects levels. Linking uses the loudest channel's level for all channels (preserves stereo image). The sidechain HPF prevents low frequencies from triggering compression.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Link Channels | Linked/Unlinked | Linked | — | Linked: shared detector across channels. Unlinked: independent per-channel compression |
| Sidechain HPF | 0 to 200 | 80 | Hz | High-pass filter on the detection sidechain. Prevents bass from triggering compression |

## Demos

### Demo: Taming Vocal Peaks

**Scenario:** A vocal recording with inconsistent dynamics — some phrases are too quiet, others clip.
**Before:** Vocal level varies by 15+ dB between soft and loud passages.
**After:** Dynamic range reduced to ~6 dB with natural-sounding transitions. Soft parts are more audible, loud parts are controlled.
**Config:**
```json
{
  "threshold_db": -18.0,
  "ratio": 3.0,
  "attack_ms": 10.0,
  "release_ms": 80.0,
  "knee_db": 6.0,
  "auto_makeup": true
}
```

### Demo: Parallel Drum Compression

**Scenario:** Drum bus needs more punch and sustain without losing transient attack.
**Before:** Drums have good transients but lack body and sustain.
**After:** Heavily compressed signal blended at 30% adds body while the dry signal preserves transient snap.
**Config:**
```json
{
  "threshold_db": -30.0,
  "ratio": 8.0,
  "attack_ms": 1.0,
  "release_ms": 40.0,
  "knee_db": 0.0,
  "mix": 0.3,
  "auto_makeup": true,
  "sidechain_hpf_hz": 100.0
}
```

### Demo: Bus Glue

**Scenario:** A stereo mix or subgroup where instruments need to feel cohesive.
**Before:** Individual instruments are well-balanced but sound separate.
**After:** Gentle compression "glues" the elements together with a unified dynamic feel.
**Config:**
```json
{
  "threshold_db": -12.0,
  "ratio": 2.0,
  "attack_ms": 30.0,
  "release_ms": 200.0,
  "knee_db": 10.0,
  "makeup_gain_db": 1.0,
  "link_channels": true
}
```

## Presets

### Gentle Vocal
**Use case:** Transparent vocal leveling
```json
{
  "threshold_db": -18.0,
  "ratio": 2.5,
  "attack_ms": 15.0,
  "release_ms": 100.0,
  "knee_db": 8.0,
  "auto_makeup": true,
  "link_channels": true,
  "sidechain_hpf_hz": 80.0,
  "mix": 1.0
}
```
**Tips:** Lower threshold for more consistent leveling. Increase ratio if peaks still poke through.

### Drum Bus Punch
**Use case:** Add punch and sustain to drums
```json
{
  "threshold_db": -24.0,
  "ratio": 4.0,
  "attack_ms": 5.0,
  "release_ms": 40.0,
  "knee_db": 3.0,
  "auto_makeup": true,
  "link_channels": true,
  "sidechain_hpf_hz": 100.0,
  "mix": 1.0
}
```
**Tips:** Increase attack to 15-25 ms to let transients through. Use mix at 50% for parallel compression.

### Bus Glue
**Use case:** Gently compress a stereo mix or subgroup
```json
{
  "threshold_db": -14.0,
  "ratio": 2.0,
  "attack_ms": 25.0,
  "release_ms": 150.0,
  "knee_db": 10.0,
  "makeup_gain_db": 1.0,
  "link_channels": true,
  "sidechain_hpf_hz": 60.0,
  "mix": 1.0
}
```
**Tips:** Aim for 1-3 dB of gain reduction at most. The compressor should be barely noticeable.

### Heavy Squeeze
**Use case:** Aggressive compression for effect (e.g., room mics, parallel chains)
```json
{
  "threshold_db": -35.0,
  "ratio": 10.0,
  "attack_ms": 0.5,
  "release_ms": 30.0,
  "knee_db": 0.0,
  "auto_makeup": true,
  "link_channels": true,
  "sidechain_hpf_hz": 120.0,
  "mix": 0.4
}
```
**Tips:** This preset is designed for parallel use (mix < 100%). Blend to taste — a little goes a long way.

### Podcast / Speech
**Use case:** Consistent speech levels for podcasts and voice-over
```json
{
  "threshold_db": -20.0,
  "ratio": 3.0,
  "attack_ms": 10.0,
  "release_ms": 120.0,
  "knee_db": 6.0,
  "auto_makeup": true,
  "link_channels": true,
  "sidechain_hpf_hz": 80.0,
  "mix": 1.0
}
```
**Tips:** Use with a limiter after to catch remaining peaks.

## Tips & Best Practices

- Start with a moderate ratio (2:1 to 4:1) and lower the threshold until you see 3-6 dB of gain reduction.
- Use the sidechain HPF (80-120 Hz) on full-range material to prevent bass energy from causing pumping.
- Link channels for stereo content to preserve the stereo image. Unlink only for independent channel processing.
- For transparent compression, use a soft knee (6-10 dB) and moderate attack/release times.
- For parallel compression, set a high ratio with fast attack, then reduce mix to 20-40%.
- Watch the gain reduction meter — if it never returns to 0 dB, your threshold is too low or release too slow.

## Signal Flow

```
Input → Sidechain HPF → Level Detection → Gain Calculation (threshold/ratio/knee)
                                              ↓
Input → Envelope Follower (attack/release) → Gain Reduction → Makeup Gain → Mix → Output
```

The sidechain path is separate from the audio path. The HPF only affects level detection, not the audio signal itself. When channels are linked, the maximum detected level across all channels drives all gain reduction.
