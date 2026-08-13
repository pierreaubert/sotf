# sotf-plugin-gain

Simple gain plugin with global and per-channel volume control, using SIMD-optimized processing and smoothed parameter transitions.

## Architecture

```
src/
  lib.rs    -- GainPlugin (ParametricPlugin), GainPluginParams
  params.rs -- Centralized parameter specs (behind params module)
```

Data flow: Global gain OR per-channel gains -> `Smoother` for click-free transitions -> sample-accurate frame processing while moving, or one whole-block SIMD kernel once settled.

**Key types:**

- `GainPlugin` -- Main plugin implementing `ParametricPlugin`. Supports two modes: global gain (single value) or per-channel gains (independent per channel).
- `GainPluginParams` -- Serde config: `gain_db` (global), `smoothing_ms` (transition time), `channel_gains` (per-channel, optional).
- Uses `Smoother` for all gain transitions. Default smoothing time is taken from `params::PARAMS` (currently 10 ms).

## Key Public API

- `GainPlugin::new(channels, gain_db) -> Self` -- Global gain mode with default smoothing (`lib.rs`)
- `GainPlugin::with_smoothing(channels, gain_db, smoothing_ms) -> Self` -- Custom smoothing time (`lib.rs`)
- `GainPlugin::new_per_channel(channel_gains) -> Result<Self, String>` -- Per-channel mode with default smoothing (`lib.rs`)
- `GainPlugin::new_per_channel_with_smoothing(channel_gains, smoothing_ms) -> Result<Self, String>` -- Per-channel mode with custom smoothing (`lib.rs`)
- `GainPlugin::from_params(channels, params) -> Result<Self, String>` -- From JSON config (`lib.rs`)
- `set_gain_db(db)`, `set_gain_linear(g)`, `set_channel_gains(dbs)`, `set_channel_gain_db(ch, db)` -- Runtime parameter updates
- Implements `ParametricPlugin` trait; host-facing `Plugin` is provided by `ParametricPluginAdapter<GainPlugin>`

**Parameters:** `gain_db` (global), `smoothing_ms`, `gain_db_{N}` (per-channel, dynamic).

## Testing

```bash
cargo test -p sotf-plugin-gain
```

## Behavior changes

- The default gain smoothing time changed from 20 ms (old hard-coded value in
  `GainPlugin::new`) to 10 ms (the value declared in `params::PARAMS`).
  `GainPlugin::new` and `GainPlugin::new_per_channel` now use this canonical
  default; use `with_smoothing` / `new_per_channel_with_smoothing` to override it.

## Important Notes

- Calling `set_gain_db()` or `set_gain_linear()` clears per-channel mode (switches back to global).
- The plugin uses deferred sample rate initialization: created with 48kHz placeholder, real rate set in `plugin_initialize()`. Smoother timing adjusts accordingly.
- SIMD paths: settled global blocks use `apply_gain_simd`; settled per-channel
  blocks use `apply_per_channel_gain_simd`. Moving smoothers retain the
  sample-accurate frame path.
- Gain range: -60 dB to +20 dB. Smoothing range: 0 to 100 ms.
