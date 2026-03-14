# Gain — UI Specification

## Layout Mode
custom (minimal)

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Gain" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close |

## Config (Left Column)

Not used — no setup parameters.

## Main (Center, always visible)

### Global Mode (default)

Single centered knob.

| Parameter | engine_key | Control | Shortcut | Notes |
|-----------|-----------|---------|----------|-------|
| Gain | gain_db | Knob (large) | g | -60 to +20 dB, default 0. Center = unity |

The knob should be visually larger than standard knobs (1.5x) since it's the only control.

### Per-Channel Mode

When per-channel gains are configured, show one knob per channel in a horizontal row. Each knob labeled "Ch 1", "Ch 2", etc. The global gain knob remains visible above the per-channel row.

| Parameter | engine_key | Control | Notes |
|-----------|-----------|---------|-------|
| Gain Ch N | gain_db_{N} | Knob (standard) | One per channel, -60 to +20 dB |

## Output (Right Column)

Not used — no output parameters or meters.

## Diagnostic

Not used.

## Tabs

Not used.

## Responsive Behavior
- **Compact:** Knob only, no label below
- **Wide:** Knob with value readout and range indicator

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Gain | gain_db | Primary | General |
| Gain Ch N | gain_db_{N} | Primary | Channels |
