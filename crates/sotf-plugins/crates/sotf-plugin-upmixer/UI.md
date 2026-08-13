# Upmixer — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Upmixer" label | Plugin name |
| Left | Speaker config badge | Shows current config (e.g., "5.1", "7.1.4") |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+-----------------------------------------------------------------------+
| GAINS (4 sliders)    | SPATIAL (4 sliders)                            |
| Front Direct         | Stereo Width                                   |
| Front Ambient        | Center Spread                                  |
| Rear Ambient         | Surr. Direct Bleed                             |
| Height Gain          | Rear Late Reflect.                             |
+------+------+------+------+------+------+----------------------------+
| LFE  | Dial | Ambi | Hght | Decor| Conf |                            |
+------+------+------+------+------+------+----------------------------+
|  Tab content: parameters for selected tab                             |
+-----------------------------------------------------------------------+
```

## Main (always visible)

Two groups of 4 vertical sliders side by side.

### GAINS group (left)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Front Direct | gain_front_direct | Vertical slider | f | 0 to 2.0x |
| Front Ambient | gain_front_ambient | Vertical slider | — | 0 to 2.0x |
| Rear Ambient | gain_rear_ambient | Vertical slider | — | 0 to 2.0x |
| Height Gain | height_gain | Vertical slider | — | 0 to 2.0x |

### SPATIAL group (right)

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Stereo Width | stereo_width | Vertical slider | w | 0 to 1.0 |
| Center Spread | center_spread | Vertical slider | — | 0 to 1.0 |
| Surr. Direct Bleed | surround_direct_bleed | Vertical slider | — | 0 to 1.0 |
| Rear Late Reflect. | rear_late_reflection | Vertical slider | — | 0 to 0.5 |

Slider height: 180px

## Tabs (Bottom)

Tab bar: `LFE & Bass | Dialogue | Ambient | Height | Decorrelation | Config`

Selected tab stored in `upmixer_tab` (0=none/closed, 1=LFE, 2=Dialogue, 3=Ambient, 4=Height, 5=Decorrelation).

### Tab: LFE & Bass

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| LFE Gain | lfe_gain | Knob | 0 to 2.0x |
| LFE Cutoff | lfe_cutoff_hz | Knob | 20–180 Hz |
| Subharmonic Synth | enable_subharmonic_synth | Toggle | Enable sub-harmonic synthesis |
| Sub Gain | subharmonic_gain | Knob | 0 to 1.0x |
| Sub Freq | subharmonic_freq_hz | Knob | 20–80 Hz |
| Sub Attack | subharmonic_attack_ms | Knob | 1–100 ms |
| Sub Release | subharmonic_release_ms | Knob | 10–500 ms |

### Tab: Dialogue

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Dialogue Weight | dialogue_weight | Knob | 0 to 1.0 |
| Voice Freq Min | voice_freq_min_hz | Knob | 200–800 Hz |
| Voice Freq Max | voice_freq_max_hz | Knob | 2000–5000 Hz |
| Centroid Weight | dialogue_centroid_weight | Knob | 0 to 1.0 |
| Variance Weight | dialogue_variance_weight | Knob | 0 to 1.0 |
| Coherence Weight | dialogue_coherence_weight | Knob | 0 to 1.0 |

### Tab: Ambient

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Ambient Boost | ambient_boost | Knob | 0.5 to 2.0x |
| Rear Ambient Boost | rear_ambient_boost | Knob | 1.0 to 3.0x |
| Safety Cap | safety_cap_db | Knob | 0 to 3 dB |
| Upmix Crossover | bandpass_hz | Knob | 150–350 Hz |

### Tab: Height

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Height HF Cap | height_hf_cap_hz | Knob | 8000–20000 Hz |
| Height Transient Reduction | height_transient_reduction | Knob | 0 to 1.0 |
| Height Direct Leak | height_direct_leak | Knob | 0 to 0.5 |
| HR Direct | enable_hr_direct | Toggle | Enable high-resolution direct path |
| HR Sharpen | hr_sharpen | Knob | 0 to 1.0 |

### Tab: Decorrelation

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| LFO Rate | decorrelation_lfo_rate_hz | Knob | 0.01–1.0 Hz |
| Velvet Duration | velvet_noise_duration_ms | Knob | 10–100 ms |
| Velvet Density | velvet_noise_density | Knob | 500–5000 |

## Diagnostic (available via bypasses)

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Bypass Decorrelation | bypass_decorrelation | Toggle | Diagnostic |
| Bypass Transient Det. | bypass_transient_detection | Toggle | Diagnostic |
| Bypass All Processing | bypass_all_processing | Toggle | Diagnostic |

## Responsive Behavior
- **Compact:** Tab content collapsed (tabs visible but no expanded panel)
- **Wide:** Tab content expanded with full knob rows

## ParamCategory Mapping

| Parameter | Category | Group |
|-----------|----------|-------|
| Speaker Config | Setup | Output |
| Front Direct/Ambient, Rear Ambient, Height Gain | Primary | Gains |
| LFE Gain, LFE Cutoff, Sub* | Secondary | LFE & Bass |
| Stereo Width, Center Spread, Bandpass | Primary | Spatial |
| HR Direct, HR Sharpen, Ambient Boost | Secondary | Enhancement |
| Surround Direct Bleed, Rear Ambient Boost, Rear Late Refl | Secondary | Routing |
| Height HF Cap, Height Transient, Height Direct Leak | Secondary | Height |
| Dialogue Weight, Voice Freq* | Secondary | Dialogue |
| Decorrelation*, Velvet* | Secondary | Decorrelation |
| Bypass* | Diagnostic | Diagnostic |
