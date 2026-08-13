# Multiband Expander — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "MB Expander" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+------------------+--------------------------------------------+------------------+
| GLOBAL           | BAND VIEW                                  | OUTPUT           |
|                  |                                            |                  |
| [Bands]    knob  | [Global] [1] [2] [3] ... tabs              | [Link Ch]  tog   |
| [XOver 1]  knob  |                                            | [Mix]      knob  |
| [XOver 2]  knob  | DYNAMICS              TIMING               |                  |
| [XOver 3]  knob  | [Threshold] slider    [Attack] slider      |                  |
| [XOver 4]  knob  | [Ratio]     slider    [Release] slider     |                  |
|                  | [Knee]      slider    [Hold]    slider      |                  |
|                  | [Range]     slider                         |                  |
|                  | [Hyst.]     slider                         |                  |
|                  | ┌─ Transfer Curve ─┐                       |                  |
|                  | └──────────────────┘                       |                  |
|                  | [Active] [Solo] [Bypass] [AutoGain]*        |                  |
+------------------+--------------------------------------------+------------------+
* Band toggles only shown when a band (not Global) is selected
```

## Config (Left Column — "GLOBAL", always visible)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Bands | num_bands | Knob (integer) | b | 2 to 5. Structural — requires rebuild |
| Crossover 1 | crossover_freq_1 | Knob | 1 | 20–500 Hz. Always visible |
| Crossover 2 | crossover_freq_2 | Knob | 2 | 500–5000 Hz. Hidden when bands < 3 |
| Crossover 3 | crossover_freq_3 | Knob | 3 | 5000–15000 Hz. Hidden when bands < 4 |
| Crossover 4 | crossover_freq_4 | Knob | 4 | 10000–18000 Hz. Hidden when bands < 5 |

Width: 120px fixed

The standalone `Expander` identity omits this column entirely because it is a
fixed one-band broadband processor. Processing mode is likewise structural;
hosts must rebuild rather than send it as realtime automation.

## Main (Center — "BAND VIEW", always visible)

### Band Tabs (top)

Horizontal tab bar: `[Global] [1] [2] [3] ...` (up to num_bands).
Selection stored in `plugin_state.selected_eq_band`. Band indices use `band_idx * 100 + param_offset`.

### DYNAMICS sub-section (below tabs, left)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Threshold | threshold | Vertical slider with ticks | t | -80 to 0 dB |
| Ratio | ratio | Vertical slider with ticks | r | 1:1 to 20:1 |
| Knee | knee | Vertical slider with ticks | k | 0 to 20 dB |
| Range | range | Vertical slider with ticks | g | 0 to 80 dB |
| Hysteresis | hysteresis | Vertical slider with ticks | y | 0 to 12 dB. Label "Hyst." |

Slider height: 180px

### TIMING sub-section (below tabs, right)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Attack | attack | Vertical slider with ticks | a | 0.1 to 50 ms |
| Release | release | Vertical slider with ticks | e | 10 to 2000 ms |
| Hold | hold | Vertical slider with ticks | h | 0 to 500 ms |

Slider height: 180px

### Transfer Curve (below DYNAMICS sliders)

Small transfer curve showing expander characteristic for selected band/global.

### Band Toggles (below transfer curve, band-level only)

Only visible when a band tab (not Global) is selected.

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Active | active | Toggle (Active/Passive) | When passive, band passes through unexpanded |
| Solo | solo | Toggle (On/Off) | Solo this band |
| Bypass | bypass | Toggle (On/Off) | Bypass expansion for this band |
| Auto Gain | auto_makeup | Toggle (On/Off) | Automatic makeup gain |

Toggles arranged horizontally, centered.

## Output (Right Column)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Link Channels | link_channels | Toggle (Linked/Unlinked) | — | Shared detection across channels |
| Mix | mix | Knob | m | Display as 0–100% (display_scale: 100.0) |

Width: 120px fixed

## Visualizations

### Transfer Curve
- **Type:** Static curve showing expander input/output relationship
- **Size:** 140x140px minimum
- **Curve:** 1:1 line above threshold, steeper slope below, limited by range parameter

## Responsive Behavior
- **Compact:** Transfer curve hidden, band toggles collapsed
- **Wide:** Transfer curve visible, all band toggles shown inline

## Parameter Indexing

Same scheme as MultibandCompressor: global uses direct indices, per-band uses `band_number * 100 + offset`.

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Bands | num_bands | Setup | Structure |
| Crossover 1-4 | crossover_freq_* | Setup | Crossover |
| Threshold | threshold | Primary | Dynamics |
| Ratio | ratio | Primary | Dynamics |
| Knee | knee | Primary | Dynamics |
| Range | range | Primary | Dynamics |
| Hysteresis | hysteresis | Primary | Dynamics |
| Attack | attack | Primary | Timing |
| Release | release | Primary | Timing |
| Hold | hold | Primary | Timing |
| Auto Makeup | auto_makeup | Output | Per-Band |
| Active | active | Setup | Per-Band |
| Solo | solo | Setup | Per-Band |
| Bypass | bypass | Setup | Per-Band |
| Mix | mix | Output | Output |
| Link Channels | link_channels | Setup | Channels |
