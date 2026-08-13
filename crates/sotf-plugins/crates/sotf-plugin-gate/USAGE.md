# Gate

## Overview

A noise gate that attenuates audio below a threshold level. Use it to remove background noise, bleed from other instruments, or clean up recordings during silent passages. Unlike a simple on/off gate, this implements a soft gate with adjustable ratio, hold time, and sidechain filtering.

## Features

### Gating

Reduces gain when the input signal falls below the threshold. The ratio controls how aggressively the signal is attenuated — low ratios provide gentle noise reduction, high ratios approach full silence.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Threshold | -80 to 0 | -40 | dB | Level below which the gate begins attenuating |
| Ratio | 1:1 to 100:1 | 10:1 | :1 | Attenuation depth. 1:1 = no gating, 100:1 ≈ full silence below threshold |
| Knee | 0 to 20 | 0 | dB | Width of a quadratic soft-knee transition around the threshold |

### Timing

Controls the gate's response speed. Fast attack opens the gate quickly to preserve transients. Hold keeps the gate open for a set time after the signal drops below threshold (prevents chattering). Release controls how fast the gate closes.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Attack | 0.1 to 50 | 1 | ms | Time to fully open the gate when signal exceeds threshold |
| Hold | 0 to 1000 | 10 | ms | Time the gate stays open after signal drops below threshold |
| Release | 10 to 2000 | 100 | ms | Time to close the gate after hold expires |

### Sidechain & Channel Linking

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Mix | 0 to 100 | 100 | % | Dry/wet blend. Allows parallel gating |
| Link Channels | Linked/Unlinked | Linked | — | Linked: gate opens/closes for all channels together |
| Sidechain HPF | 0 to 200 | 0 | Hz | High-pass filter on detector. Prevents low rumble from holding gate open |
| HPF Order | 2nd / 4th | 2nd | — | Detector HPF slope; structural |
| Detection | Peak / RMS | Peak | — | Detector model; structural |
| External Sidechain | Off / On | Off | — | Uses a matching detector channel after each frame's programme channels; structural |
| Range | 0 to 120 | 80 | dB | Maximum attenuation; 0 means unlimited with a finite 240 dB ceiling |
| Hysteresis | 0 to 12 | 4 | dB | Difference between opening and closing thresholds |
| Lookahead | 0 to 20 | 0 | ms | Programme delay for transient detection; structural latency |

### Host and realtime contract

- Initialize before the first callback. The process context must retain that
  sample rate and the buffer must contain exactly `num_frames * input_channels()`
  interleaved samples; checked arithmetic errors are reported.
- `link_channels`, sidechain HPF frequency/order, detection mode, external
  sidechain mode, and lookahead require graph replacement. Writing the current
  value is an accepted no-op; actual live changes are rejected transactionally.
- External-sidechain samples are read-only. Non-finite programme or detector
  samples are interpreted as silence before filters, detectors, and delay state.
- Processing, realtime parameter setters, and reset allocate no memory. Reset
  deterministically clears envelopes, hold state, smoothers, detectors, filters,
  delay lines, diagnostic counters, and scratch storage.
- Monitoring snapshots are immutable after publication and update at a
  sample-derived 30 Hz cadence independent of callback partitioning.

## Demos

### Demo: Drum Gate

**Scenario:** A snare drum mic picks up hi-hat and kick bleed.
**Before:** Hi-hat and kick are audible between snare hits.
**After:** Clean snare hits with silence between — bleed is removed.
**Config:**
```json
{
  "threshold_db": -30.0,
  "ratio": 50.0,
  "attack_ms": 0.5,
  "hold_ms": 50.0,
  "release_ms": 100.0,
  "sidechain_hpf_hz": 100.0
}
```

### Demo: Vocal Noise Reduction

**Scenario:** A vocal recording has room noise and HVAC hum between phrases.
**Before:** Audible background noise during pauses.
**After:** Clean silence between vocal phrases with natural-sounding transitions.
**Config:**
```json
{
  "threshold_db": -45.0,
  "ratio": 8.0,
  "attack_ms": 2.0,
  "hold_ms": 200.0,
  "release_ms": 300.0,
  "sidechain_hpf_hz": 80.0
}
```

### Demo: Gentle Noise Floor Reduction

**Scenario:** A recording has a mild noise floor that's only noticeable in quiet sections.
**Before:** Slight hiss audible during pauses.
**After:** Noise floor is gently reduced without obvious gating artifacts.
**Config:**
```json
{
  "threshold_db": -55.0,
  "ratio": 3.0,
  "attack_ms": 5.0,
  "hold_ms": 100.0,
  "release_ms": 500.0,
  "mix": 0.7
}
```

## Presets

### Drum Gate (Tight)
**Use case:** Clean drum mic isolation
```json
{
  "threshold_db": -30.0,
  "ratio": 50.0,
  "attack_ms": 0.5,
  "hold_ms": 30.0,
  "release_ms": 80.0,
  "link_channels": true,
  "sidechain_hpf_hz": 100.0,
  "mix": 1.0
}
```
**Tips:** Adjust threshold to just above the bleed level. Short hold for tight gating.

### Vocal Gate
**Use case:** Remove background noise between vocal phrases
```json
{
  "threshold_db": -45.0,
  "ratio": 10.0,
  "attack_ms": 1.0,
  "hold_ms": 200.0,
  "release_ms": 300.0,
  "link_channels": true,
  "sidechain_hpf_hz": 80.0,
  "mix": 1.0
}
```
**Tips:** Long hold (200+ ms) prevents the gate from closing during natural pauses within phrases.

### Gentle Denoise
**Use case:** Subtle noise floor reduction
```json
{
  "threshold_db": -55.0,
  "ratio": 3.0,
  "attack_ms": 5.0,
  "hold_ms": 100.0,
  "release_ms": 500.0,
  "link_channels": true,
  "sidechain_hpf_hz": 0.0,
  "mix": 1.0
}
```
**Tips:** Low ratio (2-4) creates a gentle "duck" rather than a hard cut. Less obvious than high ratios.

### Podcast Cleanup
**Use case:** Remove background noise for podcasts and conference calls
```json
{
  "threshold_db": -40.0,
  "ratio": 15.0,
  "attack_ms": 2.0,
  "hold_ms": 300.0,
  "release_ms": 400.0,
  "link_channels": true,
  "sidechain_hpf_hz": 60.0,
  "mix": 1.0
}
```
**Tips:** Generous hold and release prevent cutting off words. Test with natural speech to verify.

## Tips & Best Practices

- Set the threshold just above the noise floor — too high and you'll cut off quiet parts of the performance.
- Use hold time (50-300 ms) to prevent the gate from chattering on sustaining notes or reverb tails.
- The sidechain HPF helps when low-frequency rumble or HVAC hum keeps the gate open falsely.
- Link channels for stereo content to prevent the gate from opening on only one side.
- A low ratio (2-4:1) creates more natural-sounding noise reduction than a hard gate (50-100:1).
- For drum gating, use very fast attack (< 1 ms) to preserve the transient.
- For speech/vocals, use longer hold and release (200-500 ms) to avoid cutting off word endings.

## Signal Flow

```
Input → Sidechain HPF → Level Detection → Threshold Comparison
                                              ↓
                              Hold Timer → Gate State (Open/Hold/Closing)
                                              ↓
Input → Envelope Follower (attack/release) → Attenuation → Mix → Output
```

The gate has three states: Open (signal above threshold), Hold (signal dropped but timer active), Closing (hold expired, gate closing at release rate).
