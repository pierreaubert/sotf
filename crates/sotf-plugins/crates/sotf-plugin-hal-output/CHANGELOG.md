# 0.5.9

## Fixes (from code review)

- `src/params.rs`: Removed stale `output_channels` entry from `PARAMS` / `LAYOUT` / `Params`.
  The channel count is fixed at construction time; exposing it as a settable runtime parameter
  was inconsistent with the plugin's `set_parameter` always returning `Err`. Old presets that
  serialised `output_channels` are silently accepted (serde ignores unknown fields by default).
- `src/lib.rs`: Renamed the `buffer_fill_level` diagnostic parameter to `write_success_ratio`
  (and updated the field name, description, and all `get_parameter` / `parameters()` references).
  The metric was never a ring-buffer fill level — it measures the write-acceptance ratio for
  the current block (100 % = all samples accepted).
- `src/lib.rs`: Rate-limited partial-write log warnings to the first underrun and then every
  1 000 blocks, preventing log floods when the HAL writer is continuously back-pressured.

## Deferred

- Adding a `running`/`connected` diagnostic parameter requires API changes to `driver_hal`
  (`HalOutputWriter`) — deferred as a cross-crate refactor.
- Graceful back-pressure signalling to the engine requires engine-layer changes — deferred.
- Mock `HalOutputWriter` for cross-platform integration tests — deferred (requires a new
  trait abstraction in `driver_hal`, cross-crate change).

# 0.5.8

## New

- Debugging of the new plugin features
- Added missing parameters for new plugins

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 2)
- Massive update to plugins, see individual markdown plan for details
