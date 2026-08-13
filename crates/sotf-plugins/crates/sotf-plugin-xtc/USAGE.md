# XTC (Crosstalk Cancellation)

## Overview

A crosstalk cancellation plugin for stereo speaker playback. Removes the acoustic crosstalk that occurs when each ear hears both speakers, creating a binaural-like listening experience from conventional stereo speakers. Uses FFT-based inverse filtering with a physical model of head shadowing, inter-aural time differences, and optional room reflection compensation.

## Features

### Geometry

Physical setup parameters that model the listener/speaker relationship. These must match your actual listening position for effective cancellation.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Distance | 0.5 to 10 | 2.0 | m | Distance from listener to speakers |
| Speaker Angle | 10 to 90 | 30 | ° | Half-angle between speakers (standard stereo = 30°) |
| Head Radius | 5 to 12 | 8.75 | cm | Head radius for ITD/ILD calculation (display_scale: 100) |

### Head Tracking

For head-tracking-enabled setups, adjusts the cancellation filters in real-time based on head position and orientation.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Head Offset X | -0.5 to 0.5 | 0 | m | Lateral head offset from sweet spot |
| Head Offset Z | -0.5 to 0.5 | 0 | m | Depth offset from sweet spot |
| Head Yaw | -90 to 90 | 0 | ° | Head rotation angle |
| Head Tracking Smooth | 0 to 1.0 | 0.1 | s | Smoothing time for head tracking updates |

### Filter Tuning

Controls the cancellation filter's behavior — the trade-off between cancellation depth and stability.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Beta Base | 0.1 to 100 | 1.0 | ×1000 | Base regularization (display_scale: 1000). Higher = more stable, less cancellation |
| Beta Low Boost | 0 to 30 | 10 | — | Extra regularization at low frequencies (<100 Hz) |
| Beta High Boost | 0 to 30 | 10 | — | Extra regularization at high frequencies (>12 kHz) |
| Max Gain | 3 to 30 | 12 | dB | Maximum filter boost per frequency bin |

### Head Shadowing

Models how the head attenuates high frequencies for the far-side ear.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Shadow Cutoff | 1000 to 10000 | 4000 | Hz | Frequency where head shadowing begins |
| Shadow Slope | 0 to 12 | 6 | dB/oct | Attenuation rate above the cutoff |

### Advanced

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Spectral Norm | On/Off | On | — | Normalize per-bin energy to reduce tonal coloration |
| Pinna Model | On/Off | Off | — | Apply ear canal/concha resonances (adds +10-12 dB at 2.7-4.5 kHz) |

### Room Reflections

Optional room model for compensating early reflections that degrade cancellation.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Room Reflections | On/Off | Off | — | Enable room reflection compensation |
| Room Width | 2 to 10 | 4.0 | m | Room width (X axis) for image-source model |
| Room Depth | 2 to 15 | 5.0 | m | Room depth (Z axis, listening axis) |
| Wall Absorption | 0 to 1.0 | 0.3 | — | Wall absorption coefficient (0 = reflective, 1 = absorptive) |
| Reflection Beta | 1 to 10 | 3.0 | — | Extra regularization at comb-filter null frequencies |

### Auto Gain

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Auto Gain | On/Off | On | — | Match output loudness to input loudness |
| AG Max | 0 to 24 | 12 | dB | Maximum auto-gain compensation |
| AG Smoothing | 10 to 500 | 100 | ms | Smoothing time for gain transitions |

### Diagnostic Bypasses

For troubleshooting audio artifacts — each bypass isolates a processing stage.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Bypass XTC Filters | On/Off | Off | — | Bypass cancellation (tests STFT framework only) |
| Bypass Spectral Norm | On/Off | Off | — | Bypass spectral normalization only |
| Bypass Neumann | On/Off | Off | — | Bypass Neumann series refinement |

## Demos

### Demo: Desktop Speaker Setup

**Scenario:** Near-field desktop speakers at 30° angles, 1.5m distance.
**Before:** Standard stereo with heavy crosstalk — no spatial depth.
**After:** Binaural-like imaging with clear left/right separation and depth.
**Config:**
```json
{
  "distance_m": 1.5,
  "speaker_angle_deg": 30.0,
  "head_radius_m": 0.0875,
  "beta_base": 0.01,
  "max_gain_db": 12.0,
  "auto_gain_enabled": true
}
```

### Demo: Wide Speaker Setup

