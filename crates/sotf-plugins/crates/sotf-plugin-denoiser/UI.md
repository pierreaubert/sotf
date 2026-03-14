# Denoiser — UI Specification

## Layout Mode
custom

## ASCII Layout

```
+---------------------------------------------------------------------------------+
| menu ui                                                      | menu preset|T S X|
+---------------------------------------------------------------------------------+
| REDUCTION    | TIMING     | DETECTION   | ADVANCED    | MODES       | HISS      | SPEC SUB  | PROFILE   |
| [Reduction]  | [Attack]   | [Smoothing] | [MCRA S]    | [Low Lat]   | [Enabled] | [Enabled] | [Learn]   |
| [Floor]      | [Release]  | [Crack Sns] | [MCRA P]    | [Polyphonic]| [Thresh]  | [Oversub] | [Use Prof]|
|  (sliders)   |  (sliders) | [Transp]    | [MCRA Win]  | [DD SNR]    | [Freq]    | [Floor]   | [Clear]   |
|              |            |             | [MCRA Delta]| [Masking]   | [Strength]|           |           |
|              |            |             |             | [Transient] |           |           |           |
|              |            |             |             | [Spec Smth] |           |           |           |
|              |            |             |             | [Temp Smth] |           |           |           |
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
| Reduction | Vertical slider (200px) | 0 | 0–40 | dB | r |
| Floor | Vertical slider (200px) | 1 | -60 to -10 | dB | f |

### Column 2: TIMING
Section title: "TIMING"

| Parameter | Control | Param Index | Range | Unit | Hotkey |
|-----------|---------|-------------|-------|------|--------|
| Attack | Vertical slider (200px) | 3 | 0.1–100 | ms | a |
| Release | Vertical slider (200px) | 4 | 10–500 | ms | e |

### Column 3: DETECTION
Section title: "DETECTION"

| Parameter | Control | Param Index | Range | Unit | Hotkey |
|-----------|---------|-------------|-------|------|--------|
| Smoothing | Knob | 2 | 0–99 | % (×100) | s |
| Crack Sens | Knob | 7 | 1–100 | — | — |
| Transparency | Knob | 12 | 0–100 | % (×100) | t |

### Column 4: ADVANCED
Section title: "ADVANCED"

| Parameter | Control | Param Index | Range | Unit |
|-----------|---------|-------------|-------|------|
| MCRA S | Knob | 8 | 0.5–0.99 | — |
| MCRA P | Knob | 9 | 0.1–0.99 | — |
| MCRA Window | Knob | 10 | 10–200 | frames |
| MCRA Delta | Knob | 11 | 1–20 | — |

### Column 5: MODES
Section title: "MODES"

| Parameter | Control | Param Index | Notes |
|-----------|---------|-------------|-------|
| Low Lat | Toggle button | 5 | Low latency mode |
| Polyphonic | Toggle button | 6 | Polyphonic pitch detection |
| DD SNR | Toggle button | 13 | Decision-directed SNR |
| Masking | Toggle button | 15 | Psychoacoustic masking |
| Transient | Toggle button | 16 | Transient detection |
| Spec Smooth | Toggle button | 17 | Spectral smoothing |
| Temp Smooth | Toggle button | 18 | Temporal smoothing |

Each toggle has a text label below it in `text_xs`, `theme.text_muted`.

### Column 6: HISS
Section title: "HISS"

| Parameter | Control | Param Index | Range | Unit |
|-----------|---------|-------------|-------|------|
| Enabled | Toggle button | 19 | — | — |
| Threshold | Knob | 20 | -60 to -10 | dB |
| Frequency | Knob | 21 | 1000–16000 | Hz |
| Strength | Knob | 22 | 0–100 | % (×100) |

### Column 7: SPEC SUB
Section title: "SPEC SUB"

| Parameter | Control | Param Index | Range | Unit |
|-----------|---------|-------------|-------|------|
| Enabled | Toggle button | 23 | — | — |
| Oversub | Knob | 24 | 0.5–6.0 | — |
| Floor | Knob | 25 | 0.001–0.5 | — |

### Column 8: PROFILE
Section title: "PROFILE"

| Parameter | Control | Param Index | Notes |
|-----------|---------|-------------|-------|
| Learn | Toggle button | 26 | Start noise profile capture |
| Use Prof | Toggle button | 27 | Use captured profile |
| Clear | Toggle button | 28 | Clear captured profile |

Each toggle has a text label below it.

## Param Index Mapping
| Index | Parameter | ParamSpec Key |
|-------|-----------|---------------|
| 0 | reduction_db | `reduction_db` |
| 1 | floor_db | `floor_db` |
| 2 | smoothing | `smoothing` (×100) |
| 3 | attack_ms | `attack_ms` |
| 4 | release_ms | `release_ms` |
| 5 | low_latency | `low_latency` (Bool) |
| 6 | polyphonic_detection | `polyphonic_detection` (Bool) |
| 7 | crack_sensitivity | `crack_sensitivity` |
| 8 | mcra_alpha_s | `mcra_alpha_s` |
| 9 | mcra_alpha_p | `mcra_alpha_p` |
| 10 | mcra_l | `mcra_l` (Int) |
| 11 | mcra_delta | `mcra_delta` |
| 12 | transparency | `transparency` (×100) |
| 13 | dd_enabled | `dd_enabled` (Bool) |
| 14 | dd_alpha | `dd_alpha` |
| 15 | psychoacoustic_masking | `psychoacoustic_masking` (Bool) |
| 16 | transient_enabled | `transient_enabled` (Bool) |
| 17 | spectral_smoothing_enabled | `spectral_smoothing_enabled` (Bool) |
| 18 | temporal_smoothing_enabled | `temporal_smoothing_enabled` (Bool) |
| 19 | hiss_enabled | `hiss_enabled` (Bool) |
| 20 | hiss_threshold_db | `hiss_threshold_db` |
| 21 | hiss_frequency_hz | `hiss_frequency_hz` |
| 22 | hiss_strength | `hiss_strength` (×100) |
| 23 | spectral_sub_enabled | `spectral_sub_enabled` (Bool) |
| 24 | spectral_sub_alpha | `spectral_sub_alpha` |
| 25 | spectral_sub_beta | `spectral_sub_beta` |
| 26 | learn_noise | `learn_noise` (Bool) |
| 27 | use_captured_profile | `use_captured_profile` (Bool) |
| 28 | clear_profile | `clear_profile` (Bool) |

## Responsive Behavior
- All 8 columns are in a single horizontal row
- Horizontal scroll if width is insufficient
- Slider height fixed at 200px for Reduction/Timing columns
