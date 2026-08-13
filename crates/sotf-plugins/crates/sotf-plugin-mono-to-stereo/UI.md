# Mono to Stereo — UI Specification

## Layout Mode
hybrid (config column + main + tabs)

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Mono to Stereo" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close |

## Config (Left Column, always visible)

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|

Width: 100px fixed

## Main (Center, always visible)

Single primary control — the width slider.

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Width | stereo_width | Vertical slider (large) | w | 0.0 to 1.0. Center position, visually prominent |

The width slider should be taller than standard (220px) since it's the primary interaction point.

## Output (Right Column)

Not used.

## Diagnostic

Not used.

## Tabs (Bottom)

### Tab: Advanced

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Haas Delay | haas_delay_ms | Knob | 0 to 5 ms |
| Decor Low | decor_low_hz | Knob | 100 to 500 Hz. Lower bound of decorrelation band |
| Decor High | decor_high_hz | Knob | 1000 to 5000 Hz. Upper bound of decorrelation band |
| Frequency Dependent | freq_dependent | Toggle | Localize phase rotation to the decorrelation band |

Decor Low, Decor High, and Frequency Dependent are structural controls: applying them rebuilds
the plugin graph. Haas Delay and Width remain live controls.

## Responsive Behavior
- **Compact:** Advanced tab content hidden, only Config + Width visible
- **Wide:** Advanced tab content can become a right column instead of a tab

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Width | stereo_width | Primary | General |
| Haas Delay | haas_delay_ms | Secondary("Advanced") | General |
| Decor Low | decor_low_hz | Secondary("Advanced") | General |
| Decor High | decor_high_hz | Secondary("Advanced") | General |
