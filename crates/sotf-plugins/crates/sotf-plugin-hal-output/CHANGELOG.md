# 0.5.12

## Fixes

- Establish a deterministic output-boundary fill equal to the negotiated HAL
  buffer size before publishing engine readiness.
- Report fixed boundary latency as target ring fill plus the Swift HAL device
  latency and safety offset, with each component exposed in typed v2 telemetry.
- Make initialization and transport re-service transactional: each transition
  quiesces readiness once, discards stale queued audio, flushes and re-primes
  the shared ring, and leaves it empty and not ready if priming fails.

## Testing

- Added target-fill/latency telemetry, re-service, transactional priming
  failure, and readiness-transition coverage while retaining allocation-free
  maximum-channel callback checks.

# 0.5.11

## Fixes

- Add a non-realtime lifecycle service for daemon reconnection, shared-memory
  replacement, configuration acknowledgement, readiness, and key rotation.
- Replace the compacting pending vector with a bounded frame-aligned FIFO;
  deterministic newest-frame drops preserve older audio ordering and are
  counted rather than returning an error after partial consumption.
- Move transport diagnostics out of automatable parameters into versioned,
  lossless 64-bit telemetry with requested, written, queued, dropped,
  connection, key, and state information.
- Persist construction-only channel layout in canonical parameters and migrate
  legacy `output_channels` presets.
- Make plain shared-memory writes commit complete interleaved frames only.

## Testing

- Added queue saturation/recovery ordering, lifecycle, key/config state,
  counter-width, serialization migration, invalid writer, zero-allocation
  maximum-channel, and partial-frame ring-wrap tests.

# 0.5.10

## Fixes

- Treat `HalOutputWriter::write` results as frames, fixing false partial-write
  diagnostics for every multichannel stream.
- Validate initialization, transport sample rate/channel count, runtime sample
  rate, checked frame/sample arithmetic, exact input size, and empty sink output.
- Retain frame-aligned unwritten tails in a preallocated queue and retry them
  before newer audio instead of silently discarding partial writes.
- Stop reporting ring capacity as fixed graph latency and mark engine readiness
  across plugin initialization/drop.
- Revalidate transport format-change notifications and reject invalid writer
  frame counts.
- Remove realtime warning logs; expose partial writes as bounded lock-free
  backpressure telemetry instead.

## Testing

- Added an injectable writer seam with cross-platform full/partial write,
  ordering, format, initialization, overflow, sink-output, and latency tests.

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
- `src/lib.rs`: Added `is_connected` and `is_backpressured` diagnostics in
  `parameters()`, `get_parameter()`, and process state updates (`writer.is_connected()` and
  write-success ratio / partial-write state). Added `latency_samples()` to report cached HAL buffer
  frames (`HalOutputWriter::buffer_frames`).

## Deferred

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
