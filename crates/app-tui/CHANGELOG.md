# 0.5.205

## Room EQ: rename `target_tilt` → `target_response` (breaking)

- `events::conf_roomeq` and the Room EQ draw path (`ui::draw_roomeq`)
  now consume `sotf_audio_player::TargetResponseUiConfig` in place
  of the removed `target_tilt` / `broadband_target_matching` pair.
  The `"from_measurement"` shape is still honoured — it now maps
  to `TargetShape::FromMeasurement` on the autoeq side, surfaced
  via the unified `target_response` boundary.
- Field rename propagates through the Simple Wizard form and the
  optimization request builder. No user-visible behaviour change
  beyond the config schema.

# 0.5.204

## Features

- Room EQ: config builder now maps `"from_measurement"` tilt type to
  `TiltType::FromMeasurement`, enabling measurement-derived target tilt
  from the Simple Wizard.
