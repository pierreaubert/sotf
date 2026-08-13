# Upmixer

## Overview

A stereo-to-surround upmixer that converts 2-channel audio to multichannel surround sound (5.1, 7.1, 5.1.4, 7.1.4, 9.1.4, 9.1.6) using FFT-based Direct/Ambient decomposition and VBAP panning. Separates direct (center-focused) and ambient (diffuse) content in the frequency domain and distributes them to the appropriate speakers with decorrelation for natural envelopment.

## Features

### Speaker Configuration

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Speaker Config | 2.0/5.0/5.1/7.1/5.1.2/5.1.4/7.1.2/7.1.4/9.1.4/9.1.6 | 5.1 | — | Output speaker layout (structural — requires rebuild) |

### Gains

Controls the level of each component routed to the speaker groups.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Front Direct | 0 to 2.0 | 1.0 | x | Gain for direct (center-panned) content to front speakers |
| Front Ambient | 0 to 2.0 | 0.5 | x | Gain for ambient content to front speakers |
| Rear Ambient | 0 to 2.0 | 1.0 | x | Gain for ambient content to rear/surround speakers |
| Height Gain | 0 to 2.0 | 0.5 | x | Gain for content routed to overhead/height speakers |

### LFE & Bass

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| LFE Gain | 0 to 2.0 | 1.0 | x | Subwoofer output level |
| LFE Cutoff | 20 to 180 | 120 | Hz | Low-pass filter cutoff for the LFE channel |
| Subharmonic Synth | On/Off | Off | — | Generate sub-harmonics below LFE cutoff for deeper bass |
| Sub Gain | 0 to 1.0 | 0.5 | x | Sub-harmonic synthesis level |
| Sub Freq | 20 to 80 | 40 | Hz | Sub-harmonic center frequency |
| Sub Attack | 1 to 100 | 10 | ms | Sub-harmonic envelope attack |
| Sub Release | 10 to 500 | 50 | ms | Sub-harmonic envelope release |

### Spatial

Controls how the stereo image is distributed across the surround field.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Stereo Width | 0 to 1.0 | 0.5 | — | Spatial separation (0 = wide, 1 = narrow, 0.5 = balanced) |
| Center Spread | 0 to 1.0 | 0.0 | — | Spreads center content to L/R front speakers |
| Upmix Crossover | 150 to 350 | 250 | Hz | Frequency below which signals are treated as omnidirectional |

### Enhancement

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| HR Direct | On/Off | On | — | High-resolution FFT for sharper direct-path separation |
| HR Sharpen | 0 to 1.0 | 1.0 | — | Sharpening intensity for high-resolution direct path |
| Ambient Boost | 0.5 to 2.0 | 1.2 | x | Multiplier for ambient signal extraction |
| Safety Cap | 0 to 3 | 3 | dB | Maximum output peak level |

### Surround Routing (Advanced)

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Surround Direct Bleed | 0 to 1.0 | 0.15 | — | Direct signal leak into surround/height channels |
| Rear Ambient Boost | 1.0 to 3.0 | 1.0 | x | Extra gain for rear ambient content |
| Rear Late Reflection | 0 to 0.5 | 0.10 | — | Late reflection level for rear height channels |

### Height Channels (Advanced)

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Height HF Cap | 8000 to 20000 | 16000 | Hz | High-frequency limit for height channels |
| Height Transient Reduction | 0 to 1.0 | 0.6 | — | Reduce transients in height channels for smoother overhead sound |
| Height Direct Leak | 0 to 0.5 | 0.05 | — | Direct signal leak into height channels |

### Dialogue Detection (Advanced)

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Dialogue Weight | 0 to 1.0 | 0.4 | — | How aggressively dialogue is routed to center |
| Voice Freq Min | 200 to 800 | 500 | Hz | Lower bound of dialogue detection range |
| Voice Freq Max | 2000 to 5000 | 3000 | Hz | Upper bound of dialogue detection range |

## Demos

### Demo: Music in 5.1

**Scenario:** A stereo music track needs to be played on a 5.1 speaker system.
**Before:** Stereo audio plays only from front L/R speakers.
**After:** Full surround immersion — direct content in center/front, ambient in rears, bass in sub.
**Config:**
```json
{
  "speaker_config": "5.1",
  "gain_front_direct": 1.0,
  "gain_front_ambient": 0.5,
  "gain_rear_ambient": 1.0,
  "stereo_width": 0.5,
  "lfe_cutoff_hz": 120.0
}
```

### Demo: Film Dialogue Enhancement

