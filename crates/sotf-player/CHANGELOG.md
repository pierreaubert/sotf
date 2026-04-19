# 0.5.122

## Room EQ: follow autoeq 0.4.30 unified `target_response` (breaking)

- `RoomEqConfig` now carries a single `target_response:
  TargetResponseUiConfig` field in place of the legacy
  `target_tilt` + `broadband_target_matching` pair. The new struct
  mirrors `autoeq::roomeq::TargetResponseConfig` — target shape
  (`flat` / `harman` / `custom` / `file` / `from_measurement`),
  preference shelves (bass + treble), and the
  `broadband_precorrection` toggle.
- The serialized wire format changes accordingly. Saved user
  room-EQ configurations from 0.5.121 and earlier will not
  deserialize without manual migration because the two old fields
  no longer exist on the struct. This matches the autoeq config
  schema bump (1.3.0 → 2.0.0).
- `multi_speaker` no longer plumbs `target_tilt` through the
  optimizer configuration — callers are expected to set
  `target_response` directly.

# 0.5.121

## QueueController: gate file-existence validation behind `testing` feature

- `QueueController::add_album` and `play_album_now` now skip
  `validate_album_has_files` when the `testing` feature is enabled.
  Production builds keep the validation; integration tests in app-gpui
  (which use synthetic `/test/*.flac` paths) no longer have albums
  silently rejected with their `Result` discarded.
- Fixes 16 failing app-gpui lifecycle tests
  (`queue_sequences::*`, `playback_sequences::*`).

# 0.5.120

## EqFilterConfig dedup re-audit (no code change)

- The 0.5.118 CHANGELOG deferred unifying
  `room_eq_types::EqFilterConfig` with the canonical `PeqFilter`
  pending "a serde-alias migration". Re-audit shows
  `EqFilterConfig` is **not** a stripped autoeq record — it is the
  runtime/UI-side canonical matching `sotf-engine::EQFilter`
  (`filter_type` / `frequency` / `q` / `gain_db`), which is the
  wire format the engine plugin loader deserializes. `PeqFilter`
  and its aliases (`HeadphoneEqBiquad`, `SpinoramaBiquad`) use the
  autoeq-side convention (`filter_type` / `freq` / `q` / `db_gain`).
  The two conventions exist by design and the codebase already has
  an explicit bridge (see `app-tui::events::conf_roomeq` mapping
  `b.freq → frequency` and `b.db_gain → gain_db`).
- Adding `#[serde(alias = "frequency")]` + `#[serde(alias = "gain_db")]`
  to `PeqFilter` would make deserialization tolerant but
  serialization would still emit `freq` / `db_gain`, silently
  breaking any consumer (including `sotf-engine::EQFilter`) that
  parses JSON by field name. `#[serde(rename)]` flips the output
  key — equally breaking. There is no painless serde-only
  unification, so the two structs stay separate. Any future
  consolidation needs a coordinated change across both wire formats
  plus a deprecation window, not a quiet alias.

# 0.5.119

## Bug fixes

- `SpeakerOptimizationResult`: the seven CEA2034 spinorama curves
  (`on_axis_curve`, `lw_curve`, `er_curve`, `sp_curve`, `pir_curve`,
  `er_di_curve`, `sp_di_curve`) are now populated with empty
  `Vec<f64>` when spin data is absent, instead of zero-filled vectors
  sized to `frequencies.len()`. Consumers in `speaker_graphs.rs`
  (e.g. `render_spinorama_main_response_plot`,
  `render_tonal_balance_plot`) and `SpinoramaCurves::is_valid` /
  `has_pir` already use `is_empty()` as an absence sentinel — the
  previous zero-filled vectors silently passed those checks and
  caused misleading flat-line plots at 0 dB. Affects both the
  single-speaker `From<SpeakerOptResult>` conversion and the
  multi-speaker `to_speaker_results` builder.

## Tier-1 dedup re-audit (no code change)

- Re-audit of `sotf-player::room_eq_types` against
  `autoeq::roomeq::types::config::*` found the six candidates
  (`TargetTiltConfig`, `ExcursionProtectionConfig`,
  `SchroederSplitConfig`, `PhaseAlignmentConfig`, `MultiSeatConfig`,
  `BroadbandTargetMatchingConfig`) are **not** trivial stripped
  copies: each pair diverges in at least one of default values,
  field names (`slope` vs `slope_db_per_octave`), field types
  (`String` vs enum, `f64` vs `Option<f64>`), field nesting
  (flat vs nested `LowFreqFilterConfig` / `HighFreqFilterConfig`),
  or field count (extra `enabled` flag on the UI side that the
  backend represents as `Option<T>`). They stay separate by design;
  any future unification needs a serde-alias migration plus
  behavioural review, not a blind `pub use`.

# 0.5.118

## Code changes

- Collapsed the two identical stripped biquad records
  (`HeadphoneEqBiquad` and `SpinoramaBiquad`) into a single canonical
  `PeqFilter` struct in the new `peq_filter` module. Both names remain
  exported as type aliases, so all GPUI/TUI call sites keep compiling
  unchanged. The duplicated 4-field struct definition with identical
  derives and identical serde representation is gone.
- `room_eq_types::EqFilterConfig` is **not** aliased here because it
  uses the `frequency` / `gain_db` naming convention instead of the
  autoeq-shaped `freq` / `db_gain` pair, so unifying it requires a
  separate serde-alias migration.

# 0.5.117

## Features

- Simple Wizard: all speaker tiers (NearField, MidField, FarField) now
  use `from_measurement` target tilt instead of hardcoded slopes. The
  optimizer derives the target slope from the measurement curve at
  optimization time, preserving the speaker's natural response.
- `import_from_backend()` handles the new `TiltType::FromMeasurement`
  variant.

# 0.5.116

## Code changes

- Extracted `parse_eq_filters_from_json` from inline closure to a public function in `room_eq_types` — now testable and reusable across all frontends

## Tests

- Added 9 unit tests for `parse_eq_filters_from_json` (autoeq/engine JSON key formats, all filter types, defaults, edge cases)
- Added 4 unit tests for `DspChainOutput::is_rack_compatible` (no drivers, with drivers, mixed, empty)
- Added 6 unit tests for save-to-rack plugin graph operations (insert EQ, update existing, per-channel config serialization, disabled EQ exclusion)

Bug fixed:
- added check when the album art or the music files are not currently available (classical example is music is on an external drive currently not mounted)
- forced all paths to be loaded through cleanup routines (affect plugins loading)
- preset filename traversal now extract only the filename component
- replaced unwrap() on map lookups with expect() containing a clear diagnostic message
- improved migration error message from "Unknown" to "Unsupported ... (minimum: N)".
- fixed apply to rack and apply to graph hosts (lots of changes there)



