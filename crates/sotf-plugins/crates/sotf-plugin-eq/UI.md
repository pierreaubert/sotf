# Parametric EQ — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "EQ" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+---------------------------------------------------------------------+
| menu EQ                                      | menu preset | T S X  |
+---------------------------------------------------------------------+
|                                                                     |
| [All Channels] [Per Channel]  [L] [R] [C] ...                      |
|                                                                     |
| ┌── EQ Frequency Response Graph ─────────────────── Legend ───────┐ |
| │  dB axis │  Composite curve + per-band curves     #1 Peak       │ |
| │          │  Interactive: click band to select      #2 Lowshelf   │ |
| │          │                                         #3 Highshelf  │ |
| └──────────┴─────────────────────────────────────────────────────┘  |
|                                                                     |
| BAND CONTROLS (for selected band)                                   |
| [Freq] knob  [Q] knob  [Gain] knob  [Type] selector  [AutoGain]   |
+---------------------------------------------------------------------+
```

## Mode Selector (top)

| Element | Control | Notes |
|---------|---------|-------|
| All Channels / Per Channel | ButtonSet toggle | Switches between global and per-channel EQ mode |
| Channel Tabs | ButtonSet (L/R/C...) | Only visible in Per Channel mode. Count matches `num_channels` |

Selection stored in `plugin_state.selected_eq_channel`.

## Main — EQ Graph (center, full width)

### Frequency Response Graph
- **Type:** Interactive frequency response curve
- **Size:** Full width, 200px minimum height
- **X-axis:** Frequency (logarithmic, 20 Hz — 20 kHz), labels at standard frequencies
- **Y-axis:** Gain (dB), labels at +3, 0, -20, -40, -60 dB
- **Curves:** Composite response (bold) + individual band curves (colored)
- **Interactive:** Click band legend to select it for editing
- **Colors:** Each band gets a unique color from the theme palette
- **Legend:** Right side panel showing band list with color swatches, filter type, frequency, and status (muted/solo)

### Band Legend (right of graph)

Each band shows:
- Color swatch
- Band number
- Filter type abbreviation (PK, LS, HS, LP, HP, BP, N)
- Frequency
- Gain
- Muted/solo status icons

## Band Controls (below graph)

Per-band knobs for the currently selected band:

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Frequency | band_{n}_freq | Knob | f | 20–20000 Hz |
| Q | band_{n}_q | Knob | q | 0.1–10.0 |
| Gain | band_{n}_gain | Knob | g | -24 to +24 dB |
| Type | band_{n}_filter_type | Selector | t | Peak/Lowshelf/Highshelf/Lowpass/Highpass/Bandpass/Notch/AllPass |
| Order | band_{n}_order | Selector | o | 2/4/6/8; SVF is restricted to 2 |
| Auto Gain | auto_gain_enabled | Toggle | a | Global toggle |

Band-relative parameter indices use `selected_band × 5 + field_offset`, where
0=frequency, 1=Q, 2=gain, 3=filter type, and 4=order. Global controls are
mapped separately before the band array.

## Visualizations

### Frequency Response Graph
- **Type:** Interactive parametric EQ curve display
- **Size:** Full width, 200px minimum height
- **Curve rendering:** Per-band cascaded-biquad magnitude response computed at the active sample rate and order
- **Composite:** Sum of all band responses in dB
- **Grid:** Horizontal dB lines, vertical frequency lines at standard points

## Responsive Behavior
- **Compact:** Legend hidden, graph fills full width, band controls wrap
- **Wide:** Legend panel visible beside graph, all band controls in one row

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Auto Gain | auto_gain_enabled | Output | Global |
| Frequency | band_{n}_freq | Primary | Band N |
| Q | band_{n}_q | Primary | Band N |
| Gain | band_{n}_gain | Primary | Band N |
| Type | band_{n}_filter_type | Setup | Band N |
| Order | band_{n}_order | Setup | Band N |
