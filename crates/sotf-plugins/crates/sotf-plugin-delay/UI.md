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

Delay and feedback sliders with allpass controls.

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Delay Time | delay_ms | Vertical slider with ticks | d | 0 to the instance maximum; 5000 ms for the standard effect |
| Feedback | feedback | Vertical slider with ticks | f | -95 to 95% |
| Allpass Coeff | allpass_coeff | Knob | a | 0 to 0.99; live changes are smoothed |
| Allpass Feedback | allpass_feedback | Toggle | p | Crossfades over 20 ms |

Slider height: 180px

## Output (Right Column)

Dry/wet mix knob (`mix`, 0 to 100%).

## Diagnostic

`Modulation` tab with LFO rate (`lfo_rate_hz`, 0-20 Hz) and depth
(`lfo_depth_ms`, 0-10 ms) knobs.

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
| LFO Rate | lfo_rate_hz | Secondary | Modulation |
| LFO Depth | lfo_depth_ms | Secondary | Modulation |
| Allpass Coeff | allpass_coeff | Primary | General |
| Allpass Feedback | allpass_feedback | Primary | General |
