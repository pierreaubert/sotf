# Fletcher-Munson — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Fletcher-Munson" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+---------------------------------------------------------------------+
| menu Fletcher-Munson                         | menu preset | T S X  |
+---------------------------------------------------------------------+
| DESCRIPTION TEXT                                                     |
+---------------------------------------------------------------------+
| GLOBAL                                                              |
| [Reference] knob  [Smooth] knob  Volume: -20.0 dB  Delta: +6.0 dB  |
+---------------------------------------------------------------------+
| AUTO GAIN                                                           |
| [Enabled] tog  [Max Gain] knob  [AG Smooth] knob  [Type] tog       |
+---------------------------------------------------------------------+
| BAND 1 - SUB-BASS              | BAND 2 - MID-BASS                 |
| [Freq] [Q] [Max] [Slope] Curr  | [Freq] [Q] [Max] [Slope] Curr    |
+---------------------------------------------------------------------+
| BAND 3 - PRESENCE              | BAND 4 - AIR                      |
| [Freq] [Q] [Max] [Slope] Curr  | [Freq] [Q] [Max] [Slope] Curr    |
+---------------------------------------------------------------------+
```

## Description (top)

Static text: "4-band parametric EQ with gains that adjust based on playback volume, following ISO 226 equal-loudness contours."

## Global Parameters (row)

| Parameter | engine_key | Control | Param Index | Shortcut | Notes |
|-----------|-----------|---------|-------------|----------|-------|
| Reference | reference_level_db | Knob | 1 | r | -40 to 0 dB |
| Smoothing | smoothing_ms | Knob | 3 | s | 1–200 ms |
| Volume | playback_volume_db | Label (read-only) | — | — | Set by engine, displayed as info |
| Delta | — | Label (read-only) | — | — | reference - playback. Yellow if > 0, green if ≤ 0 |

Note: `playback_volume_db` (index 0) and `enabled` (index 2) exist but are set by the engine, not directly editable in this row.

## Auto Gain Section (row)

| Parameter | engine_key | Control | Param Index | Notes |
|-----------|-----------|---------|-------------|-------|
| Enabled | auto_gain_enabled | Toggle | 4 | Enable auto gain |
| Max Gain | auto_gain_max_db | Knob | 5 | 0–24 dB |
| AG Smoothing | auto_gain_smoothing_ms | Knob | 6 | 10–500 ms |
| Loudness Type | auto_gain_loudness_type | Toggle (Momentary/ShortTerm) | 7 | Choice between 400ms and 3s measurement window |

## Band Sections (2×2 grid)

Four band sections arranged in a 2×2 grid. Each band has 4 knobs and a current gain display.

### Band 1 — Sub-Bass (param offset: 8)

| Parameter | engine_key | Control | Param Index | Notes |
|-----------|-----------|---------|-------------|-------|
| Freq | band1_freq | Knob | 8 | 20–20000 Hz. Default 60 Hz |
| Q | band1_q | Knob | 9 | 0.1–10.0. Default 0.5 |
| Max Gain | band1_max_gain | Knob | 10 | 0–24 dB. Default 15 dB |
| Slope | band1_slope | Knob | 11 | 0–1.0. Default 0.6 |
| Current | — | Label (read-only) | — | Computed: min(slope × delta, max_gain). Green if > 0 |

### Band 2 — Mid-Bass (param offset: 12)

| Parameter | engine_key | Control | Param Index | Notes |
|-----------|-----------|---------|-------------|-------|
| Freq | band2_freq | Knob | 12 | Default 250 Hz |
| Q | band2_q | Knob | 13 | Default 0.707 |
| Max Gain | band2_max_gain | Knob | 14 | Default 8 dB |
| Slope | band2_slope | Knob | 15 | Default 0.4 |
| Current | — | Label (read-only) | — | Computed current gain |

### Band 3 — Presence (param offset: 16)

| Parameter | engine_key | Control | Param Index | Notes |
|-----------|-----------|---------|-------------|-------|
| Freq | band3_freq | Knob | 16 | Default 3500 Hz |
| Q | band3_q | Knob | 17 | Default 1.0 |
| Max Gain | band3_max_gain | Knob | 18 | Default 3 dB |
| Slope | band3_slope | Knob | 19 | Default 0.15 |
| Current | — | Label (read-only) | — | Computed current gain |

### Band 4 — Air (param offset: 20)

| Parameter | engine_key | Control | Param Index | Notes |
|-----------|-----------|---------|-------------|-------|
| Freq | band4_freq | Knob | 20 | Default 12000 Hz |
| Q | band4_q | Knob | 21 | Default 0.5 |
| Max Gain | band4_max_gain | Knob | 22 | Default 10 dB |
| Slope | band4_slope | Knob | 23 | Default 0.45 |
| Current | — | Label (read-only) | — | Computed current gain |

## Visualizations

### Current Gain Indicators
- **Type:** Per-band computed gain readout
- **Color:** Green when gain > 0 (active compensation), primary text when 0 (no compensation)
- **Formula:** `current_gain = min(slope × (reference - playback), max_gain)`, clamped to 0 if delta ≤ 0

## Responsive Behavior
- **Compact:** Bands stack vertically (1 column), auto gain section wraps
- **Wide:** Bands in 2×2 grid, all sections single row

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Playback Volume | playback_volume_db | Setup | Global |
| Reference Level | reference_level_db | Setup | Global |
| Enabled | enabled | Setup | Global |
| Smoothing | smoothing_ms | Setup | Global |
| Auto Gain | auto_gain_enabled | Output | Auto Gain |
| Max Correction | auto_gain_max_db | Output | Auto Gain |
| AG Smoothing | auto_gain_smoothing_ms | Output | Auto Gain |
| AG Loudness Type | auto_gain_loudness_type | Output | Auto Gain |
| Band N Freq | bandN_freq | Primary | Band N |
| Band N Q | bandN_q | Primary | Band N |
| Band N Max Gain | bandN_max_gain | Primary | Band N |
| Band N Slope | bandN_slope | Primary | Band N |
