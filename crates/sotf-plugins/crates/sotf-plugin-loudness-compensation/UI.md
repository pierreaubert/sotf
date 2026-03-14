# Loudness Compensation — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Loudness Comp" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+------------------+--------------------------------------------+------------------+
| (empty)          | LOW                   HIGH                  | AUTO GAIN        |
|                  |                                            |                  |
|                  | [Frequency]  knob     [Frequency]  knob    | [Enabled]  tog   |
|                  | [Gain]       knob     [Gain]       knob    | [Max]      knob  |
|                  |                                            | [Smooth]   knob  |
|                  |                                            | [Current]  label |
+------------------+--------------------------------------------+------------------+
```

## Config (Left Column — empty)

Width: 100px fixed. No setup parameters.

## Main (Center — shelf EQ sections)

Two side-by-side sections for the low and high shelving filters:

### LOW section

| Parameter | engine_key | Control | Param Index | Shortcut | Notes |
|-----------|-----------|---------|-------------|----------|-------|
| Frequency | low_freq | Knob | 0 | f | 20–500 Hz |
| Gain | low_gain | Knob | 1 | g | -20 to +20 dB |

### HIGH section

| Parameter | engine_key | Control | Param Index | Shortcut | Notes |
|-----------|-----------|---------|-------------|----------|-------|
| Frequency | high_freq | Knob | 2 | — | 2000–20000 Hz |
| Gain | high_gain | Knob | 3 | — | -20 to +20 dB |

## Output (Right Column — "AUTO GAIN")

Width: 120px fixed

| Parameter | engine_key | Control | Param Index | Notes |
|-----------|-----------|---------|-------------|-------|
| Enabled | auto_gain_enabled | Toggle | 4 | Structural — enables auto gain |
| Max | auto_gain_max_db | Knob | 5 | 0–24 dB. Only relevant when auto gain enabled |
| Smoothing | auto_gain_smoothing_ms | Knob | 6 | 1–1000 ms |
| Current | — | Label (read-only) | — | Shows current auto-gain value in dB. Green if positive, yellow if negative. Only visible when auto gain enabled |

## Visualizations

None.

## Responsive Behavior
- **Compact:** Low and High sections stack vertically instead of side-by-side
- **Wide:** Low and High sections side-by-side with auto gain column

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Low Freq | low_freq | Primary | Low |
| Low Gain | low_gain | Primary | Low |
| High Freq | high_freq | Primary | High |
| High Gain | high_gain | Primary | High |
| Auto Gain | auto_gain_enabled | Output | Auto Gain |
| Max Auto Gain | auto_gain_max_db | Output | Auto Gain |
| Smoothing | auto_gain_smoothing_ms | Output | Auto Gain |
