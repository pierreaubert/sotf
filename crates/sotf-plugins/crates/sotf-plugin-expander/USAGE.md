# Expander

## Overview

A downward expander that increases the dynamic range below a threshold. Use it for noise reduction, tightening up recordings, or reducing bleed — similar to a gate but with smoother, more musical behavior. Features hysteresis for chatter-free operation and a configurable maximum attenuation range.

## Features

### Expansion

Attenuates signals that fall below the threshold. Unlike a gate (which simply cuts), an expander increases the dynamic range proportionally. A 2:1 ratio means signals 10 dB below threshold are pushed to 20 dB below.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Threshold | -80 to 0 | -40 | dB | Level below which expansion begins |
| Ratio | 1:1 to 20:1 | 2:1 | :1 | Expansion ratio. 1:1 = off, higher = more aggressive |
| Range | 0 to 80 | 40 | dB | Maximum attenuation depth. Limits how quiet the signal can get |
| Knee | 0 to 20 | 6 | dB | Soft knee width for smoother transition around threshold |

### Timing

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Attack | 0.1 to 50 | 1 | ms | Time to reach full expansion when signal drops below threshold |
| Release | 10 to 2000 | 100 | ms | Time to return to unity when signal rises above threshold |
| Hold | 0 to 500 | 10 | ms | Time to hold before expanding. Prevents chattering |

### Hysteresis

Prevents the expander from chattering near the threshold by using different thresholds for opening and closing.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Hysteresis | 0 to 12 | 4 | dB | Difference between open and close thresholds. Signal must rise to threshold to open, but must fall to threshold-hysteresis to close |

### Output & Sidechain

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Mix | 0 to 100 | 100 | % | Dry/wet blend. Allows parallel expansion |
| Auto Makeup | On/Off | Off | — | Compensate for average attenuation |
| Link Channels | Linked/Unlinked | Linked | — | Shared or independent per-channel detection |
| Sidechain HPF | 0 to 500 | 80 | Hz | High-pass filter on detector sidechain |

## Demos

### Demo: Transparent Noise Reduction

**Scenario:** A high-quality recording has a faint noise floor that's only noticeable in very quiet passages.
**Before:** Subtle hiss audible during pianissimo sections.
**After:** Noise floor gradually pushed down without audible gating artifacts.
**Config:**
```json
{
  "threshold_db": -50.0,
  "ratio": 2.0,
  "range_db": 20.0,
  "knee_db": 8.0,
  "attack_ms": 5.0,
  "release_ms": 200.0,
  "hold_ms": 50.0,
  "hysteresis_db": 6.0
}
```

### Demo: Drum Tightening

**Scenario:** Drum overheads need tighter dynamics — less bleed between hits.
**Before:** Open overheads with significant cymbal wash and room sound between drum hits.
**After:** Tighter overhead sound with reduced bleed, but smoother than a hard gate.
**Config:**
```json
{
  "threshold_db": -35.0,
  "ratio": 4.0,
  "range_db": 30.0,
  "knee_db": 4.0,
  "attack_ms": 1.0,
  "release_ms": 80.0,
  "hold_ms": 20.0,
  "hysteresis_db": 4.0,
  "sidechain_hpf_hz": 120.0
}
```

### Demo: Vocal Cleanup

**Scenario:** A vocal recording has room noise and breath sounds that need taming.
**Before:** Room ambience and breaths between phrases.
**After:** Clean breaks between phrases with gentle, natural-sounding transitions.
**Config:**
```json
{
  "threshold_db": -42.0,
  "ratio": 3.0,
  "range_db": 25.0,
  "knee_db": 10.0,
  "attack_ms": 3.0,
  "release_ms": 300.0,
  "hold_ms": 100.0,
  "hysteresis_db": 6.0,
  "sidechain_hpf_hz": 80.0
}
```

## Presets

### Gentle Expansion
**Use case:** Subtle noise reduction for clean recordings
```json
{
  "threshold_db": -50.0,
  "ratio": 2.0,
  "range_db": 15.0,
  "knee_db": 10.0,
  "attack_ms": 5.0,
  "release_ms": 200.0,
  "hold_ms": 50.0,
  "hysteresis_db": 6.0,
  "mix": 1.0,
  "link_channels": true,
  "sidechain_hpf_hz": 80.0,
  "auto_makeup": false
}
```
**Tips:** Low ratio and wide knee make this nearly transparent. Increase range for more noise reduction.

### Drum Tightener
**Use case:** Tighten drum mics without hard gating
```json
{
  "threshold_db": -35.0,
  "ratio": 4.0,
  "range_db": 30.0,
  "knee_db": 4.0,
  "attack_ms": 1.0,
  "release_ms": 80.0,
  "hold_ms": 20.0,
  "hysteresis_db": 4.0,
  "mix": 1.0,
  "link_channels": true,
  "sidechain_hpf_hz": 120.0,
  "auto_makeup": false
}
```
**Tips:** Use sidechain HPF to prevent kick drum from triggering expansion on snare mics.

### Vocal Cleanup
**Use case:** Remove room noise between vocal phrases
```json
{
  "threshold_db": -42.0,
  "ratio": 3.0,
  "range_db": 25.0,
  "knee_db": 8.0,
  "attack_ms": 3.0,
  "release_ms": 300.0,
  "hold_ms": 100.0,
  "hysteresis_db": 6.0,
  "mix": 1.0,
  "link_channels": true,
  "sidechain_hpf_hz": 80.0,
  "auto_makeup": false
}
```
**Tips:** Long hold and release prevent cutting off word endings and reverb tails.

### Aggressive Expansion
**Use case:** Strong noise reduction or near-gate behavior
```json
{
  "threshold_db": -30.0,
  "ratio": 10.0,
  "range_db": 60.0,
  "knee_db": 2.0,
  "attack_ms": 0.5,
  "release_ms": 50.0,
  "hold_ms": 10.0,
  "hysteresis_db": 3.0,
  "mix": 1.0,
  "link_channels": true,
  "sidechain_hpf_hz": 100.0,
  "auto_makeup": false
}
```
**Tips:** High ratio with wide range approaches gate behavior but with smoother transitions.

## Tips & Best Practices

- Expanders are more musical than gates — start with an expander and only switch to a gate if you need harder cuts.
- Hysteresis (4-6 dB) prevents chattering near the threshold. Increase for noisy or reverberant sources.
- The range parameter limits maximum attenuation — set it to 15-25 dB for natural-sounding noise reduction.
- Use a wide knee (6-10 dB) for transparent operation on dynamic material.
- The sidechain HPF prevents low-frequency rumble from keeping the expander open.
- Auto makeup compensates for the average attenuation — useful when expansion significantly reduces overall level.
- The expander uses a three-state model (Open/Hold/Closing) with hysteresis for stable, chatter-free operation.

## Signal Flow

```
Input → Sidechain HPF → Level Detection → Hysteresis State Machine
                                              ↓
                              (Open → Hold → Closing → Open)
                                              ↓
                              Expansion Attenuation (ratio/knee/range)
                                              ↓
Input → Envelope Follower (attack/release) → Gain × Auto Makeup → Mix → Output
```
