# Limiter — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Limiter" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+------------------+--------------------------------------------+------------------+
| SETUP            | DYNAMICS              TIMING                | OUTPUT           |
|                  |                                            |                  |
| [Soft Knee] tog  | [Ceiling] slider      [Release] slider     | [GR Meter]       |
|                  |                       [Lookahead] slider   | [Mix]      knob  |
|                  | ┌─ Limiter Curve ──────────────────┐       |                  |
|                  | │                                  │       |                  |
|                  | └──────────────────────────────────┘       |                  |
+------------------+--------------------------------------------+------------------+
```

## Config (Left Column, always visible)

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Soft Knee | soft | Toggle (Soft/Hard) | Clipping mode selector |

Width: 100px fixed

## Main (Center, always visible)

Two sub-sections side by side: DYNAMICS (left) and TIMING (right).

### DYNAMICS sub-section

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Ceiling | threshold | Vertical slider with ticks | c | -20 to 0 dB. Labeled "Ceiling" in UI |

Slider height: 180px

### TIMING sub-section

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Release | release | Vertical slider with ticks | r | 10 to 1000 ms |
| Lookahead | lookahead | Vertical slider with ticks | l | 0 to 20 ms |

Slider height: 180px

### Transfer Curve (below DYNAMICS sliders)

Spans the DYNAMICS sub-section width. Shows limiter characteristic with infinite ratio above threshold.

## Output (Right Column)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| GR Meter | — | Gain reduction bar meter | — | Read-only, range: 0 to -20 dB |
| Mix | mix | Knob | m | Display as 0–100% (display_scale: 100.0) |

Width: 120px fixed

## Visualizations

### Transfer Curve
- **Type:** Static curve showing input/output relationship
- **Size:** 200x200px minimum, stretches with DYNAMICS column
- **Axes:** X = Input level (dB), Y = Output level (dB)
- **Curve:** 1:1 line below threshold, flat (infinite ratio) above threshold
- **Soft mode:** Gradual saturation curve above 90% of threshold when soft knee enabled
- **Interactive:** Input level indicator showing current operating point from peak detection data
- **Colors:** Linear region = primary, limited region = accent/red, operating point = yellow/warning

## Diagnostic

No separate diagnostic panel. The GR meter in the output column serves as the primary diagnostic.

## Responsive Behavior
- **Compact:** Transfer curve hidden, sliders shortened to 120px
- **Wide:** Transfer curve grows to fill available space, up to 300x300px

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Threshold | threshold | Primary | Dynamics |
| Release | release | Primary | Timing |
| Lookahead | lookahead | Primary | Timing |
| Soft Knee | soft | Setup | Mode |
| Mix | mix | Output | Output |
