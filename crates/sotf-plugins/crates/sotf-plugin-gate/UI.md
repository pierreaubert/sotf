# Gate — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Gate" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+------------------+--------------------------------------------+------------------+
| SETUP            | DYNAMICS              TIMING                | OUTPUT           |
|                  |                                            |                  |
| [Link Ch] toggle | [Threshold] slider    [Attack] slider      | [GR Meter]       |
| [SC HPF]  knob   | [Ratio]     slider    [Hold]   slider      | [Mix]      knob  |
|                  |                       [Release] slider     |                  |
|                  | ┌─ Gate Status ────────────────────┐       |                  |
|                  | │ (●) OPEN/CLOSED  [input meter]   │       |                  |
|                  | └──────────────────────────────────┘       |                  |
+------------------+--------------------------------------------+------------------+
```

## Config (Left Column, always visible)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Link Channels | link_channels | Toggle (Linked/Unlinked) | — | Labeled toggle switch |
| Sidechain HPF | sidechain_hpf_hz | Knob | s | Display range 40–160 Hz in UI, full range 0–200 Hz |
| HPF Order | sidechain_hpf_order | Selector (2nd/4th) | — | Structural; rebuild graph |
| Detection | detection_mode | Selector (Peak/RMS) | — | Structural; rebuild graph |
| External Sidechain | sidechain_external | Toggle | — | Structural; doubles input width |

Width: 100px fixed

## Main (Center, always visible)

Two sub-sections side by side: DYNAMICS (left) and TIMING (right), with Gate Status below.

### DYNAMICS sub-section

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Threshold | threshold | Vertical slider with ticks | t | -80 to 0 dB |
| Ratio | ratio | Vertical slider with ticks | r | 1:1 to 100:1 |
| Range | range_db | Vertical slider with ticks | — | 0–120 dB; 0 means unlimited (240 dB finite ceiling) |
| Hysteresis | hysteresis_db | Vertical slider with ticks | — | 0–12 dB |
| Knee | knee_db | Vertical slider with ticks | — | 0–20 dB |

Slider height: 180px

### TIMING sub-section

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Attack | attack | Vertical slider with ticks | a | 0.1 to 50 ms |
| Hold | hold | Vertical slider with ticks | h | 0 to 1000 ms |
| Release | release | Vertical slider with ticks | e | 10 to 2000 ms |
| Lookahead | lookahead_ms | Knob | — | 0–20 ms; structural latency change |

Slider height: 180px

### Gate Status (below sliders)

Spans the full center column width.

| Element | Type | Notes |
|---------|------|-------|
| Status circle | 48px indicator | Green (OPEN) or Red (CLOSED) with glow background |
| Input meter | Horizontal bar | Shows input level (-80 to 0 dB) with threshold marker (yellow line) |

## Output (Right Column)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| GR Meter | — | Gain reduction bar meter | — | Read-only, shows attenuation depth. Range: 0 to -40 dB |
| Mix | mix | Knob | m | Display as 0–100% (display_scale: 100.0) |

Width: 120px fixed

## Visualizations

### Gate Status Indicator
- **Type:** Real-time state display with input level meter
- **Status circle:** 48px diameter, border + glow matching state color
- **States:** OPEN (green/success) or CLOSED (red/error)
- **Input meter:** Horizontal bar showing input level relative to threshold
- **Threshold marker:** Yellow/warning vertical line on the input meter at threshold position
- **Data source:** GateData { input_levels_db, is_open, attenuation_db }

## Diagnostic

No separate diagnostic panel. Gate status indicator and GR meter serve as diagnostics.

## Responsive Behavior
- **Compact:** Gate status indicator hidden, sliders shortened to 120px
- **Wide:** Gate status indicator shows full input level meter with threshold marker

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Threshold | threshold | Primary | Dynamics |
| Ratio | ratio | Primary | Dynamics |
| Attack | attack | Primary | Timing |
| Hold | hold | Primary | Timing |
| Release | release | Primary | Timing |
| Mix | mix | Output | Output |
| Link Channels | link_channels | Setup | Channels |
| Sidechain HPF | sidechain_hpf_hz | Setup | Sidechain |
