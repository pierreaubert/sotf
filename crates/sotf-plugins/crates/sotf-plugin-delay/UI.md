# Delay — UI Specification

## Layout Mode
custom (simple)

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Delay" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close |

## Config (Left Column)

Not used — no setup parameters.

## Main (Center, always visible)

Three vertical sliders side-by-side, horizontally centered.

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Delay Time | delay_ms | Vertical slider with ticks | d | 0.1 to 5000 ms. Logarithmic scale recommended for display |
| Feedback | feedback | Vertical slider with ticks | f | 0 to 95% (display_scale: 100.0) |
| Mix | mix | Vertical slider with ticks | x | 0 to 100% (display_scale: 100.0) |

Slider height: 180px

## Output (Right Column)

Not used — no output meters.

## Diagnostic

Not used.

## Tabs

Not used.

## Responsive Behavior
- **Compact:** Sliders shortened to 120px, labels abbreviated
- **Wide:** Sliders at full 180px height with tick marks and value readout

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Delay Time | delay_ms | Primary | General |
| Feedback | feedback | Primary | General |
| Mix | mix | Primary | General |
