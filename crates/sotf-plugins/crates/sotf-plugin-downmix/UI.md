# Downmix — UI Specification

## Layout Mode
auto-layout (with minor customizations)

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Downmix" label | Plugin name |
| Left | Input channels badge | Shows detected input config (e.g., "5.1", "7.1.4") |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+----------------------------------------------------------------------+
| CHANNEL GAINS                              | PHASE                    |
|                                            |                         |
| [Center Gain]   knob                       | [Phase Coherence] toggle |
| [Surround Gain] knob                       |                         |
| [Height Gain]   knob                       |                         |
| [LFE Gain]      knob                       |                         |
+----------------------------------------------------------------------+
```

## Main (always visible)

### CHANNEL GAINS (left)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Center Gain | center_gain_db | Knob | c | -24 to 12 dB |
| Surround Gain | surround_gain_db | Knob | s | -24 to 12 dB |
| Height Gain | height_gain_db | Knob | h | -24 to 12 dB |
| LFE Gain | lfe_gain_db | Knob | l | -24 to 12 dB |

### PHASE (right)

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Phase Coherence | phase_coherence | Toggle | Enable FFT-based phase alignment. Adds latency. |

Phase blend parameters (phase_blend_low_hz, phase_blend_high_hz) are JSON-config only — not exposed in the UI since they rarely need adjustment.

## Responsive Behavior
- **Compact:** 2x2 knob grid for channel gains, phase toggle below
- **Wide:** 4 knobs in a row with phase toggle to the right

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Center Gain | center_gain_db | Primary | Channel Gains |
| Surround Gain | surround_gain_db | Primary | Channel Gains |
| Height Gain | height_gain_db | Primary | Channel Gains |
| LFE Gain | lfe_gain_db | Primary | Channel Gains |
| Phase Coherence | phase_coherence | Setup | Phase |
