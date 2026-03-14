# XTC — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "XTC" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+----------------------------------------------------------------------+
| SETUP       | BETA        | SHADOW      | ADVANCED    | AUTO GAIN   |
| Distance    | Beta Base   | Cutoff      | Spectral N  | AG Enabled  |
| Spkr Angle  | Beta LF     | Slope       | Pinna Model | AG Max      |
| Head Radius | Beta HF     | Max Gain    |             | AG Smooth   |
+------+------+------+------+------+------+------+-----+--------------+
| Head Track  | Room  | Diag  |                                        |
+------+------+------+------+-----------------------------------------+
| Tab content                                                          |
+----------------------------------------------------------------------+
```

## Main (always visible, 5 columns)

### SETUP column

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Distance | distance_m | Knob | d | 0.5 to 10 m |
| Speaker Angle | speaker_angle_deg | Knob | — | 10 to 90° |
| Head Radius | head_radius_m | Knob | — | 5 to 12 cm (display_scale: 100) |

### BETA column

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Beta Base | beta_base | Knob | — | 0.1 to 100 ×1000 (display_scale: 1000) |
| Beta Low Boost | beta_low_freq_boost | Knob | — | 0 to 30 |
| Beta High Boost | beta_high_freq_boost | Knob | — | 0 to 30 |

### SHADOW column

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Shadow Cutoff | head_shadow_cutoff_hz | Knob | — | 1000 to 10000 Hz |
| Shadow Slope | head_shadow_slope_db_per_octave | Knob | — | 0 to 12 dB/oct |
| Max Gain | max_gain_db | Knob | — | 3 to 30 dB |

### ADVANCED column

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Spectral Norm | spectral_normalization | Toggle | Reduce tonal coloration |
| Pinna Model | pinna_model_enabled | Toggle | Ear canal/concha resonances |

### AUTO GAIN column

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Auto Gain | auto_gain_enabled | Toggle | Match output loudness to input |
| AG Max | auto_gain_max_db | Knob | 0 to 24 dB |
| AG Smoothing | auto_gain_smoothing_ms | Knob | 10 to 500 ms |

## Tabs (Bottom)

Tab bar: `Head Tracking | Room | Diagnostic`

### Tab: Head Tracking

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Head Offset X | head_offset_x | Knob | -0.5 to 0.5 m |
| Head Offset Z | head_offset_z | Knob | -0.5 to 0.5 m |
| Head Yaw | head_yaw_deg | Knob | -90 to 90° |
| Tracking Smooth | head_tracking_smooth_s | Knob | 0 to 1.0 s |

### Tab: Room

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Room Reflections | room_reflections_enabled | Toggle | Enable room compensation |
| Room Width | room_width_m | Knob | 2 to 10 m |
| Room Depth | room_depth_m | Knob | 2 to 15 m |
| Wall Absorption | wall_absorption | Knob | 0 to 1.0 |
| Reflection Beta | reflection_beta_boost | Knob | 1 to 10 |

### Tab: Diagnostic

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Bypass XTC Filters | bypass_xtc_filters | Toggle | Tests STFT framework only |
| Bypass Spectral Norm | bypass_spectral_normalization | Toggle | Tests normalization |
| Bypass Neumann | bypass_neumann_refinement | Toggle | Tests refinement step |

## Responsive Behavior
- **Compact:** Main columns become 2-row layout (Setup+Beta top, Shadow+Advanced+AutoGain bottom). Tabs collapsed.
- **Wide:** All 5 columns visible in single row, tab content expanded.

## ParamCategory Mapping

| Parameter | Category | Group |
|-----------|----------|-------|
| Distance, Speaker Angle, Head Radius | Setup | Geometry |
| Head Offset X/Z, Head Yaw, Tracking Smooth | Setup | Head Tracking |
| Beta Base, Beta Low/High Boost | Primary | Beta |
| Shadow Cutoff, Shadow Slope, Max Gain | Primary | Shadow |
| Spectral Norm, Pinna Model | Secondary | Advanced |
| Room Reflections, Room Width/Depth, Wall Absorption, Reflection Beta | Secondary | Room |
| Bypass XTC/Spectral/Neumann | Diagnostic | Diagnostic |
| Auto Gain, AG Max, AG Smoothing | Output | Auto Gain |
