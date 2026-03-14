# Loudness Monitor — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Loudness" label | Plugin name |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+---------------------------------------------------------------------+
| menu Loudness                                            | T S X    |
+---------------------------------------------------------------------+
| LUFS METERS                     | TRUE PEAK          | CORRELATION  |
|                                 |                    |              |
| Momentary  [-14.2 LUFS] ▓▓▓▓▓▓ | L  [-0.3 dBTP]    | L/R  [+0.85] |
| Short-term [-13.8 LUFS] ▓▓▓▓▓▓ | R  [-0.5 dBTP]    |   ◄────►     |
| Integrated [-14.0 LUFS] ▓▓▓▓▓▓ | C  [-1.2 dBTP]    | Mono ↔ Wide  |
|                                 | ...                |              |
+---------------------------------------------------------------------+
```

## Main — LUFS Meters (left, flex)

### Loudness Readings
- **Type:** Horizontal bar meters with numeric readout
- **Data source:** RealTimeCache<LoudnessData> via `get_data()`

| Measurement | Display | Color | Notes |
|-------------|---------|-------|-------|
| Momentary LUFS | Bar + value | Green/yellow/red by level | 400 ms window, fast-responding |
| Short-term LUFS | Bar + value | Green/yellow/red by level | 3 s window |
| Integrated LUFS | Bar + value | Primary text color | Cumulative from start |

### Color Thresholds (LUFS)
- Green: below -14 LUFS
- Yellow: -14 to -1 LUFS
- Red: above -1 LUFS

## True Peak (center-right)

Per-channel true peak meters:

| Element | Display | Notes |
|---------|---------|-------|
| Channel label | L/R/C/... | Channel name |
| True Peak value | dBTP | Highlighted red if > 0 dBTP (clipping) |

## Correlation (right)

| Element | Display | Notes |
|---------|---------|-------|
| L/R Correlation | Numeric value (-1.0 to +1.0) | Only shown for stereo signals |
| Visual indicator | Horizontal bar or arrow | Shows mono ↔ wide ↔ out-of-phase |

### Correlation Color
- Green: > +0.5 (good mono compatibility)
- Yellow: 0.0 to +0.5 (wide stereo)
- Red: < 0.0 (phase issues)

## Parameters

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Enabled | enabled | Toggle | Enable/disable metering |

## Visualizations

### LUFS Meters
- **Type:** Horizontal bar meters with EBU R128 loudness values
- **Size:** Full width of left section
- **Update rate:** Real-time (every audio frame processed)

### True Peak Meters
- **Type:** Per-channel dBTP readings with clipping indicators
- **Color:** Red when > 0 dBTP

### Correlation Meter
- **Type:** Horizontal indicator from -1 to +1
- **Size:** 100px wide minimum

## Responsive Behavior
- **Compact:** Correlation hidden, true peak shows only max across channels
- **Wide:** All three sections visible side by side

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Enabled | enabled | Setup | General |
