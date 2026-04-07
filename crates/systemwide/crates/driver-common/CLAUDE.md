# driver-common

Platform-agnostic audio driver trait for system-wide audio capture.

## Architecture

Single-file crate (`lib.rs`) defining the `AudioDriver` trait and supporting types. No platform-specific code.

```
lib.rs
  AudioDriver   -- Core trait: initialize/shutdown, read_audio, config negotiation, engine readiness
  NullDriver    -- No-op fallback (compiles everywhere, returns zero frames)
  DriverStatus  -- Runtime status: platform_supported, driver_installed, capture_active, sample_rate, channels, buffer_frames
  DriverConfig  -- Configuration request: sample_rate, buffer_frames
  ConfigResult  -- Accepted, Negotiated { actual_rate, actual_frames }, Error(String)
```

## Key Public API

- `AudioDriver` trait -- `initialize()`, `shutdown()`, `status()`, `read_audio()`, `available_frames()`, `sample_rate()`, `channel_count()`, `request_config()`, `poll_config_change()`, `acknowledge_config_change()`, `set_engine_ready()`
- `NullDriver` -- Fallback implementation: `platform_supported: false`, reads zero frames, always compiles
- `DriverStatus` -- Serializable status snapshot
- `DriverConfig` -- Sample rate + buffer size request
- `ConfigResult` -- Three-way result for config negotiation

## Testing

```bash
cargo test -p driver-common
```

## Important Notes

- `AudioDriver` is `Send + 'static` for use as `Box<dyn AudioDriver>` in the daemon
- `read_audio()` returns number of *samples* (not frames) -- caller provides buffer of `frame_count * channel_count` floats
- Driver-initiated config changes use the `poll_config_change()` / `acknowledge_config_change()` handshake
- Platform implementations: macOS HAL (`driver-hal`), Linux PipeWire (planned), Windows APO (planned)
- `NullDriver` allows the daemon to compile and run gracefully on unsupported platforms
