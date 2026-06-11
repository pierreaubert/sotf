# sotf-types

Shared configuration types for the SOTF audio system -- lightweight, serializable, no audio processing dependencies.

## Architecture

```
lib.rs            -- Re-exports all public types
audio_source.rs   -- AudioSource enum (file path, URL, stream), ServiceId
config.rs         -- EngineConfig: frame_size, buffer_ms, sample_rate, channels, plugins, volume, driver_mode, sink_type
plugin_config.rs  -- PluginConfig (type + JSON params), PluginGraphConfig, PluginGraphNodeConfig, PluginGraphEdgeConfig
sink.rs           -- SinkConfig, SinkOpenResult, SinkType (Cpal, PipeWire, AirPlay, etc.)
state.rs          -- AudioEngineState, AudioFrame, PlaybackState
```

## Key Public API

- `EngineConfig` -- Full engine configuration: frame_size, buffer_ms, output_sample_rate, input/output_channels, plugins, volume, muted, driver_mode, sink_type. Supports JSON serialization, `load_from_file()`, `save_to_file()`, `sanitize()`, version migration.
- `PluginConfig` -- Plugin type string + `serde_json::Value` parameters
- `PluginGraphConfig` / `PluginGraphNodeConfig` / `PluginGraphEdgeConfig` -- DAG-based plugin routing config
- `AudioSource` / `ServiceId` -- How to obtain audio (file, URL, stream)
- `SinkConfig` / `SinkType` / `SinkOpenResult` -- Output sink configuration
- `AudioEngineState` / `PlaybackState` / `AudioFrame` -- Runtime state types

## Testing

```bash
cargo test -p sotf-types
```

## Important Notes

- This crate intentionally has zero audio processing dependencies (no cpal, symphonia, rustfft) -- it is safe to depend on from any crate
- `EngineConfig::sanitize()` guards against zero frame_size and zero sample_rate from corrupt config files
- `EngineConfig::queue_capacity_frames()` uses `div_ceil` to avoid under-allocation
- `frame_size: 0` is handled gracefully in `queue_capacity_frames()` (treated as 1)
- Config version migration support is scaffolded but currently only version 1 exists
- Some fields are `#[serde(skip)]` (output_device, config_path, watch_config, sink_type) -- not persisted to JSON
