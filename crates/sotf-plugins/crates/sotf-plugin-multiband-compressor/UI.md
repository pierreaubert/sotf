# Multiband Compressor — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "MB Compressor" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+------------------+--------------------------------------------+------------------+
| GLOBAL           | BAND VIEW                                  | OUTPUT           |
|                  |                                            |                  |
| [Bands]    knob  | [Global] [1] [2] [3] ... tabs              | [Link Ch]  tog   |
| [XOver 1]  knob  |                                            | [Mix]      knob  |
| [XOver 2]  knob  | DYNAMICS            TIMING                 |                  |
| [XOver 3]  knob  | [Threshold] slider  [Attack] slider        |                  |
| [XOver 4]  knob  | [Ratio]     slider  [Release] slider       |                  |
|                  | [Knee]      slider  [Makeup*] slider       |                  |
|                  | ┌─ Transfer Curve ─┐                       |                  |
|                  | └──────────────────┘                       |                  |
|                  | [Active] [Solo] [Bypass] [AutoGain]*        |                  |
+------------------+--------------------------------------------+------------------+
* Makeup slider and band toggles only shown when a band (not Global) is selected
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

Crossover knobs are conditionally rendered based on `num_bands`:
- 2 bands: XOver 1 only
- 3 bands: XOver 1, 2
- 4 bands: XOver 1, 2, 3
- 5 bands: XOver 1, 2, 3, 4

## Main (Center — "BAND VIEW", always visible)

### Band Tabs (top)

Horizontal tab bar: `[Global] [1] [2] [3] ...` (up to num_bands).

| Element | Type | Notes |
|---------|------|-------|
| Tab buttons | Pill-shaped toggles | Selected tab: accent bg + bold. Unselected: secondary bg |
| Global tab | Always present | Shows/edits global defaults |
| Band tabs | 1..num_bands | Shows/edits per-band overrides |

Selection stored in `plugin_state.selected_eq_band`. Band indices use `band_idx * 100 + param_offset` for param addressing.

### DYNAMICS sub-section (below tabs, left)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Threshold | threshold | Vertical slider with ticks | t | -60 to 0 dB |
| Ratio | ratio | Vertical slider with ticks | r | 1:1 to 20:1 |
| Knee | knee | Vertical slider with ticks | k | 0 to 20 dB |

Slider height: 180px

### TIMING sub-section (below tabs, right)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Attack | attack | Vertical slider with ticks | a | 0.1 to 100 ms |
| Release | release | Vertical slider with ticks | e | 10 to 1000 ms |
| Makeup | makeup_gain | Vertical slider with ticks | g | -24 to 24 dB. Only visible when band selected (not Global) |

Slider height: 180px

### Transfer Curve (below DYNAMICS sliders)

Small transfer curve showing current band/global compression characteristic.

### Band Toggles (below transfer curve, band-level only)

Only visible when a band tab (not Global) is selected.

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Active | active | Toggle (Active/Passive) | When passive, band passes through uncompressed |
| Solo | solo | Toggle (On/Off) | Solo this band (mutes all others) |
| Bypass | bypass | Toggle (On/Off) | Bypass compression for this band |
| Auto Gain | auto_makeup | Toggle (On/Off) | Automatic makeup gain for this band |

Toggles arranged horizontally, centered.

## Output (Right Column)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Link Channels | link_channels | Toggle (Linked/Unlinked) | — | Shared detection across channels |
| Mix | mix | Knob | m | Display as 0–100% (display_scale: 100.0) |

Width: 120px fixed

## Visualizations

### Transfer Curve
- **Type:** Static curve showing input/output relationship for selected band/global
- **Size:** 140x140px minimum
- **Axes:** X = Input level (dB), Y = Output level (dB)
- **Curve:** 1:1 line below threshold, compressed slope above, soft knee transition
- **Colors:** Linear region = primary, compressed region = accent/red

## Diagnostic

No separate diagnostic panel. Future: per-band GR meters could be added to the band view.

## Responsive Behavior
- **Compact:** Transfer curve hidden, Makeup slider hidden even on band tabs
- **Wide:** Transfer curve visible, all band toggles shown inline

## Parameter Indexing

Global parameters use direct indices (0–12). Per-band parameters use `band_number * 100 + offset`:
- Band 1: 100+offset, Band 2: 200+offset, etc.
- Offset mapping: 6=threshold, 7=ratio, 8=attack, 9=release, 10=knee, 13=makeup, 14=bypass, 15=solo, 16=auto_makeup, 17=active

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Bands | num_bands | Setup | Structure |
| Crossover 1 | crossover_freq_1 | Setup | Crossover |
| Crossover 2 | crossover_freq_2 | Setup | Crossover |
| Crossover 3 | crossover_freq_3 | Setup | Crossover |
| Crossover 4 | crossover_freq_4 | Setup | Crossover |
| Threshold | threshold | Primary | Dynamics |
| Ratio | ratio | Primary | Dynamics |
| Attack | attack | Primary | Timing |
| Release | release | Primary | Timing |
| Knee | knee | Primary | Dynamics |
| Makeup Gain | makeup_gain | Output | Per-Band |
| Auto Makeup | auto_makeup | Output | Per-Band |
| Active | active | Setup | Per-Band |
| Solo | solo | Setup | Per-Band |
| Bypass | bypass | Setup | Per-Band |
| Mix | mix | Output | Output |
| Link Channels | link_channels | Setup | Channels |
