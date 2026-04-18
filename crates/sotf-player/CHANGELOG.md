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



