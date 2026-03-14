# A/B Compare — UI Specification

## Layout Mode
custom

## ASCII Layout

```
+---------------------------------------------------------------------+
| menu ui                                        | menu preset | T S X|
+---------------------------------------------------------------------+
| MIX (160px)              | AUTO GAIN (140px)                        |
| [MIX A+B | Choice]      | [ON | OFF]                               |
| [Mix knob] (dimmed if    | [Fast | Slow]                            |
|  binary mode)            | [Max Gain knob]                          |
| [A] [N] [B] (dimmed if  | [Gain Smooth slider]                     |
|  pot mode)               |                                          |
| [Mix Smooth slider]     |                                          |
+---------------------------------------------------------------------+
| PATH CONFIG                                                         |
| A: [Load] filename.json                                             |
| B: [Load] filename.json                                             |
+---------------------------------------------------------------------+
```

## Menu Bar
| Position | Element | Behavior |
|----------|---------|----------|
| Right | Preset picker | Standard |
| Right | T S X | Toggle/Solo/Close |

## Top Row: Mix + Auto Gain (side by side)

### Mix Column (Left, 160px)
Section title: "MIX"

| Parameter | Control | Param Index | Notes |
|-----------|---------|-------------|-------|
| Mix Mode | ButtonSet (MIX A+B / Choice) | 1 | 0=Potentiometer, 1=Binary |
| Mix | Knob | 0 | Range: -100 to +100 (display_scale ×100). Dimmed (opacity 0.4) in Binary mode |
| A/N/B | ButtonSet (A / N / B) | 0,1,2,3 | Compound action: sets mix_mode, bypass, selected_path, mix. Dimmed in Potentiometer mode |
| Mix Smooth | Horizontal Slider | 8 | Range: 1–500 ms (code clamps min to 5) |

### A/N/B Button Behavior
| Button | Action |
|--------|--------|
| A | Set mix_mode=Binary, bypass=false, selected_path=A, mix=-100 |
| N | Set mix_mode=Binary, bypass=true |
| B | Set mix_mode=Binary, bypass=false, selected_path=B, mix=+100 |

### Auto Gain Column (Right, 140px)
Section title: "AUTO GAIN"

| Parameter | Control | Param Index | Notes |
|-----------|---------|-------------|-------|
| Auto Gain | ButtonSet (ON / OFF) | 4 | Enable loudness matching |
| Loudness Type | ButtonSet (Fast / Slow) | 5 | 0=Momentary, 1=Short-term |
| Max Gain | Knob | 6 | Range: 0–24 dB |
| Gain Smooth | Horizontal Slider | 7 | Range: 10–500 ms |

## Bottom Section: Path Config
Section title: "PATH CONFIG"

| Element | Type | Notes |
|---------|------|-------|
| Path A row | Label "A:" + Load button + filename display | Load button dispatches `OpenAbConfigFile` action |
| Path B row | Label "B:" + Load button + filename display | Same action with path_id="b" |

### Load Button
- Small button with `theme.surface_hover` background
- Hover: `theme.accent` + `theme.text_on_accent`
- Triggers file picker dialog via `OpenAbConfigFile` action
- Filename display: max 200px, overflow hidden, shows filename from loaded path or preset name

### Path Presets (internal, for display)
| Value | Label | Description |
|-------|-------|-------------|
| none | None | Pass-through |
| eq | EQ | Parametric EQ |
| gain | Gain | Volume control |
| comp | Compressor | Dynamic compression |
| limiter | Limiter | Peak limiter |
| gate | Gate | Noise gate |
| expander | Expander | Expansion |
| denoiser | Denoiser | Noise reduction |
| loudness | Loudness Comp | Loudness compensation |

## Param Index Mapping
| Index | Parameter | ParamSpec Key |
|-------|-----------|---------------|
| 0 | mix | `mix` (×100) |
| 1 | mix_mode | `mix_mode` (Choice) |
| 2 | selected_path | `selected_path` (Choice) |
| 3 | bypass | `bypass` (Bool) |
| 4 | auto_gain_enabled | `auto_gain_enabled` (Bool) |
| 5 | loudness_type | `loudness_type` (Choice) |
| 6 | max_auto_gain_db | `max_auto_gain_db` (Float) |
| 7 | gain_smoothing_ms | `gain_smoothing_ms` (Float) |
| 8 | mix_transition_ms | `mix_transition_ms` (Float) |
| 9 | path_a_config | `path_a_config` (String) |
| 10 | path_b_config | `path_b_config` (String) |

## Responsive Behavior
- Mix and Auto Gain columns stack horizontally in top row
- Path config section stretches full width below
