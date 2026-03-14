# PND — UI Specification

## Layout Mode
custom (simple)

## ASCII Layout

```
+---------------------------------------------------------------------+
| menu ui                                        | menu preset | T S X|
+---------------------------------------------------------------------+
| CORRECTION              | ANALYSIS                                  |
| [Strength knob]         | [Window knob]                             |
|                         | [Smoothing knob]                          |
+---------------------------------------------------------------------+
```

## Menu Bar
| Position | Element | Behavior |
|----------|---------|----------|
| Right | Preset picker | Standard |
| Right | T S X | Toggle/Solo/Close |

## Main Layout
Two columns side by side with `gap_6`.

### Column 1: CORRECTION
Section title: "CORRECTION"

| Parameter | Control | Param Index | Range | Unit | Hotkey |
|-----------|---------|-------------|-------|------|--------|
| Strength | Knob | 0 | 0–200 | % (×100) | s |

### Column 2: ANALYSIS
Section title: "ANALYSIS"

| Parameter | Control | Param Index | Range | Unit | Hotkey |
|-----------|---------|-------------|-------|------|--------|
| Window | Knob | 1 | 20–500 | ms | w |
| Smoothing | Knob | 2 | 1–1000 | ×0.001 (display_scale 1000) | m |

## Param Index Mapping
| Index | Parameter | ParamSpec Key | Display Scale |
|-------|-----------|---------------|---------------|
| 0 | correction_strength | `correction_strength` | ×100 |
| 1 | analysis_window_ms | `analysis_window_ms` | 1.0 |
| 2 | drift_smoothing | `drift_smoothing` | ×1000 |

## Responsive Behavior
- No responsive breakpoints — layout is minimal and fixed
- Two columns always visible side by side
