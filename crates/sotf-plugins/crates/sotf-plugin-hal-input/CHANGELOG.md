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

## Deferred

- **Sample-rate conversion on mismatch** (review §2): integrating a synchronous/asynchronous
  resampler is a cross-crate change (requires `sotf-plugin-resampler`); deferred.

- **`is_active` / latency reporting** (review §4): feature additions, not bugs; deferred.

- **Zero-fill SIMD** (review §3): `.fill()` is already optimized by the stdlib; no action needed.

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