**Scenario:** Living room speakers at 45° angles, 3m distance.
**Before:** Wider stereo but still audible crosstalk.
**After:** Enhanced imaging with stable cancellation for the wider angle.
**Config:**
```json
{
  "distance_m": 3.0,
  "speaker_angle_deg": 45.0,
  "beta_base": 0.02,
  "max_gain_db": 10.0,
  "auto_gain_enabled": true
}
```

### Demo: Room-Compensated XTC

**Scenario:** A reflective listening room degrades cancellation from early reflections.
**Before:** Cancellation is unstable with visible comb-filter artifacts.
**After:** Room model compensates for first-order reflections, improving stability.
**Config:**
```json
{
  "distance_m": 2.0,
  "speaker_angle_deg": 30.0,
  "room_reflections_enabled": true,
  "room_width_m": 4.0,
  "room_depth_m": 5.0,
  "wall_absorption": 0.3,
  "beta_base": 0.01,
  "auto_gain_enabled": true
}
```

## Presets

### Standard Desktop
**Use case:** Near-field desktop monitoring
```json
{
  "distance_m": 1.5,
  "speaker_angle_deg": 30.0,
  "head_radius_m": 0.0875,
  "beta_base": 0.01,
  "beta_low_freq_boost": 10.0,
  "beta_high_freq_boost": 10.0,
  "max_gain_db": 12.0,
  "head_shadow_cutoff_hz": 4000.0,
  "head_shadow_slope_db_per_octave": 6.0,
  "spectral_normalization": true,
  "auto_gain_enabled": true
}
```
**Tips:** Adjust distance and angle to match your setup. Start with spectral normalization on.

### Conservative
**Use case:** Subtle cancellation with maximum stability
```json
{
  "distance_m": 2.0,
  "speaker_angle_deg": 30.0,
  "beta_base": 0.05,
  "max_gain_db": 6.0,
  "spectral_normalization": true,
  "auto_gain_enabled": true
}
```
**Tips:** Higher beta and lower max gain reduce cancellation depth but eliminate artifacts. Good starting point for untreated rooms.

### Aggressive
**Use case:** Maximum cancellation depth for treated rooms
```json
{
  "distance_m": 2.0,
  "speaker_angle_deg": 30.0,
  "beta_base": 0.003,
  "max_gain_db": 18.0,
  "spectral_normalization": true,
  "pinna_model_enabled": true,
  "auto_gain_enabled": true
}
```
**Tips:** Low beta and high max gain give deep cancellation but require a quiet, treated room. Enable pinna model for more realistic HRTF response.

### With Room Compensation
**Use case:** Reflective rooms where standard XTC doesn't work well
```json
{
  "distance_m": 2.0,
  "speaker_angle_deg": 30.0,
  "beta_base": 0.01,
  "max_gain_db": 12.0,
  "room_reflections_enabled": true,
  "room_width_m": 4.0,
  "room_depth_m": 5.0,
  "wall_absorption": 0.3,
  "reflection_beta_boost": 3.0,
  "auto_gain_enabled": true
}
```
**Tips:** Measure your room dimensions for best results. Higher wall_absorption = less reflections to compensate for.

## Tips & Best Practices

- XTC is for speaker listening only — do not use with headphones (use Crossfeed instead).
- The listener must be in the sweet spot for effective cancellation — head tracking can help.
- Start with the **Conservative** preset and increase cancellation depth gradually.
- **Spectral normalization** reduces tonal coloration at the cost of some cancellation depth — leave it on unless you've verified it's unnecessary.
- Higher **Beta Base** values are more stable but cancel less crosstalk.
- **Max Gain** limits how much any frequency can be boosted — lower values prevent ringing artifacts.
- Room reflections degrade cancellation — use the room model or acoustic treatment.
- The plugin adds latency equal to the FFT size (default 2048 samples).
- Use diagnostic bypasses to isolate which processing stage causes any artifacts.

## Signal Flow

Source/artifact and FFT-size changes are structural. Rebuild the plugin graph
after changing them; runtime automation is limited to parameters that preserve
the compiled output layout.

```
Input (Stereo) → STFT (windowed FFT, 75% overlap)
                    ↓
              Physical Model:
                - Ipsilateral path (direct: speaker → near ear)
                - Contralateral path (crosstalk: speaker → far ear)
                - Head shadowing filter
                - Optional room reflections
                    ↓
              Inverse Filter Matrix:
                - Regularized inverse (β controls stability)
                - Neumann series refinement
                - Gain capping (max_gain_db)
                    ↓
              Apply Filter → Spectral Normalization (optional)
                    ↓
              ISTFT → Overlap-Add → Auto Gain Compensation → Output (Stereo)
```
