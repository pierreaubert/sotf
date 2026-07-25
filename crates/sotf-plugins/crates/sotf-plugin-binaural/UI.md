# Binaural Decoder — UI Specification

## Layout Mode
custom

## ASCII Layout

```
+---------------------------------------------------------------------+
| menu ui                                        | menu preset | T S X|
+---------------------------------------------------------------------+
| SETUP (180px)            | CONTROLS (flex)       | (empty, 120px)   |
| SOFA File: [name] [Load] | [Externalization knob]|                  |
| Input Channels: 6        | [Near Field knob]     |                  |
| [Optimization] toggle    |                       |                  |
+---------------------------------------------------------------------+
```

## Menu Bar
| Position | Element | Behavior |
|----------|---------|----------|
| Right | Preset picker | Standard |
| Right | T S X | Toggle/Solo/Close |

## Setup (Left Column, 180px)
Section title: "SETUP"

| Parameter | Control | Param Index | Notes |
|-----------|---------|-------------|-------|
| SOFA File | File display + Load button | 0 | Shows filename (basename only, ellipsis if >120px). Load button dispatches `OpenSofaFile` action |
| Input Channels | Read-only text | 1 | Shows channel count as text |
| Optimization | Toggle | 2 | Sum-before-IFFT optimization |

### SOFA File Display
- Container with `theme.background_secondary`, rounded, padded
- Label "SOFA File" in `text_xs`, `theme.text_muted`
- Filename in `text_sm`, bold, `theme.text_primary` (or `theme.text_muted` if empty, showing "None")
- Load button: `theme.surface` background, 1px border, hover to `theme.surface_hover`

## Controls (Center Column, flex)
Section title: "CONTROLS"

| Parameter | Control | Param Index | Range | Unit |
|-----------|---------|-------------|-------|------|
| Externalization | Knob | 3 | 0.0–1.0 | — |
| Near Field | Knob | 4 | 0.0–1.0 | — |

Both knobs in a horizontal row with `gap_4`.

## Right Column (120px, empty)
Empty spacer for visual balance.

## Param Index Mapping
| Index | Parameter | ParamSpec Key |
|-------|-----------|---------------|
| 0 | sofa_file | `sofa_file` (FilePath) |
| 1 | input_channels | `input_channels` (Int) |
| 3 | externalization | `externalization` (Float) |
| 4 | near_field_strength | `near_field_strength` (Float) |

## Responsive Behavior
- 3-column layout maintained at all widths
- Center column is flex, right column is fixed spacer
