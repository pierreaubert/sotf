# Channel Mute/Solo — UI Specification

## Layout Mode
custom

## ASCII Layout

```
+---------------------------------------------------------------------+
| menu ui                                        | menu preset | T S X|
+---------------------------------------------------------------------+
| SETUP      | CHANNELS                                              |
| [Enabled]  | [FL]  [FR]  [C]   [LFE]  [RL]  [RR]                  |
| 6 channels | [|  ] [|  ] [|  ] [|   ] [|  ] [|  ]  ← level meters  |
|            | [M]   [M]   [M]   [M]    [M]   [M]                    |
|            | [S]   [S]   [S]   [S]    [S]   [S]                    |
+---------------------------------------------------------------------+
```

## Menu Bar
| Position | Element | Behavior |
|----------|---------|----------|
| Right | Preset picker | Standard |
| Right | T S X | Toggle/Solo/Close |

## Setup (Left Column, 100px)
| Parameter | Control | Param Index | Notes |
|-----------|---------|-------------|-------|
| Enabled | Toggle | 0 | Master enable/disable |
| Channel count | Text display | — | Read-only, shows "{N} channels" |

## Channels (Center, flex)
Section title: "CHANNELS"

Dynamic number of channel strips arranged in a horizontal flex row with wrapping.

### Channel Strip Layout (per channel)
Each strip is a vertical column:

| Element | Type | Notes |
|---------|------|-------|
| Name | Text, bold | Channel name: Mono, Left/Right, FL/FR/C/LFE/RL/RR/SL/SR |
| Level meter | Bar, 16×60px | Green when active, empty when muted/not soloed |
| M button | Toggle, 24×20px | Mute — red (`theme.error`) when active |
| S button | Toggle, 24×20px | Solo — yellow (`theme.warning`) when active |

### Channel Name Mapping
| Count | Names |
|-------|-------|
| 1 | Mono |
| 2 | Left, Right |
| 6 | FL, FR, C, LFE, RL, RR |
| 8 | FL, FR, C, LFE, RL, RR, SL, SR |
| Other | Ch0, Ch1, Ch2... |

### Strip Appearance
| State | Border Color | Meter |
|-------|-------------|-------|
| Normal (active) | `theme.border` | Green, 60% height |
| Muted | `theme.error` | Empty |
| Soloed | `theme.warning` | Green, 60% height |
| Silenced by solo | `theme.border` | Empty |

Each strip has padding, rounded corners, and `theme.surface` background.

## Param Index Mapping
| Index | Parameter |
|-------|-----------|
| 0 | enabled (bool) |

Channel mute/solo/dim states are stored in a separate `channel_states` array, not in ParamSpec.

## Responsive Behavior
- Channel strips wrap to next row when width is insufficient
- No right column (no output meters or auto gain)
