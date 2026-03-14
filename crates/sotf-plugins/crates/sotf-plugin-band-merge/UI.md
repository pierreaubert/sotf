# Band Merge — UI Specification

## Layout Mode
custom (simple)

## ASCII Layout

```
+---------------------------------------------------------------------+
| menu ui                                        | menu preset | T S X|
+---------------------------------------------------------------------+
|                    MERGE CONFIG                                     |
|              [Number of Bands knob]                                 |
|                                                                     |
| Merges multiple frequency bands back together by summation.         |
+---------------------------------------------------------------------+
```

## Menu Bar
| Position | Element | Behavior |
|----------|---------|----------|
| Right | Preset picker | Standard |
| Right | T S X | Toggle/Solo/Close |

## Main (Single Section)
Section title: "MERGE CONFIG"

| Parameter | Control | Param Index | Range | Unit | Notes |
|-----------|---------|-------------|-------|------|-------|
| Number of Bands | Knob | 0 | 2–8 | — | Must match the splitting plugin's band count |

Knob is centered horizontally with `justify_center`.

### Description Text
Italic, `text_xs`, `theme.text_muted`:
"Merges multiple frequency bands back together by summation."

## Param Index Mapping
| Index | Parameter | ParamSpec Key |
|-------|-----------|---------------|
| 0 | bands | `bands` (Int) |

## Responsive Behavior
- No responsive breakpoints — layout is minimal and fixed
