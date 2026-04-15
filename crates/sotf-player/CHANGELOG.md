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



