# Compressor — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Compressor" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Config (Left Column, always visible)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Link Channels | link_channels | Toggle (Linked/Unlinked) | — | Labeled toggle switch |
| Sidechain HPF | sidechain_hpf_hz | Knob | s | Display range 40–160 Hz in UI, full range 0–200 Hz |

Width: 100px fixed

## Main (Center, always visible)

Two sub-sections side by side: DYNAMICS (left) and TIMING (right).

### DYNAMICS sub-section

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Threshold | threshold | Vertical slider with ticks | t | -60 to 0 dB |
| Ratio | ratio | Vertical slider with ticks | r | 1:1 to 20:1 |
| Knee | knee | Vertical slider with ticks | k | 0 to 20 dB |

Slider height: 180px

### TIMING sub-section

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Attack | attack | Vertical slider with ticks | a | 0.1 to 100 ms |
| Release | release | Vertical slider with ticks | e | 10 to 1000 ms |

Slider height: 180px

### Transfer Curve (below sliders)

Spans the full center column width, below the DYNAMICS sliders.

## Output (Right Column)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| GR Meter | — | Gain reduction bar meter | — | Read-only, shows current gain reduction in dB. Range: 0 to -30 dB |
| Auto Makeup | auto_makeup | Toggle (On/Off) | — | |
| Makeup Gain | makeup_gain | Knob | m | -24 to 24 dB |
| Mix | mix | Knob | x | Display as 0–100% (display_scale: 100.0) |

Width: 120px fixed

## Visualizations

### Transfer Curve
- **Type:** Static curve showing input/output relationship
- **Size:** 200×200px minimum, stretches with center column
- **Axes:** X = Input level (dB), Y = Output level (dB)
- **Curve:** 1:1 line below threshold, compressed slope above threshold, soft knee transition
- **Interactive:** Input level indicator (vertical + horizontal line) showing current operating point based on gain reduction data
- **Colors:** Linear region = primary, compressed region = accent/red, operating point = yellow/warning

## Diagnostic

No separate diagnostic panel. The GR meter in the output column serves as the primary diagnostic.

## Responsive Behavior
- **Compact:** Transfer curve hidden, sliders shortened to 120px
- **Wide:** Transfer curve grows to fill available space, up to 300×300px

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Threshold | threshold | Primary | Dynamics |
| Ratio | ratio | Primary | Dynamics |
| Attack | attack | Primary | Timing |
| Release | release | Primary | Timing |
| Knee | knee | Primary | Dynamics |
| Makeup Gain | makeup_gain | Output | Output |
| Mix | mix | Output | Output |
| Auto Makeup | auto_makeup | Output | Output |
| Link Channels | link_channels | Setup | Channels |
| Sidechain HPF | sidechain_hpf_hz | Setup | Sidechain |
