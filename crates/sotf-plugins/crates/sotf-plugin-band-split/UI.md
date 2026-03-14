# Band Split — UI Specification

## Layout Mode
custom (simple)

## ASCII Layout

```
+---------------------------------------------------------------------+
| menu ui                                        | menu preset | T S X|
+---------------------------------------------------------------------+
|                    CROSSOVER                                        |
|       [Frequency knob]     [LR24|LR48]                             |
|                                                                     |
| Splits audio into low and high frequency bands for parallel proc.   |
+---------------------------------------------------------------------+
```

## Menu Bar
| Position | Element | Behavior |
|----------|---------|----------|
| Right | Preset picker | Standard |
| Right | T S X | Toggle/Solo/Close |

## Main (Single Section)
Section title: "CROSSOVER"

| Parameter | Control | Param Index | Range | Unit | Notes |
|-----------|---------|-------------|-------|------|-------|
| Frequency | Knob | 0 | 20–20000 | Hz | Crossover frequency (structural — rebuilds filters) |
| Type | Button pair (LR24/LR48) | 1 | 0=LR24, 1=LR48 | — | Toggle between crossover slopes (structural — rebuilds plugin) |

### Type Button Behavior
Two adjacent buttons in a rounded container with 1px border:
- Active button: `theme.accent` background, `theme.text_on_accent` text
- Inactive button: `theme.surface` background, `theme.text_muted` text
- Click dispatches `set_plugin_param(plugin_idx, 1, 0.0)` for LR24, `1.0` for LR48

### Description Text
Italic, `text_xs`, `theme.text_muted`:
"Splits audio into low and high frequency bands for parallel processing."

## Param Index Mapping
| Index | Parameter | ParamSpec Key |
|-------|-----------|---------------|
| 0 | frequency | `frequency` |
| 1 | type | `type` (Choice: LR24=0, LR48=1) |

## Responsive Behavior
- No responsive breakpoints — layout is minimal and fixed
- Knob and type selector centered horizontally with `justify_around`
