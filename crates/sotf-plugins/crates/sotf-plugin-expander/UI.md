# Expander — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Expander" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+------------------+--------------------------------------+------------------+
| SETUP            | DYNAMICS                 TIMING      | OUTPUT           |
|                  |                                      |                  |
| [Link Ch] toggle | [Threshold] slider   [Attack] slider | [GR Meter]       |
| [SC HPF]  knob   | [Ratio]     slider   [Release]slider | [AutoGain] tog   |
|                  | [Range]     slider   [Hold]   slider | [Mix]      knob  |
|                  | [Knee]      slider                   |                  |
|                  | [Hyst.]     slider                   |                  |
|                  | ┌─ Transfer Curve ──────────┐        |                  |
|                  | │                           │        |                  |
|                  | └───────────────────────────┘        |                  |
+------------------+--------------------------------------+------------------+
```

## Config (Left Column, always visible)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Link Channels | link_channels | Toggle (Linked/Unlinked) | — | Labeled toggle switch |
| Sidechain HPF | sidechain_hpf_hz | Knob | s | Display range 40–160 Hz in UI, full range 0–500 Hz |

Width: 100px fixed

## Main (Center, always visible)

Two sub-sections side by side: DYNAMICS (left) and TIMING (right).

### DYNAMICS sub-section

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Threshold | threshold | Vertical slider with ticks | t | -80 to 0 dB |
| Ratio | ratio | Vertical slider with ticks | r | 1:1 to 20:1 |
| Range | range | Vertical slider with ticks | g | 0 to 80 dB. Maximum attenuation depth |
| Knee | knee | Vertical slider with ticks | k | 0 to 20 dB |
| Hysteresis | hysteresis | Vertical slider with ticks | y | 0 to 12 dB. Label abbreviated "Hyst." |

Slider height: 180px

### TIMING sub-section

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Attack | attack | Vertical slider with ticks | a | 0.1 to 50 ms |
| Release | release | Vertical slider with ticks | e | 10 to 2000 ms |
| Hold | hold | Vertical slider with ticks | h | 0 to 500 ms |

Slider height: 180px

### Transfer Curve (below DYNAMICS sliders)

Spans the DYNAMICS sub-section width. Shows expander characteristic — signals below threshold are pushed further down.

## Output (Right Column)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| GR Meter | — | Gain reduction bar meter | — | Read-only, shows expansion attenuation. Range: 0 to -40 dB |
| Auto Makeup | auto_makeup | Toggle (On/Off) | — | Compensate for average attenuation |
| Mix | mix | Knob | m | Display as 0–100% (display_scale: 100.0) |

Width: 120px fixed

## Visualizations

### Transfer Curve
- **Type:** Static curve showing input/output relationship (expansion)
- **Size:** 200x200px minimum, stretches with DYNAMICS column
- **Axes:** X = Input level (dB), Y = Output level (dB)
- **Curve:** 1:1 line above threshold, steeper slope below threshold (signals pushed further down). Range parameter limits how far below unity the curve goes
- **Knee:** Soft transition zone centered on threshold
- **Colors:** Linear region = primary, expanded region = accent/red

## Diagnostic

No separate diagnostic panel. The GR meter in the output column serves as the primary diagnostic.

## Responsive Behavior
- **Compact:** Transfer curve hidden, sliders shortened to 120px
- **Wide:** Transfer curve grows to fill available space, up to 300x300px

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Threshold | threshold | Primary | Dynamics |
| Ratio | ratio | Primary | Dynamics |
| Range | range | Primary | Dynamics |
| Knee | knee | Primary | Dynamics |
| Hysteresis | hysteresis | Primary | Dynamics |
| Attack | attack | Primary | Timing |
| Release | release | Primary | Timing |
| Hold | hold | Primary | Timing |
| Mix | mix | Output | Output |
| Auto Makeup | auto_makeup | Output | Output |
| Link Channels | link_channels | Setup | Channels |
| Sidechain HPF | sidechain_hpf_hz | Setup | Sidechain |
