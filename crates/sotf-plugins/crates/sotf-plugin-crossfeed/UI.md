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
| Mode | ButtonSet (Bauer / Meier / Multiband / HRTF / Disable) | Pill-button group, mutually exclusive |

## Main (Center, conditional on mode)

### Bauer Mode

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Bauer Cutoff | bauer_fcut_hz | Knob | 400 to 1000 Hz |
| Bauer Feed | bauer_feed_db | Knob | 0 to 15 dB |

### Meier Mode

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Meier Level | meier_level | Knob | 0 to 100% |

### Multiband Mode

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| MB Low Freq | mb_low_freq_hz | Knob | 50 to 500 Hz |
| MB Mid-High Freq | mb_mid_high_freq_hz | Knob | 2000 to 15000 Hz |
| MB Low Feed | mb_low_feed_db | Knob | -20 to 0 dB |
| MB Mid Feed | mb_mid_feed_db | Knob | 0 to 15 dB |
| MB High Feed | mb_high_feed_db | Knob | 0 to 15 dB |

### Disable Mode

No parameters shown. Plugin passes audio through unchanged.

### HRTF Mode

Uses the fixed compact parametric HRTF contract documented in `USAGE.md`.
The public ITD and head-yaw controls remain active; no mode-specific controls
are required.

## Output (Right Column)

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Auto Gain | autogain_enabled | Toggle | Loudness compensation |
| Auto Gain Target | autogain_target_lufs | Knob | Absolute loudness target used by compensation DSP |
| Auto Gain Max | autogain_max_gain_db | Knob | 0 to 24 dB |
| Auto Gain Smoothing | autogain_smoothing_ms | Knob | 10 to 5000 ms |
| Mix | mix | Knob | 0 to 1.0 |

Head yaw (`head_yaw_deg`, -90 to +90 degrees) and static ITD (`itd_delay_ms`, 0 to 1 ms) are
available as host parameters. The current generated layout exposes static ITD; head yaw is intended
for head-tracking automation.

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
