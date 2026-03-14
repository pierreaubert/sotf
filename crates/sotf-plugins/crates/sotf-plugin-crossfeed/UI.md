# Crossfeed — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Crossfeed" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+----------------------------------------------------------------------+
| [ Bauer ] [ Meier ] [ Multiband ] [ Disable ]     mode selector      |
+----------------------------------------------------------------------+
| MODE PARAMS (conditional)                  | OUTPUT                   |
|                                            |                         |
| Bauer:   [Cutoff] knob  [Feed] knob       | [Auto Gain] toggle      |
| Meier:   [Level] knob                      | [Mix]       knob        |
| MB:      [Low Freq] [Mid-High Freq] knobs  |                         |
|          [Low Feed] [Mid Feed] [High Feed]  |                         |
+----------------------------------------------------------------------+
```

## Mode Selector (top, always visible)

| Element | Control | Notes |
|---------|---------|-------|
| Mode | ButtonSet (Bauer / Meier / Multiband / Disable) | Pill-button group, mutually exclusive |

## Main (Center, conditional on mode)

### Bauer Mode

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Bauer Cutoff | bauer_fcut_hz | Knob | 300 to 2000 Hz |
| Bauer Feed | bauer_feed_db | Knob | 0 to 12 dB |

### Meier Mode

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Meier Level | meier_level | Knob | 0 to 100% |

### Multiband Mode

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| MB Low Freq | mb_low_freq_hz | Knob | 50 to 500 Hz |
| MB Mid-High Freq | mb_mid_high_freq_hz | Knob | 1000 to 10000 Hz |
| MB Low Feed | mb_low_feed_db | Knob | -12 to 12 dB |
| MB Mid Feed | mb_mid_feed_db | Knob | -12 to 12 dB |
| MB High Feed | mb_high_feed_db | Knob | -12 to 12 dB |

### Disable Mode

No parameters shown. Plugin passes audio through unchanged.

## Output (Right Column)

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Auto Gain | autogain_enabled | Toggle | Loudness compensation |
| Mix | mix | Knob | 0 to 1.0 |

## Responsive Behavior
- **Compact:** Mode selector wraps to 2x2 grid. Knobs stack vertically.
- **Wide:** Mode selector horizontal. MB knobs in 2 rows (freqs top, feeds bottom).

## ParamCategory Mapping

| Parameter | Category | Group |
|-----------|----------|-------|
| Mode | Setup | Mode |
| Bauer Cutoff, Bauer Feed | Primary | Bauer |
| Meier Level | Primary | Meier |
| MB Low Freq, MB Mid-High Freq | Primary | Multiband |
| MB Low/Mid/High Feed | Primary | Multiband |
| Auto Gain | Output | Auto Gain |
| Mix | Output | Output |
