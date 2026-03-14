# Matrix — UI Specification

## Layout Mode
custom

## ASCII Layout

```
+---------------------------------------------------------------------+
| menu ui                                        | menu preset | T S X|
+---------------------------------------------------------------------+
| Preset: [Identity] [Swap L/R] [Mono] [M/S Enc] [M/S Dec]           |
+---------------------------------------------------------------------+
|           IN: L     R     C    ...          | M S D              |
| OUT:  L  [0 dB] [-∞ ] [-∞ ] ...           | [M] [S] [D] Front  |
|       R  [-∞ ] [0 dB] [-∞ ] ...           |                     |
|       C  [-∞ ] [-∞ ] [0 dB] ...           | [M] [S] [D] Center  |
|      ...                                   | ...                 |
+---------------------------------------------------------------------+
```

## Menu Bar
| Position | Element | Behavior |
|----------|---------|----------|
| Right | Preset picker | Standard preset load/save |
| Right | T S X | Toggle/Solo/Close |

## Preset Buttons (Top Row)
| Element | Type | Behavior |
|---------|------|----------|
| Identity | Button | Sets diagonal matrix (1:1 pass-through) |
| Swap L/R | Button | Swaps left and right inputs |
| Mono Mix | Button | Sums all inputs equally to all outputs |
| M/S Encode | Button | Mid/Side encoding (disabled if >2 channels) |
| M/S Decode | Button | Mid/Side decoding (disabled if >2 channels) |

Active preset is highlighted with `theme.accent`. Disabled presets (M/S when >2 channels) shown at 50% opacity.

## Main: Interactive Grid
| Element | Type | Notes |
|---------|------|-------|
| Corner label | Text | Shows "OUT\IN" |
| Column headers | Text labels | Input channel names from speaker config (L, R, C, LFE, LS, RS...) |
| Row labels | Text labels | Output channel names from speaker config |
| Grid cells | Interactive cells | 48×48px each, display gain in dB |

### Cell Behavior
| Interaction | Action |
|-------------|--------|
| Single click | Toggle gain between 0.0 and 1.0 (linear); selects cell for editing |
| Double click | Reset cell to 0.0 and clear M/S/D for that output channel |
| Scroll up | Increase gain by 1 dB step (preserves sign for negative gains) |
| Scroll down | Decrease gain by 1 dB step |

### Cell Appearance
| State | Background | Border | Text |
|-------|------------|--------|------|
| Inactive (gain ≈ 0) | `theme.surface` | `theme.background_secondary` | `theme.text_muted`, "-∞" |
| Active (positive gain) | Blend toward `theme.accent` by intensity | `theme.border` | `theme.text_primary`, bold |
| Active (negative gain) | Blend toward `theme.warning` by intensity | `theme.border` | `theme.text_primary`, bold |
| Selected | `theme.accent_muted` | `theme.accent` | `theme.text_primary` |

Gain range: -60 to +6 dB. Display: "-∞" for silence, "0" for unity, "{sign}{dB:.1}" otherwise.

## M/S/D Sidebar (Right of Grid)
| Element | Type | Notes |
|---------|------|-------|
| Header | Text | "M S D" |
| Group rows | Grouped by speaker groups | Height matches corresponding grid rows |
| M button | Toggle, 20×18px | Mute — `theme.error` when active |
| S button | Toggle, 20×18px | Solo — `theme.warning` when active |
| D button | Toggle, 20×18px | Dim — `theme.info` when active |
| Group label | Text | Shown when group has >1 channel (e.g., "Front") |

Groups are computed from `MeterGroupSpec` based on speaker configuration. Toggle behavior: if any channel in group has the flag set, clicking clears all; otherwise sets all.

Column width: 80px. Separated from grid by `border_l_1`.

## Param Index Mapping
Matrix cells are indexed as: `output_idx * input_channels + input_idx`

The single ParamSpec entry (`gain`) is used for display scale; actual gain values are stored in the flat matrix array.

## Responsive Behavior
- Grid scales with channel count (cell size fixed at 48px)
- M/S/D sidebar always visible
- Horizontal scroll if grid exceeds available width
