# sotf-plugin-channel-mute-solo

Per-channel mute, solo, and dim plugin with smoothed gain transitions for click-free operation.

## Architecture

```
src/
  lib.rs    -- ChannelMuteSoloPlugin (InPlacePlugin), ChannelState, ChannelMuteSoloParams
  params.rs -- Centralized parameter specs
```

Data flow: Per-channel state (muted/soloed/dimmed) -> target gain computation -> `Smoother` per channel (default 5ms fade) -> SIMD per-channel gain application.

**Key types:**

- `ChannelMuteSoloPlugin` -- Main plugin implementing `InPlacePlugin`. Per-channel `Smoother` instances for click-free transitions.
- `ChannelState` -- Per-channel flags: `muted`, `soloed`, `dimmed`. Serde-serializable.
- `ChannelMuteSoloParams` -- Config: `enabled`, `channel_states`, `dim_gain_db`, `fade_ms`.

**Priority rules:** Solo takes priority over everything. If any channel is soloed, only soloed channels produce output. Mute takes priority over dim.

## Key Public API

- `ChannelMuteSoloPlugin::new(channels, enabled) -> Self` (`lib.rs`)
- `ChannelMuteSoloPlugin::from_params(channels, params) -> Self` (`lib.rs`)
- `set_channel_state(channel, muted, soloed, dimmed)` -- Per-channel control
- `set_channel_states(states)` -- Bulk update
- `set_dim_gain_db(db)` -- Configure dim attenuation
- `set_fade_ms(ms)` -- Configure transition time
- Implements `InPlacePlugin` trait

**Parameters:** `enabled` (bool), `channel_states` (JSON string), `dim_gain_db` (-60 to 0 dB, default -20), `fade_ms` (0-100 ms, default 5), `mute_{N}` / `solo_{N}` / `dim_{N}` (per-channel bool).

## Testing

```bash
cargo test -p sotf-plugin-channel-mute-solo
```

## Important Notes

- `from_params()` resets smoothers to current state immediately (no fade-in from previous state). This ensures the initial configuration is applied without transition artifacts.
- When `enabled=false` and all smoothers have converged to 1.0, processing is bypassed entirely (zero overhead).
- SIMD optimization via `apply_per_channel_gain_simd` from sotf-host.
- The `ChannelState` type is re-exported and used by `sotf-plugin-matrix` for its per-output channel mute/solo support.
- Dim gain default is -20 dB (~0.1 linear). Configurable via parameter or constructor.
