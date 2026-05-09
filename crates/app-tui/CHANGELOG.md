# 0.5.206 (unreleased)

## Room EQ: pass-through support for new strategies

- The TUI's room-eq configuration screen now accepts the two new
  `RoomEqOptimizerConfig` strategy strings introduced in `sotf-player`
  0.5.123 — `"minimax_uncertainty"` for the bootstrap uncertainty path and
  `"continuous_area"` for the continuous listening-area prior. The TUI
  binding is string-based and goes through `to_optimizer_config()`, so the
  new strategies travel through unchanged. Sub-config payloads currently
  default to the library-side defaults; explicit TUI editors for them are
  not yet built.

# 0.5.205

## Room EQ recording: N-by-M capture matrix

- The recording channel list now expands across speakers, selected
  input mics, and measurement positions, so TUI captures can represent
  the full acoustic transfer matrix instead of one mono take per
  speaker.
- The configuration step now exposes CTC matrix strategy and loopback
  input fields. Raw-sweep mode writes the reference sweep, records a
  loopback WAV alongside captures, and preserves the full CTC config for
  Room EQ processing.
- Recording saves now group those captures back into per-speaker
  measurements and persist measured CTC matrix metadata for Room EQ.

## Room EQ: expose Bayesian optimizer controls

- Added `autoeq:bo` to the Room EQ configure step and exposed BO
  hot-start samples, batch size, posterior-std stop threshold,
  acquisition, and qEHVI toggles in the TUI field list. The configure
  field navigation count was expanded to include the new controls.

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