**Scenario:** A stereo film soundtrack needs surround upmixing with clear dialogue in the center.
**Before:** Dialogue and effects are mixed together in L/R.
**After:** Dialogue is locked to the center channel, effects and ambience envelope the room.
**Config:**
```json
{
  "speaker_config": "5.1",
  "gain_front_direct": 1.0,
  "gain_front_ambient": 0.3,
  "gain_rear_ambient": 0.8,
  "dialogue_weight": 0.6,
  "stereo_width": 0.4
}
```

### Demo: Immersive Atmos Experience

**Scenario:** A stereo recording of a live concert needs to fill a 7.1.4 Atmos room.
**Before:** Stereo only — no height or surround presence.
**After:** Room ambience extracted to surrounds and overhead, stage presence maintained in front.
**Config:**
```json
{
  "speaker_config": "7.1.4",
  "gain_front_direct": 1.0,
  "gain_front_ambient": 0.4,
  "gain_rear_ambient": 1.2,
  "height_gain": 0.6,
  "stereo_width": 0.5,
  "enable_hr_direct": true
}
```

## Presets

### Music 5.1
**Use case:** Balanced stereo-to-5.1 upmix for music listening
```json
{
  "speaker_config": "5.1",
  "gain_front_direct": 1.0,
  "gain_front_ambient": 0.5,
  "gain_rear_ambient": 1.0,
  "height_gain": 0.5,
  "lfe_gain": 1.0,
  "lfe_cutoff_hz": 120.0,
  "stereo_width": 0.5,
  "center_spread": 0.0,
  "bandpass_hz": 250.0,
  "enable_hr_direct": true,
  "safety_cap_db": 0.0
}
```
**Tips:** Start here for most music. Increase rear ambient for more envelopment, decrease for tighter imaging.

### Cinema
**Use case:** Film/TV content with dialogue focus
```json
{
  "speaker_config": "5.1",
  "gain_front_direct": 1.0,
  "gain_front_ambient": 0.3,
  "gain_rear_ambient": 0.8,
  "dialogue_weight": 0.6,
  "stereo_width": 0.4,
  "safety_cap_db": 0.0
}
```
**Tips:** Higher dialogue weight anchors speech to center. Reduce rear ambient if surrounds are distracting.

### Immersive Atmos
**Use case:** Height-enabled systems (5.1.4, 7.1.4)
```json
{
  "speaker_config": "7.1.4",
  "gain_front_direct": 1.0,
  "gain_front_ambient": 0.4,
  "gain_rear_ambient": 1.2,
  "height_gain": 0.6,
  "height_hf_cap_hz": 16000.0,
  "height_transient_reduction": 0.6,
  "height_direct_leak": 0.05,
  "enable_hr_direct": true,
  "safety_cap_db": 0.0
}
```
**Tips:** Height channels carry mostly ambient high-frequency content. Reduce height_direct_leak to prevent voice from appearing overhead.

### Wide Ambient
**Use case:** Maximum immersive envelopment
```json
{
  "speaker_config": "5.1",
  "gain_front_direct": 0.8,
  "gain_front_ambient": 0.6,
  "gain_rear_ambient": 1.4,
  "ambient_boost": 1.5,
  "stereo_width": 0.3,
  "safety_cap_db": 0.0
}
```
**Tips:** Emphasizes ambient content for a more enveloping experience. Best for ambient/electronic music.

## Tips & Best Practices

- The upmixer adds latency equal to the FFT size (typically 2048 samples).
- **Stereo Width** is the most important tuning parameter — 0.5 is balanced, lower values spread more to surrounds.
- Use **Safety Cap** at 0 dB to prevent clipping in the output channels.
- **HR Direct** improves center channel separation quality but uses more CPU.
- Height channels should carry mostly diffuse ambient content — keep **Height Direct Leak** low (0.05-0.15).
- The **Subharmonic Synth** generates sub-bass content for the LFE — use sparingly to avoid muddiness.
- **Dialogue Weight** controls how aggressively center-panned content goes to the center speaker — increase for speech-heavy content.
- Surround speakers should be wired to match the selected speaker configuration.

## Signal Flow

```
Input (Stereo) → FFT → Frequency Domain Analysis
                            ↓
              ┌── Direct Signal (high coherence, center-panned)
              ├── Ambient Signal (low coherence, diffuse)
              └── Dialogue Detection (optional)
                            ↓
              VBAP Panning → Front L/R (direct + ambient)
                           → Center (direct + dialogue)
                           → Surround L/R (ambient + decorrelation)
                           → Rear L/R (ambient + late reflections)
                           → Height (ambient, HF-capped, transient-reduced)
                           → LFE (low-pass filtered + sub-harmonic synth)
                            ↓
              Per-channel IFFT → Overlap-Add → Safety Cap → Output
```
