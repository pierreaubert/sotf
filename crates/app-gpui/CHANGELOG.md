# 0.5.16

## Code changes

### Room EQ: dropped lossy DSP-chain conversion in Step-4 optimiser

- `step_4_optimise.rs` previously rebuilt the optimiser's `DspChainOutput`
  field-by-field into a stripped-down sotf-player copy, losing
  `initial_curve`, `final_curve`, `eq_response`, `target_curve`,
  `pre_ir`, `post_ir`, `loss_type`, `inter_channel_deviation`, and
  `epa_per_channel` on every run. Now passes
  `room_result.to_dsp_chain_output()` straight through — sotf-player's
  `DspChainOutput` is a re-export of the autoeq type, so no data loss.
- `room_eq_config_tests.rs` and `room_eq_apply_tests.rs` migrated to
  the rich `ChannelDspChain` / `DspChainOutput` shape via local
  `chain(...)` / `driver(...)` / `output(...)` helpers so the `None`
  soup of optional curve/IR fields doesn't pollute every test site.

## Fixes

### Room EQ: Schroeder split disabled for stereo without subwoofer

- `apply_smart_defaults` now only enables Schroeder split when a
  subwoofer is present. For 2.0 stereo, a single-pass optimizer across
  the full frequency range is more effective — the Schroeder split was
  fragmenting the optimization and preventing filters from landing on
  bass room modes.

### Room EQ: decomposed correction enabled

- `to_room_config()` now passes `DecomposedCorrectionSerdeConfig::default()`
  instead of `None`. This enables room mode detection and seeds the
  optimizer's initial guesses with detected modes at correct frequencies.

# 0.5.15

## Features

- Room EQ: `to_room_config()` now maps `"from_measurement"` tilt type
  to `TiltType::FromMeasurement`, enabling measurement-derived target
  tilt from the Simple Wizard.

# 0.5.14

## Fixes

### Room EQ "save to rack" — EQ filters were applied flat (all 0 dB)

- The `parse_filters` helper in `apply_room_eq_to_player` expected JSON
  keys `"frequency"` and `"gain_db"`, but the autoeq optimizer outputs
  `"freq"` and `"db_gain"`. Every filter silently fell through to the
  defaults (freq=1000 Hz, gain=0.0 dB), producing a flat EQ curve.
  The parser now accepts both key forms via `.or_else()` fallback.

### Room EQ "save to rack" — workflow graph canvas not refreshed

- The `WorkflowCanvas` entity was created once and never invalidated.
  When plugins were added or removed (from room EQ, spinorama EQ,
  headphone EQ, preset loading, or manual editing), the graph view
  kept showing the stale topology. Fixed by setting
  `workflow_canvas = None` on every structural plugin update so the
  canvas rebuilds on the next render.

### Room EQ "save to rack" — filter parser extracted for reuse

- The inline `parse_filters` closure in `apply_room_eq_to_player` was
  extracted to `sotf_audio_player::room_eq_types::parse_eq_filters_from_json`,
  making it testable and available to all frontends.

## Tests

- Added 9 integration tests for the save-to-rack flow (stereo, 5.1 surround,
  update existing EQ, no filters, missing channels, merged EQ plugins,
  non-EQ plugins skipped, case-insensitive plugin type, multi-driver
  rack incompatibility)

## Fixes

### Room EQ "save to rack" — silent failure on insert_plugin error

- `insert_plugin` returned `Result` but the error was discarded with
  `let _ =`. If graph insertion failed (e.g. non-linear topology), the
  code silently continued and tried to configure a plugin at the wrong
  index. Now uses `match` with proper error logging.

### Recording evaluation — magnitude plot was vertically flipped

- The "MAGNITUDE (dB)" chart in the recording evaluation screen
  (`components/recording/evaluating.rs::render_magnitude_chart`) was
  rendering every measured curve upside-down. A stray unary minus in
  the per-point normalization (`-(mag - normalization_offset)`) was
  flipping the sign of the offset-relative magnitude, so real room
  modes appeared as nulls and real cancellations appeared as peaks.
  The formula is now `mag - normalization_offset` and the chart
  matches both the raw `L.wav` / `R.wav` Welch PSD and the curves
  stored in `dsp.json` (which are also what `scripts/display-roomeq.py`
  has been displaying correctly all along).
- Phase, group-delay, distortion, RT60, clarity, impulse-response, and
  spectrogram charts were checked in the same pass and do *not* have
  the same bug — they use straight `mag - offset` or no normalization
  at all.
