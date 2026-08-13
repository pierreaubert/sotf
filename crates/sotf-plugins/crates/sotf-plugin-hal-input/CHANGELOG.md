# 0.5.87

## HAL Input review closure

- Consolidate serialized and UI channel configuration into typed
  `input_channels: usize`, accepting legacy `channels` JSON while rejecting
  fractional/non-numeric values; expose it only as build-time configuration.
- Move dynamic diagnostics out of automatable parameters into lossless typed
  telemetry with `u64` underrun/missing-frame and connection/recovery/format
  generations, negotiated format, capacity/fill, and explicit error kind.
- Add a non-realtime `refresh_transport` control operation that opens and
  validates a replacement shared-memory mapping and cipher before activation;
  the realtime callback only raises recovery generations and emits silence.
- Validate callback sample rate and initialization before consuming transport,
  preserve the active reader when replacement format validation fails, and
  distinguish disconnect, cipher reload, format mismatch, corruption, and
  underrun states.
- Add focused configuration migration, format/recovery generation, failed
  replacement, context-rate, missing-frame, 1–16 channel, and zero-allocation
  callback tests with the real macOS HAL feature enabled.

# 0.5.86

## Fixes

- Treat `HalInputReader::read` results as frames and convert to interleaved
  sample indices with checked multiplication, preserving complete and partial
  reads for 1–16 channels.
- Validate configured channels and sample rate against the negotiated shared
  format during initialization and every callback, failing before consuming a
  changed or corrupt transport format.
- Count complete starvation after the connected stream is armed while keeping
  startup/disconnected silence distinct, and remove all logging from the audio
  callback.
- Report conservative zero graph latency instead of shared-buffer capacity,
  reject nonempty source input and malformed/overflowing buffers, and replace
  unsafe tail clearing with optimized safe slice filling.

## Testing

- Added an injectable reader contract with deterministic full, partial, empty,
  oversized, channel-change, and rate-negotiation coverage on every platform.

# 0.5.85

## Fixes

- `parameters()` now reports the constructed `input_channels` value in parameter metadata instead of
  always advertising stereo. This keeps the runtime parameter list consistent with `get_parameter()`
  for multichannel HAL input instances.

# 0.5.84

## Fixes (from code review)

- **Parameter name mismatch** (`lib.rs:126`, `lib.rs:181`, `lib.rs:199`): `Plugin::parameters()`,
  `set_parameter()`, and `get_parameter()` all used the stale id `"channels"` while `params.rs`
  and the `PluginParamDef` implementation used `"input_channels"`.  The mismatch broke the
  parameter-bridge mapping (queries for `"input_channels"` returned `None`; writes were silently
  routed to the wrong id).  All three sites now use `"input_channels"`.

- **Structural parameter mutability** (`lib.rs:set_parameter`): `set_parameter("input_channels")`
  previously let callers silently mutate `self.channels` without reinitializing the
  `HalInputReader`.  The reader was constructed with a fixed channel count, so the mismatch
  caused buffer-size errors and channel misalignment.  The parameter is now read-only
  post-construction; `set_parameter` returns `Err` with an actionable message directing the
  caller to construct a new plugin instance.

- **Sample-rate mismatch** (`lib.rs:initialize`): `initialize()` logged a warning but returned
  `Ok(())` when the HAL native rate differed from the engine rate.  Playing at mismatched rates
  produces incorrect pitch and duration.  `initialize()` now returns `Err` so the host can fail
  fast and direct the user to configure the HAL device or insert a resampler.

- **Underrun counter inflated by empty reads** (`lib.rs:process`): Any read shorter than the
  output buffer was counted as an underrun, including fully-empty reads (0 samples) that are
  normal during device startup or device switching.  The counter now only increments on partial
  reads (`samples_read > 0 && samples_read < output.len()`).

- **Underrun tail zeroing efficiency** (`lib.rs:process`): switched from `slice.fill(0.0)` to
  `std::ptr::write_bytes` for the under-run tail.  This keeps the same behavior while using a
  contiguous memory clear path that is efficient for large tails.

- **HAL diagnostics and latency** (`lib.rs:parameters`, `lib.rs:get_parameter`,
  `lib.rs:process`, `lib.rs:latency_samples`): added `is_connected` diagnostic
  parameter and cached shared-memory buffer frame count as latency (frames), updated at
  each successful HAL read using `HalInputReader::is_connected()` and `buffer_frames()`.

## Deferred

- **Sample-rate conversion on mismatch** (review §2): integrating a synchronous/asynchronous
  resampler is a cross-crate change (requires `sotf-plugin-resampler`); deferred.


---

# 0.5.83

## New

- Added missing parameters for new plugins
- Added missing autogain to some plugins
- Initial merge of the new components into the app-gpui

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 2)
- Massive update to plugins, see individual markdown plan for details
