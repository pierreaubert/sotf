# Denoiser UI Specification

## Layout Mode
custom

## ASCII Layout

```
+---------------------------------------------------------------------------------+
| menu ui                                                      | menu preset|T S X|
+---------------------------------------------------------------------------------+
| REDUCTION    | TIMING     | MCRA       | SNR        | SMOOTHING  | SPEC SUB  | PROFILE   | MODES      |
| [Reduction]  | [Attack]   | [MCRA S]   | [Transp]   | [Masking]  | [Enabled] | [Learn]   | [Low Lat]  |
| [Floor]      | [Release]  | [MCRA P]   | [DD SNR]   | [Spec Smth]| [Oversub] | [Use Prof]| [Polyph]   |
| [Smoothing]  |            | [MCRA Win] | [DD Alpha] | [Temp Smth]| [Floor]   | [Clear]   | [Formant]  |
|              |            | [MCRA Del] |            |            |           |           | [MultiRes] |
|              |            |            |            |            |           |           | [H/P]      |
|              |            |            |            |            |           |           | [Spatial]  |
+---------------------------------------------------------------------------------+
```

## Menu Bar
| Position | Element | Behavior |
|----------|---------|----------|
| Right | Preset picker | Standard |
| Right | T S X | Toggle/Solo/Close |

## Main Layout
8 columns side by side with `gap_6`, `items_start` alignment.

### Column 1: REDUCTION
Section title: "REDUCTION"

| Parameter | Control | Param Index | Range | Unit | Hotkey |
|-----------|---------|-------------|-------|------|--------|
| Reduction | Vertical slider (200px) | 0 | 0-40 | dB | r |
| Floor | Vertical slider (200px) | 1 | -60 to -10 | dB | f |
| Smoothing | Knob | 2 | 0-99 | % (x100) | s |

### Column 2: TIMING
Section title: "TIMING"

| Parameter | Control | Param Index | Range | Unit | Hotkey |
|-----------|---------|-------------|-------|------|--------|
| Attack | Vertical slider (200px) | 3 | 0.1-100 | ms | a |
| Release | Vertical slider (200px) | 4 | 10-500 | ms | e |

### Column 3: MCRA
Section title: "MCRA"

| Parameter | Control | Param Index | Range | Unit |
|-----------|---------|-------------|-------|------|
| MCRA S | Knob | 7 | 0.5-0.99 | - |
| MCRA P | Knob | 8 | 0.1-0.99 | - |
| MCRA Window | Knob | 9 | 10-200 | frames |
| MCRA Delta | Knob | 10 | 1-20 | - |

### Column 4: SNR
Section title: "SNR"

| Parameter | Control | Param Index | Range | Unit |
|-----------|---------|-------------|-------|------|
| Transparency | Knob | 11 | 0-100 | % (x100) |
| DD SNR | Toggle button | 12 | - | - |
| DD Alpha | Knob | 13 | 0.9-0.999 | - |

### Column 5: SMOOTHING
Section title: "SMOOTHING"

| Parameter | Control | Param Index | Notes |
|-----------|---------|-------------|-------|
| Masking | Toggle button | 14 | Psychoacoustic masking |
| Spec Smooth | Toggle button | 15 | Spectral smoothing |
| Temp Smooth | Toggle button | 16 | Temporal smoothing |

### Column 6: SPEC SUB
Section title: "SPEC SUB"

| Parameter | Control | Param Index | Range | Unit |
|-----------|---------|-------------|-------|------|
| Enabled | Toggle button | 17 | - | - |
| Oversub | Knob | 18 | 0.5-6.0 | - |
| Floor | Knob | 19 | 0.001-0.5 | - |

### Column 7: PROFILE
Section title: "PROFILE"

| Parameter | Control | Param Index | Notes |
|-----------|---------|-------------|-------|
| Learn | Toggle button | 20 | Start noise profile capture |
| Use Prof | Toggle button | 21 | Use captured profile |
| Clear | Toggle button | 22 | Clear captured profile |

### Column 8: MODES
Section title: "MODES"

| Parameter | Control | Param Index | Notes |
|-----------|---------|-------------|-------|
| Low Lat | Toggle button | 5 | Low latency mode |
| Polyphonic | Toggle button | 6 | Polyphonic pitch detection |
| Formant | Toggle button | 23 | Preserve spectral peaks |
| Formant Strength | Knob | 24 | Formant preservation strength |
| MultiRes | Toggle button | 25 | Multi-resolution spectral denoising |
| Harm/Perc | Toggle button | 26 | Harmonic/percussive denoising |
| Spatial | Toggle button | 27 | Spatial denoising |
| Spatial Strength | Knob | 28 | Spatial processing strength |

Each toggle has a text label below it in `text_xs`, `theme.text_muted`.

## Param Index Mapping
| Index | Parameter | ParamSpec Key |
|-------|-----------|---------------|
| 0 | reduction_db | `reduction_db` |
| 1 | floor_db | `floor_db` |
| 2 | smoothing | `smoothing` (x100) |
| 3 | attack_ms | `attack_ms` |
| 4 | release_ms | `release_ms` |
| 5 | low_latency | `low_latency` (Bool) |
| 6 | polyphonic_detection | `polyphonic_detection` (Bool) |
| 7 | mcra_alpha_s | `mcra_alpha_s` |
| 8 | mcra_alpha_p | `mcra_alpha_p` |
| 9 | mcra_l | `mcra_l` (Int) |
| 10 | mcra_delta | `mcra_delta` |
| 11 | transparency | `transparency` (x100) |
| 12 | dd_enabled | `dd_enabled` (Bool) |
| 13 | dd_alpha | `dd_alpha` |
| 14 | psychoacoustic_masking | `psychoacoustic_masking` (Bool) |
| 15 | spectral_smoothing_enabled | `spectral_smoothing_enabled` (Bool) |
| 16 | temporal_smoothing_enabled | `temporal_smoothing_enabled` (Bool) |
| 17 | spectral_sub_enabled | `spectral_sub_enabled` (Bool) |
| 18 | spectral_sub_alpha | `spectral_sub_alpha` |
| 19 | spectral_sub_beta | `spectral_sub_beta` |
| 20 | learn_noise | `learn_noise` (Bool) |
| 21 | use_captured_profile | `use_captured_profile` (Bool) |
| 22 | clear_profile | `clear_profile` (Bool) |
| 23 | formant_preservation | `formant_preservation` (Bool) |
| 24 | formant_strength | `formant_strength` |
| 25 | multi_resolution | `multi_resolution` (Bool) |
| 26 | harmonic_percussive | `harmonic_percussive` (Bool) |
| 27 | spatial_denoise | `spatial_denoise` (Bool) |
| 28 | spatial_strength | `spatial_strength` |

## Responsive Behavior
- All 8 columns are in a single horizontal row
- Horizontal scroll if width is insufficient
- Slider height fixed at 200px for Reduction/Timing columns
